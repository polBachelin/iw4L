# Run and poke the live game

How to touch the live process. Observation is [`PERF.md`](PERF.md).
Two players through the dev master: `make approved SCENARIO=master_duo_chaos`
([scenario and setup](../crates/approved_tests/README.md#two-clients-through-the-dev-master)).

```bash
set -a; . ./.env; set +a          # IW4L_GAMES; DISPLAY=:0 if the session has none
make map mp_boneyard CMDS='spawn 0; wait 2s; quit'
cargo run --profile play -p launcher -- map iw5:mp_overwatch --cmds '…'
cargo run --profile play -p launcher -- survival --cmds '…'
```

`survival [<zone>]` needs only MW3 data: an IW5 runtime, no multiplayer frontend
or master, the map from `crates/assets/src/survival/settings.csv` by default.
`survival --headless [<zone>]` is the same listen host with no window, for a
machine without a display (a locked macOS screen never draws the window). The
local player still joins and spawns; `wait world` waits for the installed
world instead of a drawn scene. With no swapchain to pace it, each headless
frame waits for its own GPU work. `serve` is always headless. Survival plays
the local player on `human_team` and every bot on `enemy_team` (both rows of
`settings.csv`), so `bot add` adds enemies beside the wave's own (clients 1–7
of wave 1). A repeatable clear of wave 1 on mp_paris: each of the seven
enemies is brought in front of the player and shot three times, then the
intermission runs into wave 2. Read it back from the log (`survival: enemy
killed client=N archetype=easy remaining=R` seven times, each followed by
`credits client=0 credits=C delta=100 reason=kill:easy` and `bots: retired
client N`, then `wave=1 complete`, the `reason=wave_bonus:1` credits,
`intermission 30s`, `armory unlocked weapon` and `wave=2 start squad=easy
x9`):

```bash
shoot='hold +attack; wait 0.3s; release +attack; wait 0.4s;'
cmds='wait world; wait spawn; god; wait 10s; bot hold on; tp -1214 2048 229 270 0;'
for k in 1 2 3 4 5 6 7; do
  cmds="$cmds bot tp $k -1214 1990 229 90; wait 0.5s; $shoot $shoot $shoot wait 1s;"
  [ $k = 4 ] && cmds="$cmds hold +reload; wait 0.2s; release +reload; wait 3s;"
done
target/play/iw4l survival --headless --cmds "$cmds wait 40s; quit"
```

Last stand needs damage, so its recipe holds the bots instead of `god`
(which blocks damage): go down, shoot one enemy up close to get up, then go
down with no self-revive left and die, which ends the run on MW3's summary
(`survival: laststand`, `revived by kill`, `player died`, `game over`, then
`menu open: sp_eog_summary`). Down, the eye is near the ground, and a level
aim at an enemy standing 58 units off misses most shots; the recipe looks
20° up (`tp … 270 -20`) before it shoots:

```bash
shoot='hold +attack; wait 0.3s; release +attack; wait 0.4s;'
cmds='wait world; wait spawn; wait 5s; bot hold on; tp -1214 2048 229 270 0; wait 1s; damage 200; wait 2s; tp -1214 2048 229 270 -20; wait 0.5s;'
cmds="$cmds bot tp 1 -1214 1990 229 90; wait 0.5s; $shoot $shoot $shoot $shoot $shoot wait 2s; damage 200; wait 5s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

The end of a wave gets a downed player up without a kill: shoot six of wave
1's seven enemies, go down, then drop the seventh from 600 units so the fall
kills it (`enemy killed client=7 … means=MOD_FALLING by=world remaining=0`,
`wave=1 complete`, `revived by wave_complete wave=1 selfrevive=1`, then the
wave bonus and no `bled out`):

```bash
shoot='hold +attack; wait 0.3s; release +attack; wait 0.4s;'
cmds='wait world; wait spawn; wait 5s; bot hold on; tp -1214 2048 229 270 0; wait 1s;'
for k in 1 2 3 4 5 6; do
  cmds="$cmds bot tp $k -1214 1990 229 90; wait 0.5s; $shoot $shoot $shoot wait 1s;"
  [ $k = 4 ] && cmds="$cmds hold +reload; wait 0.2s; release +reload; wait 3s;"
done
target/play/iw4l survival --headless --cmds "$cmds damage 200; wait 2s; bot tp 7 -1214 1900 829 90; wait 40s; quit"
```

Down mid-wave with no kill still bleeds out (`laststand … bleedout=30`, `bled
out client=0 after 30s`, `player died`, `game over`):

```bash
target/play/iw4l survival --headless --cmds 'wait world; wait spawn; wait 5s; bot hold on; tp -1214 2048 229 270 0; wait 1s; damage 200; wait 40s; quit'
```

The pause menu stops a solo game (`host pause: paused tick=T menu=iw4l_pause`,
then `host pause: resumed tick=T paused_tick=T after=3.0s` with the same
tick, and the next pause 3 s later on a tick 60 further on). Console `wait`s
count wall time while the game is paused, so `wait 3s` still ends:

```bash
target/play/iw4l survival --headless --cmds 'wait world; wait spawn; wait 5s; togglemenu; wait 3s; togglemenu; wait 3s; togglemenu; wait 1s; togglemenu; wait 2s; quit'
```

A bleed-out waits for the pause: go down, pause 10 s, then 25 s later a
marker pause is 26 s of game time after going down and nothing has bled out;
`bled out client=0 after 30s` comes after it:

```bash
target/play/iw4l survival --headless --cmds 'wait world; wait spawn; wait 5s; bot hold on; tp -1214 2048 229 270 0; wait 1s; damage 200; wait 1s; togglemenu; wait 10s; togglemenu; wait 25s; togglemenu; wait 0.2s; togglemenu; wait 10s; quit'
```

The survival screens, windowed (the screen must be unlocked and awake): the
HUD with no multiplayer scorebar bottom-left, the pause menu, last stand,
then MW3's `sp_eog_summary` after the second down:

```bash
shoot='hold +attack; wait 0.3s; release +attack; wait 0.4s;'
cmds='wait world; wait spawn; wait 5s; screenshot sv_hud; wait 1s; togglemenu; wait 2s; screenshot sv_pause; wait 1s; togglemenu; wait 1s; bot hold on; tp -1214 2048 229 270 0; wait 1s; damage 200; wait 3s; screenshot sv_down; wait 1s; tp -1214 2048 229 270 -20; wait 0.5s;'
cmds="$cmds bot tp 1 -1214 1990 229 90; wait 0.5s; $shoot $shoot $shoot $shoot $shoot wait 2s; damage 200; wait 5s; screenshot sv_eog; wait 1s; quit"
target/play/iw4l survival --cmds "$cmds"
```

Air support needs the armories wave 2 (equipment) and wave 3 (air support)
unlock, and credits. A purchase is made at the armory: standing in front of
it (on mp_paris the air support laptop at `649 1334 18`, the equipment case at
`-825 -57 99`, as `survival: armory <kind> spawned at` logs them), `+activate`
opens its menu (`survival: armory <kind> opened`), `menuresponse <menu>
<response>` sends the purchase MW3's armory menus send (`purchase_<c0>` of
`sp/survival_armories.csv`), and `menukey escape` closes it. Clearing
waves 1–3 pays 3250 credits, one airstrike. The recipe buys the airstrike,
the predator, the Delta squad and both sentries, 15000 credits at MW3's
prices (2500 + 2500 + 3000 + 4000 + 3000), so it needs `start_credits` set
to `15000` in `crates/assets/src/survival/settings.csv` and a rebuild, then
both set back after (`make approved SCENARIO=survival_paris` fails while the
binary's start credits differ from the file's). It clears waves 1–3 with an
LMG. On wave 4 it puts the enemies with odd client ids (`bot tp` skips an id
that is no bot) under the airstrike, called on the map selector's default
point (the map's centre), and the rest in the predator's lane, where the
predator is fired unsteered. In the intermission it buys the Delta squad
(clients 1–3), places the grenade-launcher sentry in front of the player and
the minigun sentry beside the equipment case. On wave 5 (clients 4–15) it
puts four enemies in front of each sentry and three by the squad
(`survival: airstrike at (…) kills=K`, `predator impact at (…) kills=K`,
`allies requested 3`, three `ally connected … team=allies`, two `sentry
placed`, `sentry kill client=0 … weapon=sentry_gun_so` with
`means=MOD_PROJECTILE_SPLASH` on the kill, `sentry kill client=0 …
weapon=sentry_minigun_so`, and `enemy killed … weapon=iw5_m16_mp
by=client<1–3>`). A placement that does not fit logs `sentry does not fit at
(…)` and waits for the next `+attack`, so the minigun sentry tries two yaws:

```bash
burst='bot tp all -1214 1990 229 90; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s;'
cmds='wait world; wait spawn; god; wait 8s; bot hold on; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 270 0; wait 1s;'
air='tp 609 1334 18 0 0; wait 0.5s; press +activate; wait 1s;'
equipment='tp -825 -17 99 270 0; wait 0.5s; press +activate; wait 1s;'
back='menukey escape; wait 0.5s; tp -1214 2048 229 270 0; wait 1s;'
A='menuresponse survival_armory_airsupport'; E='menuresponse survival_armory_equipment'
tps() { local where=$1; shift; for k in "$@"; do printf 'bot tp %s %s; ' "$k" "$where"; done; }
for w in 1 2 3; do
  for k in 1 2 3 4 5 6; do cmds="$cmds $burst"; [ $((k % 3)) = 0 ] && cmds="$cmds press +reload; wait 3s;"; done
  cmds="$cmds wait 34s;"
done
cmds="$cmds $air $A purchase_10001; wait 0.5s; $back press +actionslot 4; wait 3s;"
cmds="$cmds bot tp all -1214 -110 60 90; $(tps '-533 810 1300 0' $(seq 1 2 31)) wait 0.5s; press +attack; wait 8s;"
cmds="$cmds $air $A purchase_10000; wait 0.5s; $back press +actionslot 4; wait 14s;"
cmds="$cmds $air $A purchase_10002; wait 0.5s; $back"
cmds="$cmds $equipment $E purchase_1008; wait 0.5s; $back press +attack; wait 1s;"
cmds="$cmds $equipment $E purchase_1007; wait 0.5s; menukey escape; wait 0.5s;"
cmds="$cmds tp -825 -17 99 180 0 &; wait 1s; press +attack; wait 1s; tp -825 -17 99 90 0 &; wait 1s; press +attack; wait 1s;"
cmds="$cmds tp -1214 2048 229 270 0 &; wait 15s;"
cmds="$cmds $(tps '-1214 1880 229 90' 4 5 6 7) $(tps '-1214 -110 60 0' 8 9 10 11 12) wait 10s;"
cmds="$cmds $(tps '963 2161 0 0' 1 2 3) $(tps '1163 2161 0 180' 13 14 15) wait 1s; bot hold off; wait 40s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

The armory menus buy through their own items too. Opening each one logs
its gated items as the menu reads them from the match player data
(`menu survival_armory_<kind>: item I '<name>' enabled|disabled`, the name
in backquotes in the log: with about
3250 credits after wave 3, the equipment armory's 4000-credit self-revive and
grenade-launcher sentry and the air support armory's 5000-credit riot squad
are disabled). On the air support menu, `menukey down` twice then
`menukey enter` buys the predator through the item's own `scriptmenuresponse`
(`credits … delta=-2500 reason=purchase:remote_missile`, `bought
remote_missile`, `playerdata client=0 armoryairsupport remote_missile=1`),
and the next `menukey down` shows the items the 750 credits left no longer
buy turning `disabled`. Setting `start_rank` to `5` in `settings.csv`
(and back after) gates by rank instead: wave 1 is enough to see the weapon
armory's LMG and sniper categories (c7 11 and 6) and the pistols above rank
5 disabled.

```bash
burst='bot tp all -1214 1990 229 90; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s;'
cmds='wait world; wait spawn; god; wait 8s; bot hold on; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 270 0; wait 1s;'
for w in 1 2 3; do
  for k in 1 2 3 4 5 6; do cmds="$cmds $burst"; [ $((k % 3)) = 0 ] && cmds="$cmds press +reload; wait 3s;"; done
  cmds="$cmds wait 34s;"
done
cmds="$cmds tp 357 33 35 180 0; wait 0.5s; press +activate; wait 1.5s; menukey escape; wait 1s;"
cmds="$cmds tp -825 -17 99 270 0; wait 0.5s; press +activate; wait 1.5s; menukey escape; wait 1s;"
cmds="$cmds tp 609 1334 18 0 0; wait 0.5s; press +activate; wait 1.5s; menukey down; wait 0.5s; menukey down; wait 0.5s;"
cmds="$cmds menukey enter; wait 1.5s; menukey down; wait 0.5s; menukey escape; wait 1s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

Going down in an armory's submenu closes the whole armory. The recipe clears
wave 1, opens the weapon armory's Handguns category (`menukey down` twice,
`menukey enter`: `menu open: survival_armory_weapon_pistol`, `menu` shows
both on the stack), then takes damage 200 (`armory weapon closed client=0
reason=downed`, `laststand …`, `menu close: survival_armory_weapon_pistol`,
`menu close: survival_armory_weapon`); `menu` then shows `stack=[]`:

```bash
burst='bot tp all -1214 1990 229 90; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s;'
cmds='wait world; wait spawn; god; wait 8s; bot hold on; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 270 0; wait 1s;'
for k in 1 2 3 4 5 6; do cmds="$cmds $burst"; [ $((k % 3)) = 0 ] && cmds="$cmds press +reload; wait 3s;"; done
cmds="$cmds wait 5s; tp 357 33 35 180 0; wait 0.5s; press +activate; wait 1.5s; menukey down; wait 0.5s; menukey down; wait 0.5s; menukey enter; wait 1.5s; menu;"
cmds="$cmds god; wait 0.5s; damage 200; wait 2s; menu; wait 1s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

The kit purchases need the weapon armory (after wave 1) and the equipment
armory (after wave 2), and more credits than two waves pay: raise
`start_credits` in `crates/assets/src/survival/settings.csv` (30000 buys
everything below) and set it back after. The recipe clears waves 1–2 with
the LMG, then at the weapon case (`357 33 35`) refills ammo, buys the MP5
into the free slot, is refused it again and buys the USP in the MP5's place
(`granted ammo … refilled iw5_pecheneg_mp=100+300`, `granted iw5_mp5_mp
grant=weapon`, `holds 1 of 1`, `replaced iw5_mp5_mp with iw5_usp45_mp`). At
the equipment case it throws its frag and a flash and takes a hit, then buys
both grenades back (the frag twice: `holds 1, all frag_grenade_mp holds`),
claymores, C4 and armor (`granted claymore grant=explosive held=5 of 10 …
action_slot=1`, `granted armor … armor=100`), is refused the self-revive it
still holds, and plants a claymore from action slot 1 (no `survival:` line
marks the plant; the claymore's clip drops from 5 to 4). It then goes down,
gets up on a wave 3 kill, buys the self-revive back and the RPG, whose
replacement warning it accepts with the warning's focused first item
(`purchase rpg_survival asks … replaces <weapon>`, `replacement answered
continue`, `granted rpg_survival grant=launcher held=2 of 4 … replaced=…`,
then `held=4 of 4`):

```bash
burst='bot tp all -1214 1990 229 90; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s;'
shoot='hold +attack; wait 0.3s; release +attack; wait 0.4s;'
cmds='wait world; wait spawn; god; wait 8s; bot hold on; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 270 0; wait 1s;'
for w in 1 2; do
  for k in 1 2 3 4 5 6; do cmds="$cmds $burst"; [ $((k % 3)) = 0 ] && cmds="$cmds press +reload; wait 3s;"; done
  cmds="$cmds wait 34s;"
done
W='menuresponse survival_armory_weapon'; E='menuresponse survival_armory_equipment'
equipment='tp -825 -17 99 270 0; wait 0.5s; press +activate; wait 1s;'
cmds="$cmds tp 357 33 35 180 0; wait 0.5s; press +activate; wait 1s; $W purchase_0; wait 0.5s; $W purchase_11; wait 0.5s; $W purchase_11; wait 0.5s; $W purchase_2; wait 0.5s; menukey escape; wait 1s;"
cmds="$cmds tp -825 -17 99 270 0; wait 0.5s; press +frag; wait 2s; press +smoke; wait 2s; god; wait 0.5s; damage 40; wait 1s; god; wait 0.5s; press +activate; wait 1s;"
for item in 1000 1000 1001 1002 1003 1005 1006; do cmds="$cmds $E purchase_$item; wait 0.5s;"; done
cmds="$cmds menukey escape; wait 1s; press +actionslot 1; wait 2s; press +attack; wait 3s;"
cmds="$cmds tp -1214 2048 229 270 0; wait 1s; god; wait 0.5s; damage 200; wait 2s; tp -1214 2048 229 270 -20; wait 0.5s;"
for r in 1 2; do cmds="$cmds bot tp all -1214 1990 229 90; wait 0.5s; $shoot $shoot $shoot $shoot $shoot $shoot $shoot $shoot $shoot $shoot press +reload; wait 3s;"; done
cmds="$cmds god; wait 0.5s; $equipment $E purchase_1006; wait 0.5s; $E purchase_1004; wait 1s; menukey enter; wait 1.5s; $E purchase_1004; wait 0.5s; menukey escape; wait 1s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

The heavy archetypes are spread over waves 7 to 19, and `claymore` is in no
wave of MW3's table, so their recipe sets `test_wave` in `settings.csv` to
`jug_regular jug_riotshield jug_explosive martyrdom claymore chemical` (wave 1
is then one of each) and `claymore_plant_range` in `heavies.csv` to `150` (so
the claymore is planted in front of the player and trips), rebuilds, and sets
both back after. It brings every enemy in front of the player and shoots
(`enemy connected … archetype=<a> … health=H` for each, `martyrdom c4
detonated`, `claymore planted`, `claymore tripped`, `claymore disarmed`,
`chemical gas at (…)`, `gas shock client=0 shock=radiation_high`); a player
RPG or frag on `jug_explosive` alone logs `explosive weakness … scaled=S`:

```bash
burst='bot tp all -1214 1990 229 90; wait 0.5s; hold +attack; wait 1.5s; release +attack; wait 0.5s;'
cmds='wait world; wait spawn; god; wait 8s; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 270 0; wait 1s;'
for r in 1 2 3 4 5 6 7 8; do cmds="$cmds $burst $burst $burst press +reload; wait 3s;"; done
target/play/iw4l survival --headless --cmds "$cmds wait 15s; quit"
```

The chopper boss is in waves 6, 15 and 21; its recipe sets `test_wave` to
`chopper chopper` (wave 6's c8) and sets it back after. Standing at
`-1214 2048 229`, both choppers close on the player along mp_paris's chopper
path; the first holds over node `pf3900_21` at `(-979.1, 1699.3, 1192)`,
which a view of yaw 304, pitch -65 hits, and the second takes it once the
first is down (`chopper spawned path=… health=1500` twice, `chopper fired at
client=0`, then `chopper destroyed reward=600 by=client0` twice and `wave=1
complete killed=2`). Without `god` the player goes down to it (`laststand …
means=MOD_RIFLE_BULLET by=chopper`):

```bash
burst='hold +attack; wait 5s; release +attack; wait 0.3s; press +reload; wait 4s; give ammo; wait 0.5s;'
cmds='wait world; wait spawn; god; give weapon/iw5:pecheneg; wait 3s; tp -1214 2048 229 304 -65; wait 25s; hold +speed_throw; wait 1s;'
cmds="$cmds $burst $burst $burst $burst wait 15s; $burst $burst $burst $burst wait 10s;"
target/play/iw4l survival --headless --cmds "$cmds quit"
```

`make` passes no foreign flags through: in the `Makefile` it is `CMDS`, on the
binary `--cmds`. The colon is a GNU make pattern, so `make map iw5:…` fails —
write `make map ZONE=iw5:mp_overwatch` or use `cargo run`. Recipes: `make
scenario`, `chaos`, `bench` ([`BENCH.md`](BENCH.md)), `bench-live`, `lifecycle-*`
(`*_CMDS` in the `Makefile`). Live recipes use `[profile.play]`; LTO is `PROFILE=release`.

Linux startup creates `iw4`, `iw5`, `t5` and `t6` symlinks in `IW4L_GAMES` for missing games with exactly one valid Steam installation. Existing entries are preserved.

**Cheats are on by default.** The host accepts the debug
actions: `move`, `look`, `tp`, `nudge`, `god`, `kill`, `damage`, `force_spawn`,
`force_match_start`, `rotatevelocity` and the `give` supply commands. `--no-cheats`
turns them off for `map`, `serve` and `menu`; a lobby host toggles them in the
game setup.

**GSC controls the start freeze.** `freezecontrols` constrains authority movement;
snapshots carry the same constraints into client prediction and command replay.
Wait for the loaded mode's countdown, or use the debug command:
`wait world; spawn 0; force_match_start; wait 1s; …`.
Firing, hit and decal probes must start after the countdown releases the player.

**Sync by default.** Ritual `wait`s are unnecessary: a command holds the FIFO
until its own fact — `map`/`demo`/`play` until the swap plus GPU-ready
`WorldScene::spawned`, `spawn` until `AppScreen::InGame`, `move`/`look`/`tp`
until a matching presented pose, `force_match_start` until `Playing`,
`disconnect` until the hold is torn down. `wait Ns` remains, for pacing and
observation windows. The `&` suffix is a loud refusal to wait (`map mp_rust &`),
`!` jumps the queue (in bash write `disconnect !`, or history expansion bites).
The 120 s gate timeout aborts the rest with an echo; parsing is
`crates/console/src/`. Hand-typed `dump`/`clip`/`screenshot` pass through any
wait while preserving the queue, which inside `--cmds` takes an explicit `!`.
A hand-typed `quit`/`exit`/`disconnect` goes further: it jumps **every** hold,
`wait 60s` included, and releases it. A `quit` written into `--cmds` does not —
it is part of the script and waits its turn.

## Verbs and traps

`map spawn class give attach name god kill damage move tp look nudge press hold
release bind bot wait mark record stoprecord clip demo dump screenshot ui menuresponse
disconnect quit finish_run`
plus the debug `force_match_start` / `showpos`. `quit` leaves now and abandons
whatever screenshot was queued or half-written; `finish_run` is the scripted
ending that waits for those files first. `disconnect` leaves the session, not
just the world — it also leaves the room, or closes it when hosting, and works
with no map installed. `bot` is
`add | dummy | hold | tp | give | fire`; `dump [name]` writes the current snapshot into
`dumps/`. `clip` writes the last available 45 s into `clips/<ULID>/` (demo + dump)
on host and clients — the host records authority, a client the snapshots it
received plus its own presented state; `demo LATEST` plays it back.

* `god` toggles invincibility for a live player; repeat it to turn protection off. It blocks damage while cheats are enabled; `kill` still forces death.
* ADS is `hold +speed_throw`, not `+speed`;
* Hold Shift (`+breath_sprint`) or bind `+holdbreath` to steady eligible sights
  at full ADS. Breath lasts 4.5 seconds; releasing or exhausting it requires recovery.
* Aim binding: `bind MOUSE2 +speed_throw` aims only while held;
  `bind MOUSE2 +toggleads_throw` toggles aim on each press. Both are available
  under Options → Controls → Actions, and the chosen bind is saved in settings.
* bolt action (`fire_type=1`): `hold +attack` is **one** shot, and a full
  magazine will not reload itself (`press +attack` ×N, then `press +reload`);
* `look` without `LookState` only writes `ps.viewangles` — no aiming;
* Use (`+activate` or `+usereload`) retrieves your settled C4, claymores and
  deployable gadgets when there is room in their equipment ammo slot.
* `give` searches one completion list: `give ammo`, `give killstreak/uav`, or `give weapon/iw5:msr [attachment...]`. Search by any part of the name, then accept the suggested item.
* `give killstreak/care_package` and `give killstreak/pave_low` acquire rewards without activating them. Available familiar names appear alongside script names in autocomplete.
* `give ammo` refills carried reserves and equipment; reload magazines normally. Supply commands require a live player and a host allowing debug actions.
* `bot` hints follow the subcommand: counts, on/off, current bot IDs, weapons, and `tp … above`.
* custom classes live in `iw4l-artifacts/profile/classes.txt` (one tab-separated
  row per class); `spawn 0` selects the first slot. Delete the file to generate
  five available classes again. Random presets use a pool of 20: five each for
  IW4, IW5, T5 and T6, with IW4 equipment. Players and bots select only
  presets supported by the loaded weapons, attachments and equipment.

In the class editor, choose Camouflage for the primary or secondary weapon.
MW3 weapons use their installed camo names and previews, including Gold, Marine
and Winter where models are available. The selection is saved with the class and
applies to first-person and world weapons. Console equip also accepts
`give weapon/iw5:acr reflex camo=gold`.

Callsign on the main menu selects a title and emblem. Killstreaks selects three
rewards with different kill requirements; Apply saves the selection for the next
loadout. Both persist in `iw4l-artifacts/profile/barracks.txt`. Titles, emblems and
rewards are available without progression requirements.

Video settings include brightness (50–150%, neutral 100%) and FOV (65–120°)
sliders and a saved First person / Third person camera selector. Use
`thirdperson` to toggle, `thirdperson 0|1` to select, or `set cg_thirdPerson 0|1`.
Death, killcam and remote missile cameras take priority. Drag with the mouse
or use Left/Right on the focused slider; the value
appears to its right. Multiplayer settings contain the player-name field
(Enter to edit and accept, Escape to cancel). These settings persist across
launches in `$XDG_CONFIG_HOME/iw4l/settings.cfg` or `~/.config/iw4l/settings.cfg`
on Linux; `IW4L_SETTINGS_PATH` selects a separate settings file for probes.
Listen hosts also persist the three script completion percentages in `profile.cfg`
beside that file. `IW4L_PROFILE_PATH` overrides the completion-profile path.

`IW4L_MUTE=1` silences the speakers only: sounds still load, mix and report
peaks and voice stats. `IW4L_SOUND=off` instead skips audio entirely.

Listen hosts and clients store a signing key and derived account ID in
`account.dat` beside `settings.cfg` (`account_iw5.dat` under the MW3 runtime
profile, since a record only imports under its own game's schema);
`IW4L_ACCOUNT_PATH` overrides that path for every profile.
The file can also hold schema-stamped player-data snapshots. Listen imports them
before match scripts start and saves authority changes. New Listen buffers receive
the authored schema stamp, one localized name per schema `customClasses` slot and
captured stat defaults before level entry scripts start. Player-data methods read and write the bound schema buffer;
external scripts block persistent-data writes. IW4 remote admission exchanges a
signed profile before gameplay; host updates save locally before acknowledgement.
Installed multiplayer validation and schema migration remain incomplete.
Older account files upgrade while retaining stats;
conflicting disk revisions refuse save instead of overwriting another writer.
