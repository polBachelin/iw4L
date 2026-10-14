//! `survival_paris`: one headless survival run on mp_paris, judged by the
//! gametype's own `gsc log: survival:` lines and the lifecycle boundaries.
//!
//! The steps, positions and the item bought are here and nowhere else; the
//! values the gametype is configured with (start credits, wave bonus, teams,
//! intermission, menus) are read from `crates/assets/src/survival/settings.csv`,
//! the file the binary embeds, never repeated here.

use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

pub const NAME: &str = "survival_paris";

const SETTINGS: &str = "crates/assets/src/survival/settings.csv";
const SURVIVAL: &str = "gsc log: survival: ";

/// Where the player stands to fight (x y z yaw pitch) and where an enemy is
/// brought to be shot (x y z yaw): docs/RUN.md's mp_paris recipe.
const PLAYER: &str = "-1214 2048 229 270 0";
/// `PLAYER` looking down at the middle of the body brought to `TARGET`, 58
/// units off: a held enemy keeps the stance it took, and a level shot from
/// the eye passes over a crouched one.
const WAVE_PLAYER: &str = "-1214 2048 229 270 27";
const TARGET: &str = "-1214 1990 229 90";
/// The weapon the waves are cleared with, a `give` reference.
const WEAPON: &str = "weapon/iw5:pecheneg";
/// Waves cleared before the purchase: the air support armory opens after the
/// third, and three waves pay for one airstrike.
const CLEARED_WAVES: u32 = 3;
/// The armory the purchase is made at and the item's row (c0) in its table.
const PURCHASE_KIND: &str = "airsupport";
const PURCHASE_ROW: &str = "10001";
/// How far in front of the armory, along the yaw it is placed with, the
/// player stands to use it: well inside the engine's use reach.
const ARMORY_STAND: f64 = 40.0;
const MAX_BURSTS: u32 = 40;
const MAX_REVIVE_TRIES: u32 = 20;

const LOAD_TIMEOUT: Duration = Duration::from_secs(600);
const STEP_TIMEOUT: Duration = Duration::from_secs(120);
const QUIT_TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(200);

type Result<T> = std::result::Result<T, String>;

/// The rows of `settings.csv` the scenario checks the run against.
struct Settings {
    map: String,
    enemy_team: String,
    start_credits: i64,
    wave_bonus: i64,
    max_health: i64,
    intermission_seconds: u64,
    eog_menu: String,
    armory_menu_prefix: String,
    armory_purchase_prefix: String,
    test_wave: String,
}

impl Settings {
    fn read(root: &Path) -> Result<Self> {
        let path = root.join(SETTINGS);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rows: BTreeMap<&str, &str> = text
            .lines()
            .filter_map(|line| line.split_once(','))
            .map(|(key, value)| (key.trim(), value.trim()))
            .collect();
        let text = |key: &str| {
            rows.get(key)
                .map(|value| (*value).to_owned())
                .ok_or_else(|| format!("{SETTINGS} has no `{key}` row"))
        };
        let number = |key: &str| {
            text(key)?
                .parse::<i64>()
                .map_err(|e| format!("{SETTINGS} `{key}`: {e}"))
        };
        let map = text("map")?;
        Ok(Self {
            map: map.rsplit(':').next().unwrap_or(&map).to_owned(),
            enemy_team: text("enemy_team")?,
            start_credits: number("start_credits")?,
            wave_bonus: number("wave_bonus")?,
            max_health: number("max_health")?,
            intermission_seconds: u64::try_from(number("intermission_seconds")?)
                .map_err(|e| format!("{SETTINGS} `intermission_seconds`: {e}"))?,
            eog_menu: text("eog_menu")?,
            armory_menu_prefix: text("armory_menu_prefix")?,
            armory_purchase_prefix: text("armory_purchase_prefix")?,
            test_wave: text("test_wave")?,
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "file": SETTINGS,
            "map": self.map,
            "enemy_team": self.enemy_team,
            "start_credits": self.start_credits,
            "wave_bonus": self.wave_bonus,
            "intermission_seconds": self.intermission_seconds,
            "eog_menu": self.eog_menu,
            "test_wave": self.test_wave,
        })
    }
}

/// The one game process, driven through its stdin console.
struct Game {
    child: Child,
    directory: PathBuf,
    started: Instant,
    log: Option<PathBuf>,
    offset: u64,
    partial: String,
    /// `gsc log: survival:` messages, prefix removed, in log order.
    lines: Vec<String>,
    /// The few engine lines the driver syncs on (`god:`, `menu open:`).
    engine: Vec<String>,
    marks: u32,
}

impl Game {
    fn launch(binary: &Path, directory: &Path) -> Result<Self> {
        let mut command = Command::new(binary);
        command
            .args(["survival", "--headless"])
            .current_dir(directory)
            .env("IW4L_ARTIFACTS_DIR", directory.join("iw4l-artifacts"))
            .env("IW4L_SETTINGS_PATH", directory.join("settings.cfg"))
            .env("IW4L_ACCOUNT_PATH", directory.join("account.dat"))
            .env("IW4L_CONSOLE_STDIN", "1")
            .env_remove("IW4L_CMDS")
            .env_remove("IW4L_LOG")
            .env_remove("IW4L_TRACES_DIR")
            .stdin(Stdio::piped())
            .stdout(
                fs::File::create(directory.join("child_stdout.txt")).map_err(|e| e.to_string())?,
            )
            .stderr(
                fs::File::create(directory.join("child_stderr.txt")).map_err(|e| e.to_string())?,
            );
        let child = command
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", binary.display()))?;
        println!("game: pid={} cwd={}", child.id(), directory.display());
        Ok(Self {
            child,
            directory: directory.to_path_buf(),
            started: Instant::now(),
            log: None,
            offset: 0,
            partial: String::new(),
            lines: Vec::new(),
            engine: Vec::new(),
            marks: 0,
        })
    }

    fn elapsed_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    fn stdout(&self) -> Result<String> {
        fs::read_to_string(self.directory.join("child_stdout.txt")).map_err(|e| e.to_string())
    }

    fn send(&mut self, script: &str) -> Result<()> {
        writeln!(
            self.child.stdin.as_mut().ok_or("console pipe closed")?,
            "{script}"
        )
        .map_err(|e| format!("console pipe: {e}"))?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join("commands.txt"))
            .map_err(|e| e.to_string())?;
        writeln!(log, "{script}").map_err(|e| e.to_string())
    }

    /// Reads what the game logged since the last poll. The log is the one the
    /// game announces on its first stdout line.
    fn poll(&mut self) -> Result<()> {
        if self.log.is_none() {
            let stdout = self.stdout()?;
            self.log = stdout
                .lines()
                .find_map(|line| line.strip_prefix("log: "))
                .map(|rest| rest.split(" (").next().unwrap_or(rest))
                .map(|path| self.directory.join(path));
        }
        let Some(path) = &self.log else {
            return Ok(());
        };
        let mut file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(end) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=end).collect();
            let line = line.trim_end();
            if let Some(at) = line.find(SURVIVAL) {
                self.lines.push(line[at + SURVIVAL.len()..].to_owned());
            } else if let Some(at) = ["god: client=", "menu open: "]
                .iter()
                .find_map(|prefix| line.find(prefix))
            {
                self.engine.push(line[at..].to_owned());
            }
        }
        Ok(())
    }

    fn alive(&mut self) -> Result<()> {
        match self.child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => Err(format!(
                "the game exited ({status}); see {}",
                self.directory.display()
            )),
            None => Ok(()),
        }
    }

    /// Polls until `found` answers, the game exits, or `timeout` passes.
    fn wait<T>(
        &mut self,
        what: &str,
        timeout: Duration,
        mut found: impl FnMut(&Self) -> Option<T>,
    ) -> Result<T> {
        let started = Instant::now();
        loop {
            self.poll()?;
            if let Some(value) = found(self) {
                return Ok(value);
            }
            self.alive()?;
            if started.elapsed() > timeout {
                return Err(format!(
                    "timed out after {}s waiting for {what}",
                    timeout.as_secs()
                ));
            }
            std::thread::sleep(POLL);
        }
    }

    /// Sends `script` and waits until the console has played all of it.
    fn play(&mut self, script: &str) -> Result<()> {
        self.marks += 1;
        let label = format!("step.{}", self.marks);
        self.send(&format!("{script}; mark {label}"))?;
        self.wait(
            &format!("the console to play `{script}`"),
            STEP_TIMEOUT,
            |game| {
                let stdout = game.stdout().ok()?;
                stdout
                    .lines()
                    .filter_map(|line| crate::runner::parse(line, "benchmark-mark:", 0))
                    .any(|event| event.get("label") == Some(label.as_str()))
                    .then_some(())
            },
        )
    }

    /// The first survival line at or after `from` that starts with `head`.
    fn find(&self, from: usize, head: &str) -> Option<usize> {
        self.lines[from.min(self.lines.len())..]
            .iter()
            .position(|line| line.starts_with(head))
            .map(|at| from + at)
    }

    fn wait_line(&mut self, from: usize, head: &str, timeout: Duration) -> Result<usize> {
        self.wait(&format!("`survival: {head}`"), timeout, |game| {
            game.find(from, head)
        })
    }

    fn engine_line(&mut self, from: usize, head: &str) -> Result<usize> {
        self.wait(&format!("`{head}`"), STEP_TIMEOUT, |game| {
            game.engine[from.min(game.engine.len())..]
                .iter()
                .position(|line| line.starts_with(head))
                .map(|at| from + at)
        })
    }

    /// The entity numbers of `team`'s actors of the window `from..` that
    /// spawned and were not killed.
    fn live_enemies(&self, from: usize, team: &str) -> Vec<u32> {
        let mut live = Vec::new();
        for line in &self.lines[from.min(self.lines.len())..] {
            if line.starts_with("enemy killed ") {
                if let Some(actor) = number(line, "actor") {
                    live.retain(|a| *a != actor as u32);
                }
            } else if line.contains(" spawned actor=")
                && field(line, "team") == Some(team)
                && let Some(actor) = number(line, "actor")
                && !live.contains(&(actor as u32))
            {
                live.push(actor as u32);
            }
        }
        live
    }

    fn finish(&mut self) -> Result<i32> {
        self.send("finish_run")?;
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                return status
                    .code()
                    .ok_or_else(|| format!("the game ended without an exit code: {status}"));
            }
            if started.elapsed() > QUIT_TIMEOUT {
                return Err(format!(
                    "the game did not exit within {}s of finish_run",
                    QUIT_TIMEOUT.as_secs()
                ));
            }
            std::thread::sleep(POLL);
        }
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.send("quit !");
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The value of `key=` in a log line.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(key)?.strip_prefix('='))
}

fn number(line: &str, key: &str) -> Option<i64> {
    field(line, key)?.parse().ok()
}

fn ensure(condition: bool, failure: impl FnOnce() -> String) -> Result<()> {
    if condition { Ok(()) } else { Err(failure()) }
}

/// The client-0 credits lines up to `end`: each must be the previous total
/// plus its delta, and the first is the configured start.
fn credit_ledger(lines: &[String], settings: &Settings) -> Result<Vec<(usize, i64, i64, String)>> {
    let mut ledger: Vec<(usize, i64, i64, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("credits client=0 ") {
            continue;
        }
        let credits = number(line, "credits").ok_or_else(|| format!("unreadable `{line}`"))?;
        let delta = number(line, "delta").ok_or_else(|| format!("unreadable `{line}`"))?;
        let reason = field(line, "reason").unwrap_or("").to_owned();
        match ledger.last() {
            None => ensure(
                reason == "start" && credits == settings.start_credits,
                || {
                    format!(
                        "the first credits line `{line}` is not the configured start {}",
                        settings.start_credits
                    )
                },
            )?,
            Some((_, previous, ..)) => ensure(credits == previous + delta, || {
                format!("`{line}` does not follow the previous total {previous}")
            })?,
        }
        ledger.push((index, credits, delta, reason));
    }
    Ok(ledger)
}

/// The enemies a wave's start line asks for, by archetype, less the ones the
/// wave skips: `squad=A xN`, `specials=B xM,C xK`, `bosses=D,E`.
fn expected_enemies(game: &Game, start: usize, wave: u32) -> Result<BTreeMap<String, i64>> {
    let line = &game.lines[start];
    let contents = line
        .split_once(" start")
        .map(|(_, rest)| rest.split(" total=").next().unwrap_or(rest))
        .unwrap_or("");
    ensure(!contents.contains("test="), || {
        format!("wave {wave} is `test_wave`, not the waves table: `{line}`")
    })?;
    ensure(!contents.contains("vehicles="), || {
        format!("wave {wave} has vehicles; this scenario clears actor waves: `{line}`")
    })?;
    let mut expected: BTreeMap<String, i64> = BTreeMap::new();
    let words = contents
        .replace("squad=", " ")
        .replace("specials=", " ")
        .replace("bosses=", " ")
        .replace(',', " ");
    let mut last: Option<String> = None;
    for word in words.split_whitespace() {
        match word.strip_prefix('x').and_then(|n| n.parse::<i64>().ok()) {
            Some(count) => {
                let archetype = last.take().ok_or_else(|| format!("unreadable `{line}`"))?;
                *expected.entry(archetype).or_default() += count;
            }
            None => {
                if let Some(archetype) = last.replace(word.to_owned()) {
                    *expected.entry(archetype).or_default() += 1;
                }
            }
        }
    }
    if let Some(archetype) = last {
        *expected.entry(archetype).or_default() += 1;
    }
    let skips = format!("wave {wave} skips ");
    for skip in game.lines[start..]
        .iter()
        .filter_map(|l| l.strip_prefix(&skips))
    {
        let mut parts = skip.split_whitespace();
        if let (Some(archetype), Some(count)) = (
            parts.next(),
            parts
                .next()
                .and_then(|n| n.strip_prefix('x')?.parse::<i64>().ok()),
        ) && let Some(n) = expected.get_mut(archetype)
        {
            *n -= count;
        }
    }
    expected.retain(|_, n| *n > 0);
    Ok(expected)
}

fn load(game: &mut Game, settings: &Settings, manifest: &mut Value) -> Result<Value> {
    let installed = game.wait("`gsc: installed` or `gsc: refused`", LOAD_TIMEOUT, |game| {
        game.stdout()
            .ok()?
            .lines()
            .find(|line| line.starts_with("gsc: installed") || line.starts_with("gsc: refused"))
            .map(str::to_owned)
    })?;
    let event = crate::runner::parse(&installed, "gsc:", 0).ok_or("unreadable gsc line")?;
    manifest["gsc"] = json!({
        "installed": event.name == "installed",
        "line": installed,
        "fields": event.fields,
    });
    ensure(event.name == "installed", || installed.clone())?;
    ensure(event.get("gametype") == Some("survival"), || {
        format!("installed gametype is not survival: {installed}")
    })?;
    ensure(event.get("map") == Some(settings.map.as_str()), || {
        format!("installed map is not {}: {installed}", settings.map)
    })?;
    game.play(&format!(
        "wait world; wait spawn; god; actor hold on; give {WEAPON}; tp {PLAYER}"
    ))?;
    game.engine_line(0, "god: client=0 on")?;
    let ready = game.wait_line(0, "ready ", STEP_TIMEOUT)?;
    let loadout = game.wait_line(0, "loadout ", STEP_TIMEOUT)?;
    let start = game.wait_line(0, "credits client=0 ", STEP_TIMEOUT)?;
    credit_ledger(&game.lines[..=start], settings)?;
    Ok(json!({
        "gsc": installed,
        "ready": game.lines[ready],
        "loadout": game.lines[loadout],
        "start_credits": game.lines[start],
    }))
}

fn wave(game: &mut Game, settings: &Settings, wave: u32) -> Result<Value> {
    let begin = format!("wave={wave} start");
    let start = game.wait_line(
        0,
        &begin,
        STEP_TIMEOUT + Duration::from_secs(settings.intermission_seconds),
    )?;
    let mut facts = json!({ "wave": wave, "start": game.lines[start] });
    if wave > 1 {
        let intermission = format!(
            "intermission {}s before wave={wave}",
            settings.intermission_seconds
        );
        let at = game.find(0, &intermission).ok_or_else(|| {
            format!("no `{intermission}` line: the intermission is not `intermission_seconds`")
        })?;
        facts["intermission"] = game.lines[at].clone().into();
        facts["unlocked"] = game.lines[..start]
            .iter()
            .filter(|l| {
                l.ends_with(&format!("after wave={}", wave - 1))
                    && l.starts_with("armory unlocked ")
            })
            .cloned()
            .collect::<Vec<_>>()
            .into();
    }
    let total = number(&game.lines[start], "total")
        .ok_or_else(|| format!("no total in `{}`", game.lines[start]))?;
    let expected = expected_enemies(game, start, wave)?;
    ensure(expected.values().sum::<i64>() == total, || {
        format!("wave {wave} asks for {expected:?}, which is not total={total}")
    })?;

    game.play(&format!("tp {WAVE_PLAYER}"))?;
    let complete = format!("wave={wave} complete");
    let mut bursts = 0;
    let team = settings.enemy_team.as_str();
    while game.find(start, &complete).is_none() {
        // One enemy at a time: a body stays on the spot for actor_corpse_ms
        // and would shield the next one brought there.
        let Some(enemy) = game.live_enemies(start, team).first().copied() else {
            game.wait(
                &format!("an enemy of wave {wave} or `{complete}`"),
                STEP_TIMEOUT,
                |game| {
                    (!game.live_enemies(start, team).is_empty()
                        || game.find(start, &complete).is_some())
                    .then_some(())
                },
            )?;
            continue;
        };
        bursts += 1;
        ensure(bursts <= MAX_BURSTS, || {
            format!("wave {wave} not cleared after {MAX_BURSTS} bursts")
        })?;
        game.play(&format!(
            "actor tp {enemy} {TARGET}; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s; press +reload; wait 3s; give ammo"
        ))?;
    }
    let end = game.find(start, &complete).expect("found above");
    let bonus = game.wait_line(end, "credits client=0 ", STEP_TIMEOUT)?;
    facts["bursts"] = bursts.into();
    facts["complete"] = game.lines[end].clone().into();

    // Who spawned and died between the start and the end. An actor's
    // entity number is free again once its body is deleted, so a later
    // enemy may reuse it: a number lives spawn, kill (s k), any times.
    let window = &game.lines[start..=end];
    let mut lives: BTreeMap<i64, String> = BTreeMap::new();
    let mut arrived: BTreeMap<String, i64> = BTreeMap::new();
    let mut kills = Vec::new();
    for line in window {
        let actor = number(line, "actor");
        if let Some((archetype, _)) = line.split_once(" spawned actor=")
            && field(line, "team") == Some(team)
        {
            let actor = actor.ok_or_else(|| format!("unreadable `{line}`"))?;
            *arrived.entry(archetype.to_owned()).or_default() += 1;
            lives.entry(actor).or_default().push('s');
        } else if line.starts_with("enemy killed ") {
            let actor = actor.ok_or_else(|| format!("unreadable `{line}`"))?;
            lives.entry(actor).or_default().push('k');
            ensure(field(line, "by") == Some("client0"), || {
                format!("`{line}`: the scenario's kills are the player's")
            })?;
            kills.push(line.clone());
        }
    }
    ensure(arrived == expected, || {
        format!("wave {wave} spawned {arrived:?}, its start line asks for {expected:?}")
    })?;
    ensure(kills.len() as i64 == total, || {
        format!("wave {wave} logged {} kills for total={total}", kills.len())
    })?;
    ensure(number(&game.lines[end], "killed") == Some(total), || {
        format!("`{}` is not total={total}", game.lines[end])
    })?;
    for (actor, life) in &lives {
        let mut rest = life.as_str();
        while let Some(next) = rest.strip_prefix("sk") {
            rest = next;
        }
        ensure(rest.is_empty() && !life.is_empty(), || {
            format!(
                "actor {actor} lived `{life}` in wave {wave}: an enemy spawns once and is killed (s k), never more"
            )
        })?;
    }

    // What the wave paid: one kill reward per kill, the same for every enemy
    // of an archetype, any challenge the kills completed, then the configured
    // bonus.
    let ledger = credit_ledger(&game.lines[..=bonus], settings)?;
    let before = ledger
        .iter()
        .rev()
        .find(|(index, ..)| *index < start)
        .map(|(_, credits, ..)| *credits)
        .ok_or("no credits before the wave")?;
    let paid: Vec<&(usize, i64, i64, String)> = ledger
        .iter()
        .filter(|(index, ..)| *index > start && *index < bonus)
        .collect();
    let mut rewards: BTreeMap<String, i64> = BTreeMap::new();
    let mut challenges = Vec::new();
    let mut kill_rewards: i64 = 0;
    for (index, _, delta, reason) in &paid {
        if let Some(challenge) = reason.strip_prefix("challenge:") {
            let completed = game.lines[start..*index].iter().rev().find_map(|line| {
                line.strip_prefix(&format!("challenge client=0 {challenge} "))
                    .and_then(|progress| progress.split_once('/'))
            });
            ensure(
                completed.is_some_and(|(done, required)| done == required),
                || {
                    format!(
                        "`{}` pays a challenge no progress line inside wave {wave} completed",
                        game.lines[*index]
                    )
                },
            )?;
            ensure(*delta > 0, || {
                format!("`{}` pays nothing", game.lines[*index])
            })?;
            challenges.push(game.lines[*index].clone());
            continue;
        }
        kill_rewards += 1;
        let archetype = reason.strip_prefix("kill:").ok_or_else(|| {
            format!(
                "`{}` inside wave {wave} is not a kill reward",
                game.lines[*index]
            )
        })?;
        ensure(*delta > 0, || {
            format!("`{}` pays nothing", game.lines[*index])
        })?;
        let reward = *rewards.entry(archetype.to_owned()).or_insert(*delta);
        ensure(reward == *delta, || {
            format!("`{archetype}` kills pay both {reward} and {delta}")
        })?;
    }
    ensure(kill_rewards == total, || {
        format!("wave {wave} paid {kill_rewards} kill rewards for total={total}")
    })?;
    let (_, after, bonus_delta, reason) = ledger.last().expect("bonus is in the ledger");
    ensure(
        *reason == format!("wave_bonus:{wave}") && *bonus_delta == settings.wave_bonus,
        || {
            format!(
                "`{}` is not the configured wave bonus {}",
                game.lines[bonus], settings.wave_bonus
            )
        },
    )?;
    let earned: i64 = paid.iter().map(|(_, _, delta, _)| delta).sum();
    ensure(*after == before + earned + settings.wave_bonus, || {
        format!(
            "credits {after} after wave {wave} are not {before} + {earned} + {}",
            settings.wave_bonus
        )
    })?;
    facts["spawned"] = json!(arrived);
    facts["kills"] = kills.into();
    facts["kill_rewards"] = json!(rewards);
    facts["challenge_rewards"] = challenges.into();
    facts["credits"] =
        json!({ "before": before, "kills": earned, "bonus": bonus_delta, "after": after });
    Ok(facts)
}

/// The origin and yaw of the `kind` armory the gametype logged it spawned.
fn armory_placement(game: &Game, kind: &str) -> Result<([f64; 3], f64)> {
    let head = format!("armory {kind} spawned at (");
    let at = game
        .find(0, &head)
        .ok_or_else(|| format!("no `{head}` line: no {kind} armory stands on the map"))?;
    let line = &game.lines[at];
    let unreadable = || format!("unreadable `{line}`");
    let (origin, rest) = line[head.len()..].split_once(')').ok_or_else(unreadable)?;
    let origin: Vec<f64> = origin
        .split(',')
        .map(|axis| axis.trim().parse::<f64>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| unreadable())?;
    let yaw = rest
        .split_whitespace()
        .skip_while(|word| *word != "yaw")
        .nth(1)
        .and_then(|yaw| yaw.parse::<f64>().ok())
        .ok_or_else(unreadable)?;
    match origin[..] {
        [x, y, z] => Ok(([x, y, z], yaw)),
        _ => Err(unreadable()),
    }
}

fn armory(game: &mut Game, settings: &Settings) -> Result<Value> {
    let unlocked = format!("armory unlocked {PURCHASE_KIND} ");
    let at = game.wait_line(0, &unlocked, STEP_TIMEOUT)?;
    let unlock = game.lines[at].clone();
    let usable = game.wait_line(
        at,
        &format!("armory {PURCHASE_KIND} usable at "),
        STEP_TIMEOUT,
    )?;
    let usable = game.lines[usable].clone();
    let before = credit_ledger(&game.lines, settings)?
        .last()
        .map(|(_, credits, ..)| *credits)
        .ok_or("no credits line")?;
    let menu = format!("{}{PURCHASE_KIND}", settings.armory_menu_prefix);
    let response = format!(
        "menuresponse {menu} {}{PURCHASE_ROW}",
        settings.armory_purchase_prefix
    );

    // Away from the armory, the same answer buys nothing.
    let from = game.lines.len();
    game.play(&format!("tp {PLAYER}; {response}"))?;
    let away = game.wait_line(from, "purchase refused client=0 ", STEP_TIMEOUT)?;
    let away = game.lines[away].clone();
    ensure(away.ends_with(": uses no armory"), || {
        format!("`{response}` away from the armory: {away}")
    })?;

    // At the armory, using it opens its menu.
    let ([x, y, z], yaw) = armory_placement(game, PURCHASE_KIND)?;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let stand = format!(
        "{:.0} {:.0} {z:.0} {:.0} 0",
        x + ARMORY_STAND * cos,
        y + ARMORY_STAND * sin,
        (yaw + 180.0) % 360.0
    );
    let from = game.lines.len();
    let menus = game.engine.len();
    game.play(&format!("tp {stand}; wait 0.5s; press +activate"))?;
    let opened = game.wait_line(
        from,
        &format!("armory {PURCHASE_KIND} opened client=0 "),
        STEP_TIMEOUT,
    )?;
    let opened = game.lines[opened].clone();
    let shown = game.engine_line(menus, &format!("menu open: {menu}"))?;
    let shown = game.engine[shown].clone();

    let from = game.lines.len();
    game.play(&response)?;
    let answer = game.wait("the purchase's answer", STEP_TIMEOUT, |game| {
        game.find(from, "bought ")
            .or_else(|| game.find(from, "purchase refused "))
    })?;
    let line = game.lines[answer].clone();
    ensure(line.starts_with("bought "), || {
        format!("`{response}`: {line}")
    })?;
    let item = line["bought ".len()..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();
    ensure(field(&line, "kind") == Some(PURCHASE_KIND), || {
        format!("`{line}` is not a {PURCHASE_KIND} item")
    })?;
    let price = number(&line, "price").ok_or_else(|| format!("no price in `{line}`"))?;
    let granted = game.wait_line(from, &format!("{item} on action slot "), STEP_TIMEOUT)?;
    let granted = game.lines[granted].clone();
    let paid = game.wait_line(from, "credits client=0 ", STEP_TIMEOUT)?;
    let ledger = credit_ledger(&game.lines[..=paid], settings)?;
    let (_, after, delta, reason) = ledger.last().expect("paid is in the ledger");
    ensure(
        *reason == format!("purchase:{item}") && *delta == -price && *after == before - price,
        || {
            format!(
                "`{}` is not {before} - {price} for {item}",
                game.lines[paid]
            )
        },
    )?;
    // Closing the menu ends the use.
    let from = game.lines.len();
    game.play("menukey escape")?;
    let closed = game.wait_line(
        from,
        &format!("armory {PURCHASE_KIND} closed client=0 reason=closed"),
        STEP_TIMEOUT,
    )?;
    Ok(json!({
        "unlocked": unlock,
        "usable": usable,
        "away": away,
        "stand": stand,
        "opened": opened,
        "menu": shown,
        "response": response,
        "bought": line,
        "granted": granted,
        "closed": game.lines[closed],
        "credits": { "before": before, "price": price, "after": after },
    }))
}

/// Damage that puts the player down from full: its health and every point of
/// the armor the start loadout gave, which a hit spends first.
fn down_damage(game: &Game, settings: &Settings) -> Result<i64> {
    let loadout = game.find(0, "loadout ").ok_or("no loadout line")?;
    let armor = number(&game.lines[loadout], "armor")
        .ok_or_else(|| format!("unreadable `{}`", game.lines[loadout]))?;
    Ok(settings.max_health + armor)
}

fn laststand(game: &mut Game, settings: &Settings, wave: u32) -> Result<(Value, usize)> {
    let start = game.wait_line(0, &format!("wave={wave} start"), STEP_TIMEOUT * 2)?;
    game.wait(&format!("an enemy of wave {wave}"), STEP_TIMEOUT, |game| {
        (!game.live_enemies(start, &settings.enemy_team).is_empty()).then_some(())
    })?;
    // The waves were cleared with `WEAPON`, which took the start pistol's
    // place; last stand draws a pistol, so the start one is given back.
    let loadout = game.find(0, "loadout ").ok_or("no loadout line")?;
    let pistol = game.lines[loadout]["loadout ".len()..]
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("unreadable `{}`", game.lines[loadout]))?
        .to_owned();
    let gods = game.engine.len();
    game.play(&format!(
        "tp {PLAYER}; give weapon/{pistol}; give ammo; god"
    ))?;
    game.engine_line(gods, "god: client=0 off")?;
    let damage = down_damage(game, settings)?;
    let from = game.lines.len();
    game.play(&format!("damage {damage}"))?;
    let down = game.wait("the player to go down", STEP_TIMEOUT, |game| {
        game.find(from, "laststand client=0 ")
            .or_else(|| game.find(from, "player died client=0 "))
    })?;
    let line = game.lines[down].clone();
    ensure(line.starts_with("laststand "), || {
        format!("`damage {damage}` with a self-revive held: {line}")
    })?;
    let held = number(&line, "selfrevive").ok_or_else(|| format!("unreadable `{line}`"))?;
    ensure(held >= 1, || format!("`{line}` holds no self-revive"))?;
    ensure(field(&line, "weapon") == Some(pistol.as_str()), || {
        format!("`{line}` does not draw the start pistol {pistol}")
    })?;

    let mut tries = 0;
    let revived = loop {
        if let Some(at) = game.find(down, "revived by kill ") {
            break at;
        }
        if let Some(at) = game.find(down, "player died ") {
            return Err(format!(
                "died in last stand instead of getting up: {}",
                game.lines[at]
            ));
        }
        tries += 1;
        ensure(tries <= MAX_REVIVE_TRIES, || {
            format!("no revive after {MAX_REVIVE_TRIES} tries")
        })?;
        let Some(enemy) = game
            .live_enemies(start, &settings.enemy_team)
            .first()
            .copied()
        else {
            game.wait(&format!("an enemy of wave {wave}"), STEP_TIMEOUT, |game| {
                (!game.live_enemies(start, &settings.enemy_team).is_empty()).then_some(())
            })?;
            continue;
        };
        let shot = "hold +attack; wait 0.3s; release +attack; wait 0.4s";
        game.play(&format!(
            "actor tp {enemy} {TARGET}; wait 0.5s; {shot}; {shot}; {shot}; wait 0.5s"
        ))?;
    };
    let revive = game.lines[revived].clone();
    ensure(number(&revive, "selfrevive") == Some(held - 1), || {
        format!("`{revive}` did not spend one of {held} self-revives")
    })?;
    let kill = game.lines[down..revived]
        .iter()
        .find(|l| l.starts_with("enemy killed ") && field(l, "by") == Some("client0"))
        .cloned()
        .ok_or("revived without a kill by the player")?;
    Ok((
        json!({ "wave": game.lines[start], "down": line, "kill": kill, "revived": revive, "tries": tries }),
        revived,
    ))
}

fn gameover(game: &mut Game, settings: &Settings, wave: u32, from: usize) -> Result<Value> {
    game.play(&format!("damage {}", down_damage(game, settings)?))?;
    let over = game.wait_line(from, "game over ", STEP_TIMEOUT)?;
    let window = &game.lines[from..over];
    ensure(!window.iter().any(|l| l.starts_with("laststand ")), || {
        "went down again with no self-revive left; that is death".into()
    })?;
    let died = window
        .iter()
        .find(|l| l.starts_with("player died client=0 "))
        .cloned()
        .ok_or("game over without the player dying")?;
    ensure(number(&died, "selfrevive") == Some(0), || {
        format!("`{died}` died holding a self-revive")
    })?;
    let line = game.lines[over].clone();
    let kills = game
        .lines
        .iter()
        .filter(|l| l.starts_with("enemy killed ") && field(l, "by") == Some("client0"))
        .count() as i64;
    ensure(number(&line, "wave") == Some(i64::from(wave)), || {
        format!("`{line}` is not wave {wave}")
    })?;
    ensure(number(&line, "kills") == Some(kills), || {
        format!("`{line}` does not count the {kills} kills logged")
    })?;
    ensure(
        field(&line, "menu") == Some(settings.eog_menu.as_str()),
        || format!("`{line}` does not open eog_menu {}", settings.eog_menu),
    )?;
    let menu = game.engine_line(0, &format!("menu open: {}", settings.eog_menu))?;
    Ok(json!({ "died": died, "game_over": line, "menu": game.engine[menu] }))
}

fn quit(game: &mut Game) -> Result<Value> {
    let code = game.finish()?;
    let stdout = game.stdout()?;
    let exit = stdout
        .lines()
        .filter_map(|line| crate::runner::parse(line, "lifecycle:", 0))
        .find(|event| event.name == "process_exit")
        .ok_or("no `lifecycle: process_exit` line")?;
    ensure(code == 0 && exit.get("code") == Some("0"), || {
        format!("exit code {code}, lifecycle {:?}", exit.get("code"))
    })?;
    Ok(json!({ "code": code, "lifecycle": exit.fields }))
}

fn phase(
    game: &mut Game,
    manifest: &mut Value,
    run: &Path,
    name: &str,
    step: impl FnOnce(&mut Game, &mut Value) -> Result<Value>,
) -> Result<()> {
    let started_ms = game.elapsed_ms();
    println!("[{:>7.1}s] phase {name}", started_ms as f64 / 1000.0);
    let outcome = step(game, manifest);
    let mut record = json!({
        "name": name,
        "started_ms": started_ms,
        "ended_ms": game.elapsed_ms(),
        "passed": outcome.is_ok(),
    });
    match &outcome {
        Ok(facts) => record["facts"] = facts.clone(),
        Err(reason) => record["failure_reason"] = reason.clone().into(),
    }
    manifest["phases"]
        .as_array_mut()
        .ok_or("phases missing")?
        .push(record);
    crate::report::write(run, manifest)?;
    outcome
        .map(drop)
        .map_err(|reason| format!("phase {name}: {reason}"))
}

fn execute(game: &mut Game, settings: &Settings, run: &Path, manifest: &mut Value) -> Result<()> {
    phase(game, manifest, run, "load", |game, manifest| {
        load(game, settings, manifest)
    })?;
    for number in 1..=CLEARED_WAVES {
        phase(game, manifest, run, &format!("wave{number}"), |game, _| {
            wave(game, settings, number)
        })?;
    }
    phase(game, manifest, run, "armory", |game, _| {
        armory(game, settings)
    })?;
    let mut revived = 0;
    phase(game, manifest, run, "laststand", |game, _| {
        let (facts, at) = laststand(game, settings, CLEARED_WAVES + 1)?;
        revived = at;
        Ok(facts)
    })?;
    phase(game, manifest, run, "gameover", |game, _| {
        gameover(game, settings, CLEARED_WAVES + 1, revived)
    })?;
    phase(game, manifest, run, "quit", |game, _| quit(game))
}

pub fn run(root: &Path, binary: &Path, cache: crate::CacheMode) -> Result<bool> {
    asset_transport::load_dotenv();
    if !binary.is_file() {
        return Err(format!(
            "{} is not built (make approved builds it)",
            binary.display()
        ));
    }
    let settings = Settings::read(root)?;
    let games = asset_transport::games_root_from_env()?;
    let run_id = format!("{}-{NAME}-{}", crate::utc_stamp(), std::process::id());
    let run = root.join("iw4l-artifacts/approved-tests").join(&run_id);
    fs::create_dir_all(&run).map_err(|e| format!("create {}: {e}", run.display()))?;
    let mut manifest = json!({
        "scenario": NAME,
        "run_id": run_id,
        "result": "running",
        "build": crate::report::build_facts(root, binary),
        "platform": crate::report::platform_facts(),
        "content": { "games_root": games.0.display().to_string() },
        "settings": settings.to_json(),
        "steps": {
            "player": PLAYER,
            "target": TARGET,
            "weapon": WEAPON,
            "cleared_waves": CLEARED_WAVES,
            "purchase": { "kind": PURCHASE_KIND, "row": PURCHASE_ROW },
            "down_damage": "max_health + the start loadout's armor",
        },
        "gsc": { "installed": false },
        "phases": [],
    });
    if !settings.test_wave.is_empty() {
        manifest["result"] = "blocked".into();
        manifest["failure"] = format!(
            "{SETTINGS} sets test_wave `{}`: wave 1 would not be the waves table's",
            settings.test_wave
        )
        .into();
        crate::report::write(&run, &manifest)?;
        println!("blocked: {}", run.join("run.json").display());
        return Ok(false);
    }
    manifest["cache"] = crate::prepare_cache(root, &run, cache)?;
    let binary = crate::stage_binary(binary, &run)?;
    manifest["command_line"] = json!([binary.display().to_string(), ["survival", "--headless"]]);
    crate::report::write(&run, &manifest)?;
    println!("run: {}", run.display());

    let mut game = Game::launch(&binary, &run)?;
    let outcome = execute(&mut game, &settings, &run, &mut manifest);
    drop(game);
    let read = |name: &str| {
        let path = run.join(name);
        fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let (stdout, stderr) = (read("child_stdout.txt")?, read("child_stderr.txt")?);
    let fault = stderr
        .lines()
        .find(|line| line.contains("GSC execution failed"))
        .or_else(|| {
            stdout.lines().find(|line| {
                line.starts_with("gsc: refused") || line.starts_with("gsc: runtime_error")
            })
        });
    manifest["no_execution_fault"] = fault.is_none().into();
    let outcome = outcome.and_then(|()| match fault {
        Some(line) => Err(format!("script fault: {line}")),
        None => Ok(()),
    });
    manifest["result"] = if outcome.is_ok() { "passed" } else { "failed" }.into();
    if let Err(error) = &outcome {
        manifest["failure"] = error.clone().into();
    }
    crate::report::write(&run, &manifest)?;
    println!("{}: {}", manifest["result"], run.join("run.json").display());
    Ok(outcome.is_ok())
}
