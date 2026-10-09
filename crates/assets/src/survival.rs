use std::collections::BTreeMap;

use crate::ScriptTable;

const SETTINGS_TABLE: &str = "survival/settings.csv";
const SETTINGS: &str = include_str!("survival/settings.csv");

const WEAPON_ALIASES_TABLE: &str = "survival/weapon_aliases.csv";
const ARCHETYPES_TABLE: &str = "survival/archetypes.csv";
const HUD_TABLE: &str = "survival/hud.csv";
const CHALLENGES_TABLE: &str = "survival/challenges.csv";
const EOG_TABLE: &str = "survival/eog.csv";
const AIRSUPPORT_TABLE: &str = "survival/airsupport.csv";
const HEAVIES_TABLE: &str = "survival/heavies.csv";
const GRANTS_TABLE: &str = "survival/grants.csv";
const PLAYER_DATA_TABLE: &str = "survival/playerdata.csv";

/// The tables survival authors itself, beside the MW3 tables it reads from
/// the user's zones: only what those tables lack.
const TABLES: [(&str, &str); 9] = [
    (
        WEAPON_ALIASES_TABLE,
        include_str!("survival/weapon_aliases.csv"),
    ),
    (ARCHETYPES_TABLE, include_str!("survival/archetypes.csv")),
    (HUD_TABLE, include_str!("survival/hud.csv")),
    (CHALLENGES_TABLE, include_str!("survival/challenges.csv")),
    (EOG_TABLE, include_str!("survival/eog.csv")),
    (AIRSUPPORT_TABLE, include_str!("survival/airsupport.csv")),
    (HEAVIES_TABLE, include_str!("survival/heavies.csv")),
    (GRANTS_TABLE, include_str!("survival/grants.csv")),
    (PLAYER_DATA_TABLE, include_str!("survival/playerdata.csv")),
];

const MAP_TABLES: [(&str, &str); 1] = [("mp_paris", include_str!("survival/maps/mp_paris.csv"))];

const ARMORY_KINDS: [&str; 3] = ["weapon", "equipment", "airsupport"];

/// MW3's waves table (`waves_table`) as the gametype script reads it. A row
/// whose c1 is `1` is a normal-mode wave (the chaos tables carry `0`).
mod wave_column {
    pub(super) const MARKER: usize = 1;
    pub(super) const NUMBER: usize = 2;
    pub(super) const SQUAD: usize = 3;
    pub(super) const SQUAD_COUNT: usize = 4;
    /// Space separated, parallel to `SPECIAL_COUNTS`.
    pub(super) const SPECIALS: usize = 5;
    pub(super) const SPECIAL_COUNTS: usize = 6;
    /// Space separated, one enemy each.
    pub(super) const BOSSES: usize = 7;
    pub(super) const VEHICLES: usize = 8;
    /// `1` on the waves that repeat past the last row (our reading).
    pub(super) const REPEAT: usize = 9;
    /// The armory the end of the wave unlocks.
    pub(super) const ARMORY: usize = 10;
}

/// An archetype of the waves table: a row whose c0 index lies here, c1 its
/// ref.
const ARCHETYPE_ROWS: std::ops::Range<u32> = 100..1000;

mod archetype_column {
    pub(super) const WEAPON: usize = 5;
    pub(super) const OFFHAND: usize = 6;
    pub(super) const HEALTH: usize = 7;
    /// The credits a player's kill of it pays.
    pub(super) const REWARD: usize = 9;
}

/// MW3's challenge table (`challenge_table`): a row whose c6 is a positive
/// count is a challenge, c1 its ref.
mod challenge_column {
    pub(super) const REF: usize = 1;
    pub(super) const REQUIRED: usize = 6;
    pub(super) const REWARD: usize = 7;
}

/// The hudelems the gametype script looks up by ref in `HUD_TABLE`.
const HUD_REFS: [&str; 3] = ["wave", "enemies", "credits"];

/// `HUD_TABLE`: ref, label, font, font scale in percent, horzalign,
/// vertalign, alignx, aligny, x, y.
const HUD_COLUMNS: usize = 10;

/// The settings the economy adds up: whole, non-negative credits.
const CREDIT_SETTINGS: [&str; 2] = ["start_credits", "wave_bonus"];

/// The settings the script waits on: whole seconds, at least one.
const SECONDS_SETTINGS: [&str; 3] = [
    "intermission_seconds",
    "bleedout_seconds",
    "getup_shock_seconds",
];

/// The names the end of a run is shown with: the shellshock of a revive,
/// MW3's summary menu and its death hint.
const NAME_SETTINGS: [&str; 3] = ["getup_shock", "eog_menu", "eog_deadquote"];

/// The settings in whole percent, 0 to 100: the share of a hit armor takes.
const PERCENT_SETTINGS: [&str; 1] = ["armor_absorb_percent"];

/// The settings the armory menus are answered by: their name prefix, the
/// prefix of a purchase and the answer they close with, the menu that warns
/// a purchase will replace a weapon and its answer to go ahead.
const ARMORY_MENU_SETTINGS: [&str; 5] = [
    "armory_menu_prefix",
    "armory_purchase_prefix",
    "armory_close_response",
    "armory_replacement_menu",
    "armory_replacement_continue",
];

/// The fields of `PLAYER_DATA_TABLE` the gametype script writes by name:
/// the credits and the experience MW3's armory menus read.
const PLAYER_DATA_SCALARS: [&str; 2] = ["surHUD_credits", "experience"];

/// MW3's Special Ops rank table (`rank_table`), as levelForExperienceSO reads
/// it: c0 the rank, c2 the experience it starts at, c7 the experience that
/// ends it; the `maxrank` row's c1 is the last rank.
mod rank_column {
    pub(super) const START: usize = 2;
    pub(super) const END: usize = 7;
    pub(super) const MAX_ROW: &str = "maxrank";
    pub(super) const MAX: usize = 1;
}

/// The armory table's column the gametype script keys `PLAYER_DATA_TABLE`
/// fields by: an item's kind.
const ARMORY_KIND_COLUMN: usize = 2;

/// How `GRANTS_TABLE` gives an armory item (c1): the two that wait on the
/// action slot, and a sentry the player carries and places, whose model is
/// c2. The predator's ref names, through `WEAPON_ALIASES_TABLE`, the
/// detonator it is held as and then the missile it launches. The ammo refill
/// tops up every weapon a launcher row does not name; a grenade, an
/// explosive (held on the action slot in c4) and a launcher (a weapon that
/// takes a primary slot) are counted in their weapon's ammo, armor in
/// points, a self-revive in revives: c3 is how many one purchase adds, empty
/// to fill up to the item's most (armory table c10).
mod grant {
    pub(super) const AIRSTRIKE: &str = "airstrike";
    pub(super) const PREDATOR: &str = "predator";
    pub(super) const SENTRY: &str = "sentry";
    pub(super) const AMMO: &str = "ammo";
    pub(super) const GRENADE: &str = "grenade";
    pub(super) const EXPLOSIVE: &str = "explosive";
    pub(super) const LAUNCHER: &str = "launcher";
    pub(super) const ARMOR: &str = "armor";
    pub(super) const LASTSTAND: &str = "laststand";
    pub(super) const ALL: [&str; 9] = [
        AIRSTRIKE, PREDATOR, SENTRY, AMMO, GRENADE, EXPLOSIVE, LAUNCHER, ARMOR, LASTSTAND,
    ];
    /// The grants that hand the player a weapon: their ref names it.
    pub(super) const WEAPONS: [&str; 6] =
        [AIRSTRIKE, PREDATOR, SENTRY, GRENADE, EXPLOSIVE, LAUNCHER];
    /// The grants counted up to the item's most, which c3 may step.
    pub(super) const COUNTED: [&str; 5] = [GRENADE, EXPLOSIVE, LAUNCHER, ARMOR, LASTSTAND];
    /// What an armory item of `WEAPON_KIND` with no row is given as: the
    /// weapon its ref names. A primary grant takes a slot of
    /// `primary_slots`.
    pub(super) const WEAPON: &str = "weapon";
    pub(super) const PRIMARY: [&str; 2] = [WEAPON, LAUNCHER];
    pub(super) const MODEL: usize = 2;
    pub(super) const AMOUNT: usize = 3;
    pub(super) const SLOT: usize = 4;
}

/// The armory kind whose items `GRANTS_TABLE` does not list are weapons,
/// named by their ref.
const WEAPON_KIND: &str = "weapon";

/// The armory table's column holding the most of an item a player holds.
const ARMORY_MOST_COLUMN: usize = 10;

/// The summary menu (`eog_menu`) reads its rows from the client dvars
/// `ui_eog_r<row>c<column>_player1`: c1 the label, c2 to c4 values.
const EOG_ROWS: std::ops::RangeInclusive<u32> = 1..=9;
const EOG_VALUE_COLUMNS: std::ops::RangeInclusive<u32> = 2..=4;

/// The stats the gametype script can put in a summary row.
const EOG_STATS: [&str; 7] = [
    "wave",
    "time",
    "credits",
    "kills",
    "headshots",
    "credits_earned",
    "downs",
];

/// A settings row `squad_<archetype>` makes that archetype of the waves table
/// an armory squad (its ref in the armory table) of that many allies.
const SQUAD_SETTING_PREFIX: &str = "squad_";

/// `AIRSUPPORT_TABLE` rows the script reads as whole numbers above zero.
const AIRSUPPORT_NUMBERS: [&str; 13] = [
    "action_slot",
    "airstrike_bombs",
    "airstrike_spacing",
    "airstrike_radius",
    "airstrike_damage_max",
    "airstrike_damage_min",
    "airstrike_height",
    "airstrike_interval_ms",
    "airstrike_flight",
    "airstrike_flight_ms",
    "support_settle_ms",
    "predator_height",
    "predator_offset",
];

/// `AIRSUPPORT_TABLE` rows that name a material, a model, an effect or a
/// means of death.
const AIRSUPPORT_NAMES: [&str; 4] = [
    "airstrike_selector",
    "airstrike_fx",
    "airstrike_means",
    "airstrike_plane",
];

/// `AIRSUPPORT_TABLE` rows that name a weapon of the loaded zones.
const AIRSUPPORT_WEAPONS: [&str; 1] = ["airstrike_bomb"];

/// `ARCHETYPES_TABLE`: ref, weapon, body, trait, move speed in percent.
mod fill_in_column {
    pub(super) const WEAPON: usize = 1;
    pub(super) const TRAIT: usize = 3;
    pub(super) const SPEED: usize = 4;
}

/// What the gametype script does for an archetype beyond its weapon and
/// health (`ARCHETYPES_TABLE` c3), and the `HEAVIES_TABLE` rows each reads.
const TRAITS: [(&str, &[&str]); 5] = [
    (
        "explosive",
        &["explosive_means", "explosive_damage_percent"],
    ),
    (
        "martyrdom",
        &[
            "martyrdom_weapon",
            "martyrdom_drop_height",
            "martyrdom_delay_ms",
        ],
    ),
    (
        "claymore",
        &[
            "claymore_weapon",
            "claymore_count",
            "claymore_plant_range",
            "claymore_plant_height",
            "claymore_plant_reach",
            "claymore_arm_ms",
            "claymore_trip_radius",
            "claymore_trip_arc",
            "claymore_trip_delay_ms",
            "claymore_poll_ms",
        ],
    ),
    (
        "gas",
        &[
            "gas_fx",
            "gas_means",
            "gas_radius",
            "gas_damage",
            "gas_ms",
            "gas_interval_ms",
            "gas_high_radius",
            "gas_med_radius",
            "gas_shock_high",
            "gas_shock_med",
            "gas_shock_low",
            "gas_shock_ms",
        ],
    ),
    (
        "chopper",
        &[
            "chopper_vehicle",
            "chopper_path_noteworthy",
            "chopper_start_targetname",
            "chopper_death_fx",
            "chopper_speed",
            "chopper_accel",
            "chopper_spawn_interval_ms",
            "chopper_dwell_ms",
            "chopper_range",
            "chopper_aim_height",
            "chopper_burst_ms",
            "chopper_rest_ms",
            "chopper_spread",
        ],
    ),
];

/// `HEAVIES_TABLE` rows that name a weapon ref (through `WEAPON_ALIASES_TABLE`),
/// an effect, a shellshock, means of death, a vehicle or a map key value,
/// rather than a whole number.
const HEAVIES_WEAPONS: [&str; 2] = ["martyrdom_weapon", "claymore_weapon"];
const HEAVIES_MEANS: [&str; 2] = ["explosive_means", "gas_means"];
const HEAVIES_NAMES: [&str; 8] = [
    "gas_fx",
    "gas_shock_high",
    "gas_shock_med",
    "gas_shock_low",
    "chopper_vehicle",
    "chopper_path_noteworthy",
    "chopper_start_targetname",
    "chopper_death_fx",
];

/// Action slots run 1 to 4.
const ACTION_SLOTS: std::ops::RangeInclusive<u32> = 1..=4;

/// An IW5 runtime carries compiled scripts only, so every module the engine
/// starts or calls by name is ours.
const MODULES: [(&str, &str); 5] = [
    (
        "codescripts/delete",
        include_str!("survival/codescripts_delete.gsc.txt"),
    ),
    (
        "codescripts/struct",
        include_str!("survival/codescripts_struct.gsc.txt"),
    ),
    (
        "maps/mp/gametypes/_callbacksetup",
        include_str!("survival/callbacksetup.gsc.txt"),
    ),
    ("maps/mp/_load", include_str!("survival/load.gsc.txt")),
    ("maps/mp/_compass", include_str!("survival/compass.gsc.txt")),
];

const GAMETYPE: &str = include_str!("survival/survival.gsc.txt");

pub fn survival_setting(key: &str) -> Option<&'static str> {
    SETTINGS
        .lines()
        .filter_map(|line| line.split_once(','))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case(key))
        .map(|(_, value)| value.trim())
}

fn required_setting(key: &str) -> Result<&'static str, String> {
    survival_setting(key).ok_or_else(|| format!("{SETTINGS_TABLE} has no `{key}` row"))
}

/// The MW3 zones survival loads after `common_mp`, in load order. Survival
/// reads its tables, menus, weapons and sounds from them, so a settings file
/// that names none is refused rather than read as a mode without zones.
pub fn survival_mode_zones() -> Result<asset_transport::ModeZones, String> {
    let zones = asset_transport::ModeZones::parse(required_setting("mode_zones")?);
    if zones.is_empty() {
        return Err(format!("{SETTINGS_TABLE}: `mode_zones` names no zone"));
    }
    Ok(zones)
}

/// The sides survival is played on. The gametype script reads the same rows,
/// so a settings file that names no two opposing teams is refused here.
pub fn survival_team_rules() -> Result<gamemode_iw4::survival::TeamRules, String> {
    gamemode_iw4::survival::TeamRules::parse(
        required_setting("human_team")?,
        required_setting("enemy_team")?,
    )
    .map_err(|error| format!("{SETTINGS_TABLE}: {error}"))
}

/// Holds survival's own tables to the data loaded with the match: every
/// weapon they name is one of its weapons, every archetype they fill in is
/// one of the waves table's, and every armory placement on `zone` is
/// readable. The gametype script trusts them, so a gap refuses the match.
pub fn survival_data_check(
    tables: &BTreeMap<String, ScriptTable>,
    zone: &str,
    is_weapon: impl Fn(&str) -> bool,
) -> Result<(), String> {
    let table = |name: &str| {
        tables
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| format!("no loaded zone holds `{name}`"))
    };
    let mut gaps = Vec::new();
    for row in table(WEAPON_ALIASES_TABLE)?.rows() {
        let reference = cell(row, 0);
        let weapons: Vec<&str> = cell(row, 1).split_whitespace().collect();
        if weapons.is_empty() {
            gaps.push(format!(
                "{WEAPON_ALIASES_TABLE}: `{reference}` names no weapon"
            ));
        }
        for weapon in weapons.into_iter().filter(|weapon| !is_weapon(weapon)) {
            gaps.push(format!(
                "{WEAPON_ALIASES_TABLE}: `{reference}` -> `{weapon}` is no weapon of the loaded zones"
            ));
        }
    }
    let friendly_fire = required_setting("friendly_fire")?;
    if !matches!(friendly_fire, "0" | "1") {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `friendly_fire` is neither 0 nor 1: `{friendly_fire}`"
        ));
    }
    let waves_name = required_setting("waves_table")?;
    let waves = table(waves_name)?;
    for row in table(ARCHETYPES_TABLE)?.rows() {
        let (archetype, weapon) = (cell(row, 0), cell(row, fill_in_column::WEAPON));
        let trait_name = cell(row, fill_in_column::TRAIT);
        if !trait_name.is_empty() && !TRAITS.iter().any(|(name, _)| *name == trait_name) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` trait `{trait_name}` is none the script plays ({})",
                TRAITS.map(|(name, _)| name).join(", ")
            ));
        }
        let speed = cell(row, fill_in_column::SPEED);
        if !speed.is_empty() && !speed.parse::<u32>().is_ok_and(|speed| speed > 0) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` move speed `{speed}` is not a whole percent above zero"
            ));
        }
        if !waves
            .rows()
            .any(|wave| cell(wave, 1).eq_ignore_ascii_case(archetype))
        {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` is no archetype of {waves_name}"
            ));
        }
        if !weapon.is_empty() && !is_weapon(weapon) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` -> `{weapon}` is no weapon of the loaded zones"
            ));
        }
    }
    check_waves(
        table(ARCHETYPES_TABLE)?,
        table(WEAPON_ALIASES_TABLE)?,
        waves_name,
        waves,
        &is_weapon,
        &mut gaps,
    )?;
    let challenges_name = required_setting("challenge_table")?;
    check_economy(
        table(HUD_TABLE)?,
        table(CHALLENGES_TABLE)?,
        challenges_name,
        table(challenges_name)?,
        &mut gaps,
    )?;
    check_endgame(table(EOG_TABLE)?, &mut gaps)?;
    check_airsupport(table(AIRSUPPORT_TABLE)?, &is_weapon, &mut gaps);
    let armory_name = required_setting("armory_table")?;
    check_grants(
        table(GRANTS_TABLE)?,
        table(WEAPON_ALIASES_TABLE)?,
        table(AIRSUPPORT_TABLE)?,
        armory_name,
        table(armory_name)?,
        &is_weapon,
        &mut gaps,
    );
    check_armory_settings(&mut gaps)?;
    check_weapon_slots(&mut gaps)?;
    check_player_data(
        table(PLAYER_DATA_TABLE)?,
        armory_name,
        table(armory_name)?,
        &mut gaps,
    );
    let rank_name = required_setting("rank_table")?;
    check_ranks(rank_name, table(rank_name)?, &mut gaps)?;
    check_heavies(
        table(HEAVIES_TABLE)?,
        table(ARCHETYPES_TABLE)?,
        table(WEAPON_ALIASES_TABLE)?,
        &is_weapon,
        &mut gaps,
    );
    let placements_name = map_table_name(zone);
    if let Some(placements) = tables.get(&placements_name.to_ascii_lowercase()) {
        let drops_name = required_setting("drop_table")?;
        let drops = table(drops_name)?;
        for row in placements.rows() {
            let (kind, origin) = (cell(row, 0), cell(row, 1));
            if !ARMORY_KINDS.contains(&kind) {
                gaps.push(format!(
                    "{placements_name}: `{kind}` is no armory kind ({})",
                    ARMORY_KINDS.join(", ")
                ));
            }
            let readable = match origin.split_whitespace().collect::<Vec<_>>()[..] {
                ["drop", key] => drops.rows().any(|drop| cell(drop, 0) == key),
                [x, y, z] => [x, y, z].iter().all(|axis| axis.parse::<i32>().is_ok()),
                _ => false,
            };
            if !readable {
                gaps.push(format!(
                    "{placements_name}: {kind} origin `{origin}` is neither `x y z` nor `drop <row of {drops_name}>` in whole units"
                ));
            }
            if cell(row, 2).parse::<i32>().is_err() || cell(row, 3).is_empty() {
                gaps.push(format!("{placements_name}: {kind} needs a yaw and a model"));
            }
        }
    }
    match gaps.is_empty() {
        true => Ok(()),
        false => Err(format!("survival data: {}", gaps.join("; "))),
    }
}

/// Holds every wave row to the archetypes and armories the script plays it
/// with: waves numbered 1 to N with a repeat block, refs that name
/// archetypes, parallel special counts, and for every archetype a bot plays
/// a weapon of the loaded zones and a health.
fn check_waves(
    archetypes: &ScriptTable,
    aliases: &ScriptTable,
    waves_name: &str,
    waves: &ScriptTable,
    is_weapon: &impl Fn(&str) -> bool,
    gaps: &mut Vec<String>,
) -> Result<(), String> {
    use wave_column as w;
    let archetype_row = |reference: &str| {
        waves.rows().find(|row| {
            cell(row, 0)
                .parse::<u32>()
                .is_ok_and(|index| ARCHETYPE_ROWS.contains(&index))
                && cell(row, 1).eq_ignore_ascii_case(reference)
        })
    };
    let unsimulated: Vec<&str> = required_setting("unsimulated_archetypes")?
        .split_whitespace()
        .collect();
    for reference in &unsimulated {
        if archetype_row(reference).is_none() {
            gaps.push(format!(
                "{SETTINGS_TABLE}: unsimulated `{reference}` is no archetype of {waves_name}"
            ));
        }
    }
    let mut played = Vec::new();
    for reference in required_setting("test_wave")?.split_whitespace() {
        if archetype_row(reference).is_none() {
            gaps.push(format!(
                "{SETTINGS_TABLE}: test_wave `{reference}` is no archetype of {waves_name}"
            ));
        } else if !unsimulated
            .iter()
            .any(|r| r.eq_ignore_ascii_case(reference))
            && !played
                .iter()
                .any(|r: &&str| r.eq_ignore_ascii_case(reference))
        {
            played.push(reference);
        }
    }
    let mut numbers = Vec::new();
    let mut repeats = 0;
    for row in waves.rows().filter(|row| cell(row, w::MARKER) == "1") {
        let Ok(number) = cell(row, w::NUMBER).parse::<u32>() else {
            gaps.push(format!(
                "{waves_name}: wave row `{}` has no wave number in c{}",
                cell(row, 0),
                w::NUMBER
            ));
            continue;
        };
        numbers.push(number);
        let wave = format!("{waves_name}: wave {number}");
        let count = |text: &str| text.parse::<u32>().ok().filter(|count| *count > 0);
        let mut refs = Vec::new();
        match (cell(row, w::SQUAD), cell(row, w::SQUAD_COUNT)) {
            ("", "") => {}
            (squad, squad_count) if count(squad_count).is_some() => refs.push(squad),
            (squad, squad_count) => gaps.push(format!(
                "{wave}: squad `{squad}` has no positive count: `{squad_count}`"
            )),
        }
        let specials: Vec<&str> = cell(row, w::SPECIALS).split_whitespace().collect();
        let special_counts: Vec<&str> = cell(row, w::SPECIAL_COUNTS).split_whitespace().collect();
        if specials.len() != special_counts.len()
            || special_counts.iter().any(|text| count(text).is_none())
        {
            gaps.push(format!(
                "{wave}: specials {specials:?} need one positive count each: {special_counts:?}"
            ));
        }
        refs.extend(specials);
        refs.extend(cell(row, w::BOSSES).split_whitespace());
        refs.extend(cell(row, w::VEHICLES).split_whitespace());
        for reference in refs {
            if archetype_row(reference).is_none() {
                gaps.push(format!("{wave}: `{reference}` is no archetype"));
            } else if !unsimulated
                .iter()
                .any(|r| r.eq_ignore_ascii_case(reference))
                && !played
                    .iter()
                    .any(|r: &&str| r.eq_ignore_ascii_case(reference))
            {
                played.push(reference);
            }
        }
        match cell(row, w::REPEAT) {
            "" => {}
            "1" => repeats += 1,
            other => gaps.push(format!(
                "{wave}: repeat mark `{other}` is neither empty nor 1"
            )),
        }
        let armory = cell(row, w::ARMORY);
        if !armory.is_empty() && !ARMORY_KINDS.contains(&armory) {
            gaps.push(format!(
                "{wave}: unlocks `{armory}`, no armory kind ({})",
                ARMORY_KINDS.join(", ")
            ));
        }
    }
    numbers.sort_unstable();
    if numbers.is_empty()
        || numbers
            .iter()
            .zip(1..)
            .any(|(number, want)| *number != want)
    {
        gaps.push(format!(
            "{waves_name}: waves are not numbered 1 to N: {numbers:?}"
        ));
    }
    if repeats == 0 {
        gaps.push(format!(
            "{waves_name}: no wave is marked in c{} to repeat past the last",
            w::REPEAT
        ));
    }
    for (reference, size) in squad_settings() {
        if !size.parse::<u32>().is_ok_and(|size| size > 0) {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `{SQUAD_SETTING_PREFIX}{reference}` is not a whole number of allies above zero: `{size}`"
            ));
        }
        if archetype_row(reference).is_none() {
            gaps.push(format!(
                "{SETTINGS_TABLE}: squad `{reference}` is no archetype of {waves_name}"
            ));
        } else if !played.iter().any(|r| r.eq_ignore_ascii_case(reference)) {
            played.push(reference);
        }
    }
    let weapon_of = |reference: &str| {
        aliases
            .rows()
            .find(|row| cell(row, 0).eq_ignore_ascii_case(reference))
            .and_then(|row| cell(row, 1).split_whitespace().next())
            .unwrap_or(reference)
            .to_owned()
    };
    for reference in played {
        let Some(row) = archetype_row(reference) else {
            continue;
        };
        let fill_in = archetypes
            .rows()
            .find(|fill| cell(fill, 0).eq_ignore_ascii_case(reference))
            .map_or("", |fill| cell(fill, 1));
        let weapon = match fill_in {
            "" => cell(row, archetype_column::WEAPON),
            weapon => weapon,
        };
        if weapon.is_empty() {
            gaps.push(format!(
                "{waves_name}: archetype `{reference}` has no weapon, in c{} or {ARCHETYPES_TABLE}",
                archetype_column::WEAPON
            ));
        }
        let offhand = cell(row, archetype_column::OFFHAND);
        for named in [weapon, offhand]
            .into_iter()
            .filter(|named| !named.is_empty())
        {
            let resolved = weapon_of(named);
            if !is_weapon(&resolved) {
                gaps.push(format!(
                    "{waves_name}: archetype `{reference}` -> `{named}` (`{resolved}`) is no weapon of the loaded zones"
                ));
            }
        }
        let health = cell(row, archetype_column::HEALTH);
        if !health.parse::<i32>().is_ok_and(|health| health > 0) {
            gaps.push(format!(
                "{waves_name}: archetype `{reference}` has no positive health in c{}: `{health}`",
                archetype_column::HEALTH
            ));
        }
        let reward = cell(row, archetype_column::REWARD);
        if reward.parse::<u32>().is_err() {
            gaps.push(format!(
                "{waves_name}: archetype `{reference}` pays no whole reward in c{}: `{reward}`",
                archetype_column::REWARD
            ));
        }
    }
    Ok(())
}

/// Holds the credits and the HUD to what the script adds and draws: whole
/// credit settings, a complete row for every hudelem it looks up, and for
/// every challenge it plays a row of MW3's table with a count and a reward.
fn check_economy(
    hud: &ScriptTable,
    challenge_kills: &ScriptTable,
    challenges_name: &str,
    challenges: &ScriptTable,
    gaps: &mut Vec<String>,
) -> Result<(), String> {
    for key in CREDIT_SETTINGS {
        let value = required_setting(key)?;
        if value.parse::<u32>().is_err() {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `{key}` is not a whole number of credits: `{value}`"
            ));
        }
    }
    for reference in HUD_REFS {
        match hud.rows().find(|row| cell(row, 0) == reference) {
            None => gaps.push(format!("{HUD_TABLE}: no `{reference}` row")),
            Some(row) => {
                let numbers_read = cell(row, 3).parse::<u32>().is_ok_and(|scale| scale > 0)
                    && cell(row, 8).parse::<i32>().is_ok()
                    && cell(row, 9).parse::<i32>().is_ok();
                if row.len() != HUD_COLUMNS
                    || (1..8).any(|column| cell(row, column).is_empty())
                    || !numbers_read
                {
                    gaps.push(format!(
                        "{HUD_TABLE}: `{reference}` needs a label, a font, a positive font scale in percent, four alignments and whole x and y: {row:?}"
                    ));
                }
            }
        }
    }
    for row in challenge_kills.rows() {
        let (reference, means) = (cell(row, 0), cell(row, 1));
        if means.is_empty() {
            gaps.push(format!(
                "{CHALLENGES_TABLE}: `{reference}` names no means of death"
            ));
        }
        let counted = |column| {
            challenges
                .rows()
                .find(|challenge| cell(challenge, challenge_column::REF) == reference)
                .is_some_and(|challenge| {
                    cell(challenge, column)
                        .parse::<u32>()
                        .is_ok_and(|count| count > 0)
                })
        };
        if !counted(challenge_column::REQUIRED) || !counted(challenge_column::REWARD) {
            gaps.push(format!(
                "{CHALLENGES_TABLE}: `{reference}` is no challenge of {challenges_name} with a positive count (c{}) and reward (c{})",
                challenge_column::REQUIRED,
                challenge_column::REWARD
            ));
        }
    }
    Ok(())
}

/// Holds last stand and the end of a run to what the script waits on and
/// shows: whole seconds, named shock, menu and hint, a value column of the
/// summary, and for every summary row a row number of the menu, a localized
/// label and a stat the script keeps.
fn check_endgame(eog: &ScriptTable, gaps: &mut Vec<String>) -> Result<(), String> {
    for key in SECONDS_SETTINGS {
        let value = required_setting(key)?;
        if !value.parse::<u32>().is_ok_and(|seconds| seconds > 0) {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `{key}` is not a whole number of seconds above zero: `{value}`"
            ));
        }
    }
    for key in NAME_SETTINGS {
        if required_setting(key)?.is_empty() {
            gaps.push(format!("{SETTINGS_TABLE}: `{key}` names nothing"));
        }
    }
    let column = required_setting("eog_value_column")?;
    if !column
        .parse::<u32>()
        .is_ok_and(|column| EOG_VALUE_COLUMNS.contains(&column))
    {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `eog_value_column` is not a value column of the summary ({}-{}): `{column}`",
            EOG_VALUE_COLUMNS.start(),
            EOG_VALUE_COLUMNS.end()
        ));
    }
    let mut seen = Vec::new();
    for row in eog.rows() {
        let (line, label, stat) = (cell(row, 0), cell(row, 1), cell(row, 2));
        match line.parse::<u32>() {
            Ok(line) if EOG_ROWS.contains(&line) && !seen.contains(&line) => seen.push(line),
            _ => gaps.push(format!(
                "{EOG_TABLE}: `{line}` is no unused summary row ({}-{})",
                EOG_ROWS.start(),
                EOG_ROWS.end()
            )),
        }
        if label.len() < 2 || !label.starts_with('@') {
            gaps.push(format!(
                "{EOG_TABLE}: row {line} label `{label}` is no `@` localize reference"
            ));
        }
        if !EOG_STATS.contains(&stat) {
            gaps.push(format!(
                "{EOG_TABLE}: row {line} stat `{stat}` is none the script keeps ({})",
                EOG_STATS.join(", ")
            ));
        }
    }
    Ok(())
}

/// The armory squads: every `squad_<archetype>` settings row, its archetype
/// and its size as written.
fn squad_settings() -> Vec<(&'static str, &'static str)> {
    SETTINGS
        .lines()
        .filter_map(|line| line.split_once(','))
        .filter_map(|(key, size)| {
            key.trim()
                .strip_prefix(SQUAD_SETTING_PREFIX)
                .map(|reference| (reference, size.trim()))
        })
        .collect()
}

/// Holds air support to what the script reads: whole numbers above zero, an
/// action slot, named materials and models, and bombs that are weapons of the
/// loaded zones. Models are not checked, as for armories and bodies.
fn check_airsupport(
    airsupport: &ScriptTable,
    is_weapon: &impl Fn(&str) -> bool,
    gaps: &mut Vec<String>,
) {
    let value = |key: &str| {
        airsupport
            .rows()
            .find(|row| cell(row, 0) == key)
            .map(|row| cell(row, 1))
    };
    for key in AIRSUPPORT_NUMBERS {
        match value(key) {
            Some(number) if number.parse::<u32>().is_ok_and(|n| n > 0) => {}
            other => gaps.push(format!(
                "{AIRSUPPORT_TABLE}: `{key}` is not a whole number above zero: {other:?}"
            )),
        }
    }
    if let Some(slot) = value("action_slot").and_then(|slot| slot.parse::<u32>().ok())
        && !ACTION_SLOTS.contains(&slot)
    {
        gaps.push(format!(
            "{AIRSUPPORT_TABLE}: `action_slot` {slot} is no action slot ({}-{})",
            ACTION_SLOTS.start(),
            ACTION_SLOTS.end()
        ));
    }
    for key in AIRSUPPORT_NAMES {
        if value(key).is_none_or(str::is_empty) {
            gaps.push(format!("{AIRSUPPORT_TABLE}: `{key}` names nothing"));
        }
    }
    for key in AIRSUPPORT_WEAPONS {
        match value(key) {
            Some(weapon) if is_weapon(weapon) => {}
            other => gaps.push(format!(
                "{AIRSUPPORT_TABLE}: `{key}` is no weapon of the loaded zones: {other:?}"
            )),
        }
    }
}

/// Holds the heavy archetypes to what the script reads for the traits
/// `archetypes` gives: for each trait in use its rows of `heavies`, whole
/// numbers above zero, weapons of the loaded zones, means of death the engine
/// knows and named effects and shocks.
fn check_heavies(
    heavies: &ScriptTable,
    archetypes: &ScriptTable,
    aliases: &ScriptTable,
    is_weapon: &impl Fn(&str) -> bool,
    gaps: &mut Vec<String>,
) {
    let value = |key: &str| {
        heavies
            .rows()
            .find(|row| cell(row, 0) == key)
            .map(|row| cell(row, 1))
    };
    let in_use = |name: &str| {
        archetypes
            .rows()
            .any(|row| cell(row, fill_in_column::TRAIT) == name)
    };
    for (_, keys) in TRAITS.iter().filter(|(name, _)| in_use(name)) {
        for key in keys.iter().copied() {
            let Some(text) = value(key).filter(|text| !text.is_empty()) else {
                gaps.push(format!("{HEAVIES_TABLE}: `{key}` names nothing"));
                continue;
            };
            if HEAVIES_WEAPONS.contains(&key) {
                let weapon = aliases
                    .rows()
                    .find(|row| cell(row, 0).eq_ignore_ascii_case(text))
                    .and_then(|row| cell(row, 1).split_whitespace().next())
                    .unwrap_or(text);
                if !is_weapon(weapon) {
                    gaps.push(format!(
                        "{HEAVIES_TABLE}: `{key}` -> `{text}` (`{weapon}`) is no weapon of the loaded zones"
                    ));
                }
            } else if HEAVIES_MEANS.contains(&key) {
                for means in text.split_whitespace() {
                    if !weapon_iw4::MEANS_OF_DEATH.contains(&means) {
                        gaps.push(format!(
                            "{HEAVIES_TABLE}: `{key}` -> `{means}` is no means of death"
                        ));
                    }
                }
            } else if !HEAVIES_NAMES.contains(&key)
                && !text.parse::<u32>().is_ok_and(|number| number > 0)
            {
                gaps.push(format!(
                    "{HEAVIES_TABLE}: `{key}` is not a whole number above zero: `{text}`"
                ));
            }
        }
    }
}

/// Holds `GRANTS_TABLE` to what the script grants: every ref an item of the
/// armory table, granted one way only (no squad), by a grant the script
/// plays, a grant that hands a weapon with a weapon of the loaded zones; a
/// predator with its missile, a sentry with a model, a counted grant with a
/// most in the armory table and a whole step, an explosive with an action
/// slot of its own. Every `WEAPON_KIND` item it does not list must be a
/// weapon of the loaded zones.
fn check_grants(
    grants: &ScriptTable,
    aliases: &ScriptTable,
    airsupport: &ScriptTable,
    armory_name: &str,
    armory: &ScriptTable,
    is_weapon: &impl Fn(&str) -> bool,
    gaps: &mut Vec<String>,
) {
    let squads = squad_settings();
    let airsupport_slot = airsupport
        .rows()
        .find(|row| cell(row, 0) == "action_slot")
        .map_or("", |row| cell(row, 1));
    let mut explosive_slots: Vec<u32> = Vec::new();
    for row in grants.rows() {
        let (reference, how) = (cell(row, 0), cell(row, 1));
        if !armory.rows().any(|item| cell(item, 1) == reference) {
            gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` is no item of {armory_name}"
            ));
        }
        if squads.iter().any(|(squad, _)| *squad == reference) {
            gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` is also a squad (`{SQUAD_SETTING_PREFIX}{reference}`)"
            ));
        }
        if !grant::ALL.contains(&how) {
            gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` grant `{how}` is none the script plays ({})",
                grant::ALL.join(", ")
            ));
        }
        let weapons: Vec<&str> = aliases
            .rows()
            .find(|alias| cell(alias, 0) == reference)
            .map_or_else(
                || vec![reference],
                |alias| cell(alias, 1).split_whitespace().collect(),
            );
        let hands_weapon = grant::WEAPONS.contains(&how);
        for weapon in weapons
            .iter()
            .filter(|weapon| hands_weapon && !is_weapon(weapon))
        {
            gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` -> `{weapon}` is no weapon of the loaded zones"
            ));
        }
        if how == grant::PREDATOR && weapons.len() < 2 {
            gaps.push(format!(
                "{GRANTS_TABLE}: predator `{reference}` names no missile after its detonator in {WEAPON_ALIASES_TABLE}"
            ));
        }
        if how == grant::SENTRY && cell(row, grant::MODEL).is_empty() {
            gaps.push(format!(
                "{GRANTS_TABLE}: sentry `{reference}` names no model in c{}",
                grant::MODEL
            ));
        }
        let amount = cell(row, grant::AMOUNT);
        if !amount.is_empty()
            && !(grant::COUNTED.contains(&how) && amount.parse::<u32>().is_ok_and(|n| n > 0))
        {
            gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` c{} `{amount}` is not a whole count above zero of a counted grant ({})",
                grant::AMOUNT,
                grant::COUNTED.join(", ")
            ));
        }
        if grant::COUNTED.contains(&how) {
            let most = armory
                .rows()
                .find(|item| cell(item, 1) == reference)
                .map(|item| cell(item, ARMORY_MOST_COLUMN));
            if !most.is_some_and(|most| most.parse::<u32>().is_ok_and(|n| n > 0)) {
                gaps.push(format!(
                    "{GRANTS_TABLE}: `{reference}` is counted, but {armory_name} c{ARMORY_MOST_COLUMN} gives no most above zero: {most:?}"
                ));
            }
        }
        let slot = cell(row, grant::SLOT);
        match (how == grant::EXPLOSIVE, slot.parse::<u32>()) {
            (true, Ok(slot)) if ACTION_SLOTS.contains(&slot) => {
                if slot.to_string() == airsupport_slot {
                    gaps.push(format!(
                        "{GRANTS_TABLE}: explosive `{reference}` action slot {slot} is {AIRSUPPORT_TABLE}'s `action_slot`"
                    ));
                }
                if explosive_slots.contains(&slot) {
                    gaps.push(format!(
                        "{GRANTS_TABLE}: explosive `{reference}` action slot {slot} holds another explosive"
                    ));
                }
                explosive_slots.push(slot);
            }
            (true, _) => gaps.push(format!(
                "{GRANTS_TABLE}: explosive `{reference}` c{} `{slot}` is no action slot ({}-{})",
                grant::SLOT,
                ACTION_SLOTS.start(),
                ACTION_SLOTS.end()
            )),
            (false, _) if !slot.is_empty() => gaps.push(format!(
                "{GRANTS_TABLE}: `{reference}` grant `{how}` waits on no action slot, c{} `{slot}`",
                grant::SLOT
            )),
            (false, _) => {}
        }
    }
    for item in armory.rows() {
        let reference = cell(item, 1);
        if cell(item, ARMORY_KIND_COLUMN) != WEAPON_KIND
            || grants.rows().any(|row| cell(row, 0) == reference)
        {
            continue;
        }
        let weapons: Vec<&str> = aliases
            .rows()
            .find(|alias| cell(alias, 0) == reference)
            .map_or_else(
                || vec![reference],
                |alias| cell(alias, 1).split_whitespace().collect(),
            );
        if weapons.first().is_none_or(|weapon| !is_weapon(weapon)) {
            gaps.push(format!(
                "{armory_name}: {WEAPON_KIND} item `{reference}` has no {GRANTS_TABLE} row and is no weapon of the loaded zones (through {WEAPON_ALIASES_TABLE})"
            ));
        }
    }
}

/// Holds the weapons' slots to what the script reads: a whole count of
/// primary slots above zero, and replacement warnings only for grants that
/// take one.
fn check_weapon_slots(gaps: &mut Vec<String>) -> Result<(), String> {
    let slots = required_setting("primary_slots")?;
    if !slots.parse::<u32>().is_ok_and(|slots| slots > 0) {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `primary_slots` is not a whole count above zero: `{slots}`"
        ));
    }
    for how in required_setting("armory_replacement_grants")?.split_whitespace() {
        if !grant::PRIMARY.contains(&how) {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `armory_replacement_grants` `{how}` is no grant that takes a primary slot ({})",
                grant::PRIMARY.join(", ")
            ));
        }
    }
    Ok(())
}

/// Holds armor and the armory menus' answers to what the script reads: a
/// whole percent and named prefixes and answer.
fn check_armory_settings(gaps: &mut Vec<String>) -> Result<(), String> {
    for key in PERCENT_SETTINGS {
        let value = required_setting(key)?;
        if !value.parse::<u32>().is_ok_and(|percent| percent <= 100) {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `{key}` is not a whole percent from 0 to 100: `{value}`"
            ));
        }
    }
    for key in ARMORY_MENU_SETTINGS {
        if required_setting(key)?.is_empty() {
            gaps.push(format!("{SETTINGS_TABLE}: `{key}` names nothing"));
        }
    }
    Ok(())
}

/// Holds `PLAYER_DATA_TABLE` to what the script and the menus read: named
/// fields, none twice, the plain numbers the script writes by name, and for
/// a keyed field armory kinds of the armory table, each keying one field.
fn check_player_data(
    fields: &ScriptTable,
    armory_name: &str,
    armory: &ScriptTable,
    gaps: &mut Vec<String>,
) {
    let mut names: Vec<&str> = Vec::new();
    let mut kinds: Vec<&str> = Vec::new();
    for row in fields.rows() {
        let name = cell(row, 0);
        if name.is_empty() {
            gaps.push(format!("{PLAYER_DATA_TABLE}: a row names no field"));
            continue;
        }
        if names.iter().any(|seen| seen.eq_ignore_ascii_case(name)) {
            gaps.push(format!("{PLAYER_DATA_TABLE}: `{name}` is declared twice"));
        }
        names.push(name);
        let keyed: Vec<&str> = cell(row, 1).split_whitespace().collect();
        let scalar = PLAYER_DATA_SCALARS
            .iter()
            .any(|scalar| scalar.eq_ignore_ascii_case(name));
        if scalar && !keyed.is_empty() {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: `{name}` is a plain number the script writes, not keyed by {keyed:?}"
            ));
        }
        for kind in keyed {
            if kinds.contains(&kind) {
                gaps.push(format!(
                    "{PLAYER_DATA_TABLE}: armory kind `{kind}` keys two fields"
                ));
            }
            kinds.push(kind);
            if !armory
                .rows()
                .any(|item| cell(item, ARMORY_KIND_COLUMN) == kind)
            {
                gaps.push(format!(
                    "{PLAYER_DATA_TABLE}: `{name}` is keyed by `{kind}`, no item kind (c{ARMORY_KIND_COLUMN}) of {armory_name}"
                ));
            }
        }
    }
    for scalar in PLAYER_DATA_SCALARS {
        if !names.iter().any(|name| name.eq_ignore_ascii_case(scalar)) {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: no `{scalar}` field, which the script writes"
            ));
        }
    }
}

/// Holds the rank table to what the script reads from it (a last rank, and
/// for every rank below it the experience that ends it) and `start_rank` to
/// `max` or a rank of it with the experience it starts at.
fn check_ranks(rank_name: &str, ranks: &ScriptTable, gaps: &mut Vec<String>) -> Result<(), String> {
    use rank_column as r;
    let rank_row = |rank: &str| ranks.rows().find(|row| cell(row, 0) == rank);
    let Some(max) = rank_row(r::MAX_ROW).and_then(|row| cell(row, r::MAX).parse::<u32>().ok())
    else {
        gaps.push(format!(
            "{rank_name}: no `{}` row with a last rank in c{}",
            r::MAX_ROW,
            r::MAX
        ));
        return Ok(());
    };
    for rank in 0..max {
        if !rank_row(&rank.to_string()).is_some_and(|row| cell(row, r::END).parse::<u32>().is_ok())
        {
            gaps.push(format!(
                "{rank_name}: rank {rank} has no experience that ends it in c{}",
                r::END
            ));
        }
    }
    let start = required_setting("start_rank")?;
    let start_rank = match start {
        "max" => Some(max),
        rank => rank.parse::<u32>().ok().filter(|rank| *rank <= max),
    };
    let starts = start_rank
        .and_then(|rank| rank_row(&rank.to_string()))
        .is_some_and(|row| cell(row, r::START).parse::<u32>().is_ok());
    if !starts {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `start_rank` `{start}` is neither `max` nor a rank 0 to {max} of {rank_name} with the experience it starts at in c{}",
            r::START
        ));
    }
    Ok(())
}

fn cell(row: &[String], column: usize) -> &str {
    row.get(column).map_or("", String::as_str)
}

fn map_table_name(zone: &str) -> String {
    format!("survival/maps/{zone}.csv")
}

pub(crate) fn insert_survival_scripts(scripts: &mut crate::ScriptSources) {
    for (module, source) in MODULES {
        scripts.insert_source(module, source.to_owned());
    }
    scripts.insert_source(
        &format!(
            "maps/mp/gametypes/{}",
            gamemode_iw4::survival::GAMETYPE_TOKEN
        ),
        GAMETYPE.to_owned(),
    );
    scripts.insert_table(SETTINGS_TABLE, SETTINGS);
    for (name, csv) in TABLES {
        scripts.insert_table(name, csv);
    }
    for (zone, csv) in MAP_TABLES {
        scripts.insert_table(&map_table_name(zone), csv);
    }
}
