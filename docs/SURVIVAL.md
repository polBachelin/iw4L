# Survival

`iw4l survival [--headless] [<zone>]` plays `GameModeKind::Survival` (token
`survival`) under the IW5 runtime: no score or time limit, players on
`human_team`, waves on `enemy_team`. The gametype is ours
(`crates/assets/src/survival/survival.gsc.txt`). It reads MW3's own survival
tables from the user's zones (`mode_zones` in `settings.csv`, later zones winning)
and our files only for what those tables lack.

## MW3 tables (read at runtime, never copied)

| setting | table | layout |
|---|---|---|
| `waves_table` | `sp/survival_waves.csv` | c1 = `1`: a wave row (c2 number, c3 squad archetype, c4 count, c5 special archetypes and c6 their counts, space separated and parallel, c7 bosses and c8 vehicles, one each, c9 `1` on the repeat block (our reading), c10 the armory the wave's end unlocks); c0 in 100–999: an archetype (c1 ref, c5 weapon, c6 offhand, c7 health, c8 aim skill (our reading), c9 the credits a player's kill pays); c1 = `weapon_N`, `grenade_N`, `equipment_N`, `armor_1`: start loadout (c2 ref, c3 count or `max`) |
| `armory_table` | `sp/survival_armories.csv` | c2 in `weapon scope main equipment airsupport`: an item (c1 ref, c3 price, c7 rank, c10 max count) |
| `drop_table` | `sp/survival_chaos_drop_locations.csv` | c0 key, c2 `x, y, z`: candidate positions for placements |
| `challenge_table` | `sp/survival_challenge.csv` | c6 a positive count: a challenge (c1 ref, c6 the kills it asks for, c7 its reward) |

Refs are table names, not always weapons: `laststand` is a self-revive,
`armor` is armor points and `ammo` a refill; the rest resolve through `weapon_aliases.csv`, or name
a weapon themselves.

## Our files (`crates/assets/src/survival/`, no header rows)

* `settings.csv` — `key,value`: zones, table names, teams, `friendly_fire`
  (`0`: a client's damage to another client of its team is refused, its own
  still hurts it; `1`: all damage counts), spawn classnames,
  `unsimulated_archetypes` (archetypes no bot can play: the waves skip them),
  `test_wave` (a development aid, empty in the tree: archetype refs that, when
  set, make wave 1 one enemy of each instead of the table's),
  health, `start_credits`, `wave_bonus` (the credits each player gets when a
  wave is complete), `rank_table` (MW3's Special Ops rank table: it must name
  the table the menus' `levelForExperienceSO` reads, `sp/ranktable.csv`, or
  the menus and the purchase rules gate by different ranks) and `start_rank` (the rank a player starts a
  run at, `max` for the table's last: their experience is the one it starts
  at, its c2), intermission and bleedout seconds,
  `armor_absorb_percent` (the share of a hit armor takes while it lasts, 0 to
  100; 100, the whole hit, is our reading), the
  revive's shellshock and its seconds (`getup_shock`, `getup_shock_seconds`),
  the summary menu, its value column and death hint (`eog_menu`,
  `eog_value_column`, `eog_deadquote`), `squad_<archetype>` (an armory
  squad: the archetype of the waves table that is also an air support ref of
  the armory, and how many allies it brings), the armory menus' name prefix,
  purchase answer prefix and close answer (`armory_menu_prefix`,
  `armory_purchase_prefix`, `armory_close_response`: MW3's menus send `quit`
  as they close), MW3's warning that a purchase will replace a weapon, its
  answer to go ahead and the grants it is shown for
  (`armory_replacement_menu`, `armory_replacement_continue`,
  `armory_replacement_grants`: `survival_armory_replacement_warning`,
  `continue`, `launcher`), `primary_slots` (how many primaries a player
  carries, 2), pause menu, `player_viewhands` (the players' first-person
  hands, `viewhands_player_delta` of `common_survival`: used ahead of the
  map kit's hands while the loaded zones hold the model, the kit's otherwise). Credits, bonus, start rank, timings, squad sizes
  and primary slots are provisional values: MW3's data has no starting
  credits, no wave-end bonus rule, no starting rank (SP kept the player's),
  no bleed-out or shock time, no squad size, no slot count. That only the
  RPG warns before it replaces a weapon, and a weapon purchase replaces one
  without a warning, is our reading of MW3.
* `playerdata.csv` — `field,kinds`: the player data fields MW3's armory menus
  read with `getPlayerData`, as a dump of their expressions lists them:
  `surHUD_credits` and `experience` (plain numbers), and `armory<kind>` keyed
  by an item's ref, c1 naming the armory table kinds (c2) whose items key it
  (`armoryweaponupgrade` is keyed by the `scope` and `main` rows). An armory
  item is a row whose kind one of these fields lists.
* `airsupport.csv` — `key,value`, all ours (MW3's survival script is compiled
  and the numbers lived there): `action_slot` (1–4, where air support items
  wait); the airstrike's location selector material, the weapon its kills are
  credited to (`airstrike_bomb`), its burst effect, means of death, radius and
  max/min damage, bomb count, spacing, the release height above the ground
  (also the trace range that finds the ground), the interval between bursts,
  the jet model, its flight length and time; `support_settle_ms` (how long
  after a strike or missile its kills are counted); the predator's launch
  height and offset.
* `grants.csv` — `ref,grant,model,amount,slot`: how an armory item is given,
  for the items no other data decides (a squad is a `squad_<archetype>`
  setting, a perk a row of `perks_table` enabled in its c6, any other item
  of the `weapon` armory the weapon its ref names). `airstrike` and
  `predator` wait on `action_slot` and are used as the airstrike's map
  selector or a steered missile; `sentry` is carried and placed, with the
  model in c2; `ammo` refills the weapons; `grenade`, `explosive` and
  `launcher` are counted in their weapon's rounds, `armor` in armor points,
  `laststand` in self-revives. For a counted grant c3 is how many one
  purchase adds, empty to fill up to all it can hold; an `explosive` waits
  on the action slot in c4. The counts are ours: claymore and C4 add 5
  (slots 1 and 2), the RPG 2, a self-revive 1; grenades and armor fill up.
* `weapon_aliases.csv` — `ref,weapon [weapon...][,missile [missile...]]`: a
  table ref mapped to the weapon it is given as, each cell a list of
  candidates, SP weapon names first and today's MP name last. When the match
  is installed every cell is narrowed to its first candidate the loaded zones
  carry under that exact script name (an `_mp` spelling does not stand in
  for the SP one), and the install log says `survival weapons:` with the picks;
  a cell with none refuses the match. `remote_missile` names the detonator
  the player holds in c1 and the missile it launches in c2. The SP names
  (`fraggrenade`, `flash_grenade`, `rpg_survival`) are not yet confirmed
  against the `IW5 weapons captured:` debug line of the zones `mode_zones`
  loads; until they are, a wrong one falls back to the MP name, which the
  `survival weapons:` line shows. The juggernaut guns (`m60jugg`,
  `riotshieldjugg`) name the MP weapon alone.
* `archetypes.csv` — `ref,weapon,body,trait,speed`: fill-ins for archetype
  cells MW3's table leaves empty or names in a form we cannot use (SP actor
  classnames), and what sets an archetype apart. The weapon resolves through
  `weapon_aliases.csv` like any ref; an empty one keeps the table's. The
  body is read only as a chopper's vehicle model (see Heavy
  archetypes). The trait is one of
  `explosive`, `martyrdom`, `claymore`, `gas`, `chopper` or empty; the speed is the
  bot's move speed in percent (`setmovespeedscale`), empty for 100. Traits
  and speeds are ours: MW3's table has only the archetype descriptions they
  follow (`Guys strapped with C4 that explode on death`, `Guys that place
  claymores`, `Chemical warfare enemies in gas masks`, `Uses and is weak to
  explosives`).
* `heavies.csv` — `key,value`, all ours (MW3's numbers lived in its compiled
  script): `explosive_means` (the means of death an `explosive` archetype is
  weak to) and `explosive_damage_percent`; the martyr's weapon ref, drop
  height and delay (`martyrdom_*`); the claymore's weapon ref, how many one
  enemy plants, the range to a player it plants at, the plant height and
  reach, arming time, trip radius, arc in degrees and delay, and the poll
  interval (`claymore_*`); the gas cloud's effect, means of death, radius,
  damage per interval, duration and interval, and its shocks by depth
  (`gas_shock_high` within `gas_high_radius`, `gas_shock_med` within
  `gas_med_radius`, `gas_shock_low` to `gas_radius`) for `gas_shock_ms`;
  the chopper's vehicle definition (`chopper_vehicle`), the map keys of
  MW3's chopper path (`chopper_path_noteworthy`, `chopper_start_targetname`),
  its death effect, speed and acceleration in mph, the time between a wave's
  choppers entering, how long it holds at a node, its gun's range, the aim
  height above a player's feet, burst and rest times, and the spread in units
  per 1000 of range (`chopper_*`). Durations are in milliseconds.
* `hud.csv` — `ref,label,font,fontscale,horzalign,vertalign,alignx,aligny,x,y`:
  the hudelems of the survival HUD, refs `wave`, `enemies` and `credits`. The
  label is plain text with `&&1` where the number goes; the font scale is in
  percent.
* `challenges.csv` — `ref,means`: the challenges of `challenge_table` a kill
  advances, and the means of death that counts (`MOD_HEAD_SHOT`, `MOD_MELEE`).
  Which kill counts is our reading; the challenges with no row (streak,
  stagger, quadkill, flash) are not played.
* `eog.csv` — `row,label,stat`: the rows of the end-of-run summary. The row
  is the summary menu's (1–9), the label a localize reference (`@…`), the
  stat one the script keeps: `wave`, `time`, `credits`, `kills`,
  `headshots`, `credits_earned`, `downs`. Which stats MW3 showed, and in
  which order, is not in the data; these are ours.
* `maps/<zone>.csv` — `kind,origin,yaw,model`: armory placements; kind
  `weapon|equipment|airsupport`, origin `x y z` or `drop <drop_table key>`,
  whole units. Each is spawned as a `script_model` of its model, made usable
  once its kind unlocks. A map without a file has no armories, and the log
  says so. The positions are ours: MW3's own lived in a zone the user's data
  lacks.

## Enemies

The script cannot create clients (`addtestclient` is refused). It asks the
session for them with `survival_requestenemies(count, archetype)` or
`survival_requestallies(count, archetype)`; an archetype that names no row of
the waves table, or a count below 1, faults the script. The host's bot roster
mints the clients, binds each to its archetype and joins it on `enemy_team`
(enemies) or `human_team` (allies) with no class pick. On connect
`self survival_archetype()` returns that archetype (undefined for a player); the
script opens the team menu, and on the session's answer spawns the bot with the
archetype's health (c7) and weapon (`archetypes.csv`, else c5). An enemy that
dies is kicked, never respawned, and the roster retires its client.

A wave enemy always knows where the players are, as MW3's survival AI does:
its Hunt goes through the navigation graph to the nearest live client of
`human_team`, and roams only while none is alive. It still has to see a
player to fight. Its aim is the default bot's scaled by the archetype's c8
(`BotTuning::scaled`, [`BOTS.md`](BOTS.md)): 0.9 for `easy` up to 1.3 for
`elite`. That c8 is an aim skill is our reading of the table, not an
established fact: it is the one column that rises with the tiers. A c8 that is
not a positive number faults the request. c11 is not read. 

## Heavy archetypes

Every archetype plays its row of the waves table: health (c7), weapon and
offhand, reward. What sets the heavy ones apart is its `archetypes.csv` trait
and speed:

* the juggernauts (`jug_*`, health 1000 or 2000 in c7) move at their speed
  (75%). `jug_riotshield` holds `iw5_riotshieldjugg_mp`: a bot with a shield
  in hand closes to melee reach and bashes with the melee button
  ([`BOTS.md`](BOTS.md)), and keeps the shield rather than reloading.
* `explosive` (`jug_explosive`): damage whose means is in `explosive_means`
  is scaled by `explosive_damage_percent` before it is dealt.
* `martyrdom`: where it dies it drops its `martyrdom_weapon` (MW3's
  survival `c4`) with `magicbullet`, and detonates it `martyrdom_delay_ms`
  later: the C4's own blast, in the martyr's name. The martyr stays
  connected, dead, until then, and leaves `support_settle_ms` after.
* `claymore`: each time a live player is within `claymore_plant_range` it
  plants a `claymore_weapon` (MW3's survival `claymore`) at its feet facing
  that player, up to `claymore_count`. Armed after `claymore_arm_ms`, the
  claymore goes off `claymore_trip_delay_ms` after a live player is within
  `claymore_trip_radius` and `claymore_trip_arc` of the way it faces, as the
  weapon's own blast. A claymore is disarmed when its planter leaves (a
  killed enemy is kicked at once): its blast would be in nobody's name and
  spare no team, so killing the planter defuses its claymores.
* `gas` (`chemical`): where it dies its tanks burst: `gas_fx` (MW3's
  `smoke/so_chemical_explode_smoke`) plays once, and for `gas_ms` the cloud
  deals `gas_damage` as `radiusdamage` within `gas_radius` every
  `gas_interval_ms`, in the chemical enemy's name, and shellshocks every live
  player in it with MW3's `radiation_high`, `radiation_med` or `radiation_low`
  by depth. It stays connected, dead, until the cloud clears.

With `friendly_fire` 0 an enemy's C4, claymore and gas spare the other
enemies and hurt the players, and the players' explosives spare each other.
* `chopper` (MW3's `Enemy Attack Chopper`, row 112: health 1500, reward
  600): no bot. It is a vehicle (`spawnvehicle`) of its `archetypes.csv` body
  (`vehicle_little_bird_armed`) with no owner, on `enemy_team`, with its c7
  health and its `archetypes.csv` gun (`minigun_littlebird`, both of
  common_survival). It flies MW3's own chopper path on the map, read at
  runtime: on mp_paris the 28 entities whose script_noteworthy is
  `so_chopper_boss_path_struct`, linked by script_linkto naming the next
  ones' script_linkname (24 nodes, 47 links, 3 links to names no entity
  has), entered from the 4 whose targetname is `chopper_boss_path_start`.
  A wave's choppers enter `chopper_spawn_interval_ms` apart, each from the
  next start. At each node it holds `chopper_dwell_ms`, then flies to the
  linked node nearest the nearest live player (in the map plane), or holds
  again when its own is nearer; a node one chopper flies to is closed to the
  others. It fires bursts with `fireweapon` at the nearest live player within
  `chopper_range` its gun can see (`vehicle_canturrettargetpoint`), each shot
  off the aim point by up to `chopper_spread`. Its hits are in its own name:
  a turret's or vehicle's bullet that hits a client passes that entity as
  eInflictor, and as eAttacker when it has no owner, so friendly fire spares
  the other enemies. A player who destroys it is paid its c9 like any kill,
  and its death is a kill of the wave; it is then deleted after its
  `chopper_death_fx`. Its speeds, timings, range and spread are ours; the
  vehicle definition is passed by name only, since the IW5 lane captures no
  vehicle definitions (no turret, no acceleration from it). A map with no
  such path skips the chopper like an unsimulated archetype, and says so at
  start.

Bodies are not set: a bot is drawn with the team's soldier kit, the one body
per side the map's kits give. `archetypes.csv`'s body column (MW3's
`fullbody_juggernaut_*`, `fullbody_chemwar_*`) needs a body per client, which
the renderer does not have. Dogs stay out (no dog body or movement).

## Waves

The first wave starts once a player has connected. A wave's enemies are its
squad (c3 × c4), its specials (each c5 ref × its c6 count), then one of each
boss (c7) and vehicle (c8), in that order. An archetype in
`unsimulated_archetypes` is left out of the wave and logged, never dropped
silently: dogs have no bot body (out of scope). A chopper (c8 in MW3's
table: two on waves 6 and 15, three on 21) takes no client slot: it counts
toward the wave's total and its enemies left, and the wave ends only once it
is destroyed.

The wave's budget is `sv_maxclients` minus every client it did not ask for:
the players (any client without an archetype, a console `bot add` bot
included) and the allies. The script asks for enemies, one request per run
of one archetype, until the enemies in play fill the budget, and asks for more
each time one leaves. An enemy is in play from its request until its client
is gone: the session frees a slot only once the kick has dropped the client,
so a refill sent on the kill itself could find no slot. A killed enemy is
kicked and never comes back; an enemy that leaves without a kill is asked for
again. A wave is complete when every enemy is killed and none is in play.

Every enemy spawns on the `enemy_spawn_classname` point farthest from the
nearest player that no live client stands on (`positionwouldtelefrag`), and
waits while every point is taken. It is given its archetype's weapon and its
offhand (c6, through `weapon_aliases.csv`; an offhand-class weapon becomes its
primary offhand) and its health (c7).

Between waves the intermission lasts `intermission_seconds`; at its start the
armory the wave names in c10 unlocks (weapon after wave 1, equipment after 2,
air support after 3; `level.armoryunlocked[kind]`, `armory_unlocked` notify).
That c10 unlocks at the end of its wave is our reading. Past the last row the
waves whose c9 is `1` repeat in order (waves 16–21 in MW3's table, so wave 22
plays 16, wave 28 plays 16 again): our reading of c9, which MW3 sets on those
waves only. Wave numbers keep counting.

## Credits and HUD

A player starts with `start_credits`. Each enemy a player kills pays its
archetype's c9, and advances every played challenge whose means of death the
kill has; a challenge that reaches its c6 pays its c7 and starts over (our
reading: whether MW3 counts in a row, per wave or once is not in the data).
When a wave is complete every player gets `wave_bonus`. A player's score is
their credits, so the scoreboard shows them.

The script sets the client dvars `surHUD_credits`, `surHUD_wave` and
`surHUD_enemy` (the wave and the enemies it has left, its total less its
kills), but no menu of the loaded zones reads them. MW3's survival HUD menu is
not in the data: `surHUD_display` has no items, it only stamps the animations
of a HUD that lived in a menu of SP's zones. So client hudelems laid out by
`hud.csv` draw the wave, the enemies left and the credits; the launch logs the
gap (``survival menus: `surhud_display` draws nothing``).

The multiplayer scorebar (`scorebar_hd`, a match score against the score
limit) is not drawn: survival has no such score, and MW3 runs it on SP's HUD,
whose menu list has no scorebar. The mode kind says which modes have one
(`GameModeKind::has_scorebar`).

## Last stand and game over

A self-revive is what lets a player go down instead of dying. The start
loadout's `equipment_N` rows give them (`laststand`, one in MW3's table),
the equipment armory sells one back once it is spent, and while a player holds one the script sets `specialty_pistoldeath`, so
lethal damage puts them in last stand (`PM_TYPE_LAST_STAND`, 1 health)
rather than killing them, the item MW3 describes in
`SO_SURVIVAL_EQUIPMENT_LASTSTAND_DESC`. A downed player switches to a pistol of their
primaries, loses their offhands and bleeds out after `bleedout_seconds`.
Killing an enemy while down gets them up: the revive spends one self-revive,
restores full health, the weapon they went down with and the offhands, and
plays the `getup_shock` shellshock for `getup_shock_seconds` (MW3's PC
`laststand_getup_pc` of patch_survival). The end of a wave gets every
downed player up the same way without spending a self-revive (our call, not
read from MW3's data), so a player down when the wave's last enemy dies to
something other than them does not bleed out in the intermission. With none
left the perk is unset, so the next lethal hit kills. Enemies keep shooting a
downed player, and lethal damage while down kills too.

Armor is points the start loadout's `armor_1` row gives (100 in MW3's
table), which the equipment armory fills back up. While a player has any, each hit takes `armor_absorb_percent` of its
damage off the armor, as much as is left, and the rest reaches health; the hit
still lands at what is left of it. Armor is spent before last stand is
decided, so a hit is lethal only once it is through the armor.

The run ends when no player is left alive (solo: the player's death). The
waves stop (`game_over`) and every player gets MW3's `sp_eog_summary` menu
(`eog_menu`) through these client dvars: `so_survival 1`,
`ui_mission_success 1` and `ui_eog_success_heading_player1 <wave>` (the
menu's own title then reads `@SO_SURVIVAL_SURVIVED_TO_WAVE`), `ui_hide_hint
0` with `ui_bled_out 1` after a bleed-out or `ui_deadquote` (`eog_deadquote`,
MW3's tip to buy a self-revive) otherwise, and `eog.csv`'s rows as
`ui_eog_r<row>c1_player1` (the label) and `ui_eog_r<row>c<eog_value_column>_player1`
(the value). The menu's c2 to c4 are value columns, right-aligned from the
right edge in that order; c4 alone for a single player is our reading. Its
buttons run MW3's engine commands `fast_restart` (play again) and
`disconnect_noMessage` (return, and escape), aliases of `map_restart` and
`disconnect`. `stopControllerRumble` on open is no console verb: the menu
bridge logs it once.

## Pause

The script sets `g_scriptMainMenu` to `pause_menu` (`iw4l_pause`, our menu),
so escape opens it. In a solo game it stops the match: while it is the menu
taking input on a listen host no remote client is connected to (bots do not
count), the host pauses the game clock (`Time<Virtual>`), which stops the
authority ticks, script waits (bleed-out, intermission, spawns), bots and the
listen client's frame clock together. Closing it resumes on the tick it
stopped on, with no catch-up. A pause is the host's call: the HUD only asks
(`net::HostPauseRequest`), the host refuses while a remote client is
connected and ends a pause one joins during. Which modes ask is
`GameModeKind::main_menu_pauses` (Survival only), so the multiplayer modes'
menus never stop a match. Solo stops and co-op does not: our call, not read
from MW3's data. Console and menu `wait`s count wall time while the
game is paused.

## Armory menus

MW3's `survival_armory_*` menus read the player through `getPlayerData`, not
dvars: `getPlayerData( "surHUD_credits" )` for the credits,
`levelForExperienceSO( getPlayerData( "experience" ) )` against the item's c7
for the rank gate, and `getPlayerData( "armory" + kind, ref )` for an item
the player already holds. These are fields of SP player data, which no
structured data definition in the user's zones has (`mp/playerdata.def` has
none). Survival keeps them in a match player data store of its own: one per
client, in the client's match state and its snapshot meta (beside its client
dvars), never written to disk, empty on connect and on a map restart that
keeps no `pers`. `playerdata.csv` declares its fields; the script writes them
with `self survival_setplayerdata( field, [ref,] value )`, which declares the
store on the first write and faults on a field it does not declare, a key on
a plain field or none on a keyed one, or a value that is not an int. Under
survival the menus' `getPlayerData` reads this store (a keyed field reads 0
for a ref with no entry); it never reads `mp/playerdata.def`.

The script writes `experience` at connect (`start_rank`'s c2 in
`rank_table`), `surHUD_credits` at every change of the credits, and
`armory<kind>` + ref whenever the value changes: for every item of an
armory's kind as the player opens it and after each purchase there (what a
player holds of a weapon, grenade or armor follows their inventory, which
firing, throwing and taking hits change), and for a squad for every player
as its allies are asked for or leave. The armory value is the count the
player holds once it is the most they can hold, 0 below that: the menus
disable an item whose value is above 0, and a purchase is refused at that
most, so the two agree on what can be bought. The most is the item's c10,
or for an item counted in a weapon's rounds what the weapon holds when that
is less. The ammo refill (c10 0) reads 1 while no weapon it refills is
short. That MW3 kept this count is our reading; its menus test only for a
value above 0. The
purchase rules use the same rank as the menus: `levelForExperienceSO` over
`rank_table` (the first rank whose c7 is above the experience, else the
`maxrank` row's c1), so with `start_rank` `max` no item is rank-gated.

What the menus send for an enabled item is `purchase_<c0>` (the item's row
index, `purchase_1` for c0 1, `iw5_fnfiveseven_mp`) through
`scriptmenuresponse localVarString( "purchase_item" )`, `weaponswitch_<c0>`
through `upgrade_item`, and `quit` on close. MW3's
`survival_armory_replacement_warning` opens over the armory's menu and
answers `continue` (its first item, focused on open) or `cancel` (its
second, and escape), then closes itself; the armory's menu stays open
beneath it. Its layout calls `getwrappedtextheight`, which the menu runtime
has no body for (`expression failed FunctionBodyMissing`): the warning
works, its text box is not sized. Special Ops rank comes from
`sp/ranktable.csv` of `code_post_gfx_mp` (c0 the rank, c2 the experience it
starts at, c7 the experience that ends it, the `maxrank` row's c1 the last
rank).

## Purchases and air support

An armory stands where its placement puts it, unusable until the wave that
unlocks its kind is over. A player who uses it (`+activate` within the
engine's use reach) is held in place and gets its menu
(`armory_menu_prefix` + kind); the use ends when the menu answers
`armory_close_response` as it closes, or when the player goes down or dies
(the menu is closed with every submenu it opened over itself: a weapon
category, the replacement warning). MW3's armory model, use hint (`SO_SURVIVAL_ARMORY_USE_*`)
and world icon are not in the user's zones: the placement's model stands in,
with no hint text.

The script answers the armory menus' purchase: a `menuresponse` from a menu
named `armory_menu_prefix…` whose answer is `armory_purchase_prefix<c0>`
(`purchase_10001` buys c0 10001, `precision_airstrike`). The menus send it
when an enabled item is activated; the console's `menuresponse <menu>
<response>` sends the same answer. A purchase is refused, and logged with
the reason, when the player is using no armory or one of another kind than
the item's c2, the item's armory has not unlocked, the player's rank is
below its c7, the player has fewer credits than its c3, this build grants no
such item, already holds all of it they can (a squad counts once while any
of its allies is in play or owed; a weapon once while carried), the ammo
refill has no weapon to refill, the action slot holds another air support
item, or a sentry is still being carried. A purchase whose grant is in
`armory_replacement_grants`, of a weapon the player does not hold, with
every primary slot taken, first opens `armory_replacement_menu`
(`purchase <ref> asks … replaces <weapon>`): `continue` buys it, as above,
anything else buys nothing.
Otherwise its price is spent (`reason=purchase:<ref>`) and it is granted as
`grants.csv`, a squad setting or the perks table says:

* `airstrike` (`precision_airstrike`), `predator` (`remote_missile`): the item's weapon goes on
  `action_slot`. Switching to it uses it. The airstrike opens MW3's map
  selector with a direction; backing out keeps the item. On confirm a jet
  (`airstrike_plane`) crosses the point and `airstrike_bombs` bursts land
  along the chosen yaw, each `radiusdamage` with the `airstrike_bomb`
  weapon's name and `playfx`. The predator launches `remote_missile_survival`
  from above and behind the player, aimed ahead, and links the player's
  controls to it until it explodes. MW3's own MP strikes deal radius damage
  in a weapon's name the same way; the counts, ranges and damage are ours.
* `sentry` (`sentry`, `sentry_gl`): a turret of the item's weapon (`sentry_minigun_so`,
  `sentry_gun_so`) with its `grants.csv` model, carried in front of the player and placed
  with `+attack` where `canplayerplacesentry` fits it. Placed, it is in
  `sentry` mode on the player's team: the turret host picks the nearest
  visible hostile and the script fires it at the weapon's fire time. Its kills
  are the player's. It is never destroyed and never expires, so c10 caps how
  many a player ever places. `shootturret` fires by the weapon's type:
  `sentry_minigun_so` (bullet) traces, `sentry_gun_so` (projectile) launches
  its grenade, so a grenade-launcher kill is `MOD_PROJECTILE_SPLASH`.
* `friendly_support_delta`, `friendly_support_riotshield`: `squad_<ref>`
  allies of that archetype (rows 115/116: health 500, `iw5_m16_mp` or
  `iw5_mp5_mp`, `fraggrenade`). They take client slots like enemies: the
  players and allies (connected, asked for or owed) come off the wave's
  budget, owed allies are asked for first as slots free up, and an ally spawns
  on the player spawn nearest the players. Enemy bots hunt them as they hunt
  players (both are on `human_team`). No body is set: the bot keeps the
  default player body, and `archetypes.csv`'s body column is not read yet. The
  riot squad has no riot shield (c5 names the MP5).
* a perk of `perks_table` enabled in its c6 (`specialty_quickdraw`,
  `specialty_bulletaccuracy`, `specialty_stalker`, `specialty_longersprint`,
  `specialty_fastreload`): `setperk`. A perk with no engine behavior is
  logged by the engine (``perk `…`: no engine behavior``).

* a weapon of the `weapon` armory (its ref, `iw5_usp45_mp`): given with a
  full clip and stock and switched to. With `primary_slots` primaries carried
  it takes the place of the one in hand when that is a primary, else of the
  first (`replaced <weapon> with <weapon>`).
* `ammo`: every primary short of a full clip or stock, its alternate mode's
  too, is filled; a launcher's weapon keeps its own ammo.
* `grenade` (`fraggrenade`, `flash_grenade`): the grenade's count is filled
  up. The MP frag the alias names holds 1 and the flash 2, where MW3's table
  allows 4 (and its start loadout 3, logged as `x1 of 3`): what a weapon
  holds caps the count.
* `explosive` (`claymore`, `c4`, MW3's survival weapons of that name, held as
  items): 5 more, up to 10, on action slot 1 or 2; `+actionslot` switches to
  it and `+attack` plants or throws it.
* `launcher` (`rpg_survival`): a primary, taking a slot as a weapon does
  (after the warning); 2 rounds more, up to 4, the clip first.
* `armor`: armor points back up to its c10 (100). MW3's own amount lived in
  its compiled script; the table's c10 is our reading.
* `laststand`: one more self-revive, up to 1, so it is bought once the start
  one is spent.

The weapon upgrades (the weapon armory's `scope` and `main` rows, which MW3
fitted to the weapon in hand) are not granted: a purchase of one is refused,
and the upgrade menu's `weaponswitch_<c0>` is not answered.

Every client's `team` field is set with its `sessionteam`: the turret host
and radius damage tell sides apart by it.

## What refuses the match

Before the program installs, a survival match is refused (`gsc: refused …
stage=install`) when an alias or archetype weapon is no weapon of the loaded
zones, an archetype names no row of the waves table, or a placement's kind,
origin, yaw or model is unreadable. The waves are held to the same: numbered
1 to N with at least one c9 repeat row, every c3/c5/c7/c8 ref an archetype,
c4 and each c6 count positive and c6 parallel to c5, c10 empty or an armory
kind, every `unsimulated_archetypes` ref an archetype, and every archetype a
bot plays given a weapon and offhand of the loaded zones and a positive
health and a whole reward. `start_credits` and `wave_bonus` must be whole
credits, `hud.csv` must lay out `wave`, `enemies` and `credits` in full, and
every `challenges.csv` ref must be a challenge of `challenge_table` with a
positive count and reward. `intermission_seconds`, `bleedout_seconds` and
`getup_shock_seconds` must be whole seconds above zero, `friendly_fire` 0 or
1, every `test_wave` ref an archetype (held to what a bot plays like any
wave's), every `archetypes.csv` trait one of the five and speed a whole
percent above zero, and for each trait in use its `heavies.csv` rows: weapon
refs that are weapons of the loaded zones (through `weapon_aliases.csv`),
means of death the engine knows, a named effect, shocks, vehicle and map
keys, and whole numbers above zero, `getup_shock`,
`eog_menu` and `eog_deadquote` must name something, `eog_value_column` must
be 2 to 4, and every `eog.csv` row an unused summary row 1–9 with an `@`
label and a stat the script keeps. Every `squad_<archetype>` row must name an
archetype of the waves table with a whole size above zero, and that archetype
is held to what a bot plays like any wave's. `airsupport.csv`'s numbers must
be whole and above zero, `action_slot` 1–4, its selector, effect, means and
plane must name something, and `airstrike_bomb` must be a weapon of the
loaded zones. Every `grants.csv` ref must be an item of `armory_table` and no
squad, its grant one the script plays, for a grant that hands a weapon its
weapons (through `weapon_aliases.csv`) weapons of the loaded zones, a
`predator` must name its missile after its detonator and a `sentry` a
model; a counted grant needs a c10 above zero in `armory_table` and a c3
empty or a whole count above zero (no other grant may set c3), an
`explosive` an action slot 1–4 in c4 that is not `action_slot` and no other
explosive's (no other grant may set c4). Every item of the `weapon` armory
with no `grants.csv` row must be a weapon of the loaded zones.
`primary_slots` must be a whole count above zero, and
`armory_replacement_grants` may name only `weapon` and `launcher`. `armor_absorb_percent`
must be a whole percent 0–100, and the armory menu prefix, purchase prefix,
close answer, replacement menu and its answer to go ahead must name
something. Every `playerdata.csv` field must be
named once, `surHUD_credits` and `experience` among them as plain numbers,
and every kind a field lists must be an item kind of `armory_table` keying
no other field. `rank_table` must have a `maxrank` row with a last rank in
c1 and every rank below it an ending experience in c7, and `start_rank` must
be `max` or a rank of it with a starting experience in c2. The fault names
each gap. Body, armory, plane and
sentry models are not checked.

## Evidence lines
`gsc log: survival: ready waves=… archetypes=… armory_items=…` counts the
tables as read; `gsc log: survival: loadout …` lists what a spawn was given;
`gsc log: survival: armory <kind> spawned at (…) yaw Y model <m> menu <menu>`
each armory placed, `armory <kind> usable at (…)` it unlocking for use,
`armory <kind> opened client=N menu=<menu>` and `armory <kind> closed
client=N reason=<closed|downed|died>` a use; `gsc log: survival: armor
absorbed client=N damage=D absorbed=A armor=L health_damage=H means=<MOD>`
a hit on armor;
`gsc log: survival: challenges played <ref>=<means>x<count>:<reward>… not
played <ref>…` the challenges as read.
`gsc log: survival: wave=N start squad=<a> xK [specials=<a> xK,…]
[bosses=<a>,…] [vehicles=<a>,…] total=T budget=B [repeats=W]` opens a wave
(T counts the enemies bots play and the choppers; `repeats` names the table wave a wave past
the last plays); `gsc log: survival: wave N skips <a> xK (not simulated)` each
archetype it leaves out; `gsc log: survival: wave=N requests <a> xK
in_play=P budget=B` a request;
`bots: add K (dummy=false) archetype=<a> skill=S → clients [...]` and
`bots: JoinMatch client=N team=axis archetype=<a> — no class` its admission;
`gsc log: survival: enemy connected client=N archetype=<a> team=axis
health=H trait=<t>` and `gsc log: survival: <a> spawned client=N at (…)
weapons=… [speed=P%] health=…` the spawn;
`bots: id=N tick=T hunt=clientM dist=D target=(…)` a hunt's choice of target,
then the same line with `origin=(…)` each time it closes by 512 units, and
`hunt=clientM dist=D` on the bot's task trace; `bots: id=N … hunt=clientM
unreachable …` a target the graph cannot reach, and `bots: id=N … hunts nobody`
the fall back to roaming;
`gsc log: survival: enemy killed client=N archetype=<a> remaining=R
means=<MOD> weapon=<w> by=<clientM|chopper|world>` (R:
the wave's total less its kills) then `bots: retired client N (…)` the end of
one, and `gsc log: survival: enemy left client=N archetype=<a> without a
kill, asked again` one that left alive. `gsc log: survival: hud wave=W
enemies=E players=P` each time the wave's dvars and hudelems change (its
start and every kill); `gsc log: survival: credits client=N credits=C
delta=D reason=<start|kill:<a>|challenge:<ref>|wave_bonus:W>` each change of
a player's credits, and `gsc log: survival: challenge client=N <ref> K/R` a
challenge's progress. `gsc log: survival: wave=N complete
killed=K` (then the wave bonus), `gsc log: survival: intermission Ss before wave=N+1` and
`gsc log: survival: armory unlocked <kind> after wave=N` close a wave.
`gsc log: survival: laststand client=N selfrevive=K bleedout=S weapon=<w>
means=<MOD> by=<clientM|chopper|world>` a player going down, `gsc log: survival: revived by kill
selfrevive=K client=N health=H` getting up by a kill, `gsc log: survival: revived by
wave_complete wave=W selfrevive=K client=N health=H` at the wave's end, `gsc log: survival: bled out
client=N after Ss` the bleed-out, `gsc log: survival: player died client=N
means=<MOD> selfrevive=K by=<clientM|chopper|world>` a death, and `gsc log: survival: game over wave=W
kills=K credits=C earned=E headshots=H downs=D time=M:SS client=N
menu=<menu> r<row>:<stat>=<value>…` the end of the run, then `ui info menu
open: sp_eog_summary`. The kill
recipe in [`RUN.md`](RUN.md) clears wave 1 and produces all of these.
`net info host pause: paused tick=T menu=<menu>` the pause menu stopping the
game, `net info host pause: resumed tick=T paused_tick=T after=Ss` it
resuming (both ticks the same), and `net info host pause: … open, game keeps
running (role=… remote_clients=…)` (the menu name in backquotes) a pause the
host refused.
`gsc log: survival: playerdata client=N experience=X rank=R` the match player
data a player starts with, and `gsc log: survival: playerdata client=N
armory<kind> <ref>=K` each armory value written (the credits are written with
every `credits` line); `ui info menu <menu>: item I '<name>' enabled|disabled`
(the item name in backquotes in the log) each time an item's disabled expression reads otherwise, as the menu
evaluates it (on open and on navigation).
`gsc log: survival: bought <ref> kind=<kind> price=P client=N` a purchase and
`gsc log: survival: purchase refused client=N <ref>: <reason>` a refusal;
`gsc log: survival: purchase <ref> asks client=N menu=<menu>: replaces <w>`
the replacement warning and `replacement answered <answer> item=<c0>` its
answer; `gsc log: survival: granted <ref> grant=<g> held=H of M [weapon=<w>
clip=C stock=S] [action_slot=A] [replaced=<w>] armor=P selfrevive=K
client=N` an item of the player's kit given, `granted ammo client=N
refilled <w>=<clip>+<stock>…` a refill, and `replaced <w> with <w>` a
weapon taking another's slot. The purchase recipe in [`RUN.md`](RUN.md)
produces them.
`gsc log: survival: <ref> on action slot S weapon=<w>` an item waiting;
`gsc log: survival: airstrike called at (…) yaw Y` then `gsc log: survival:
airstrike at (…) kills=K bombs=B weapon=<w>`; `gsc log: survival: predator
launched … from (…)` then `gsc log: survival: predator impact at (…)
kills=K`; `gsc log: survival: sentry carried`, `sentry does not fit at (…)`,
`sentry placed weapon=<w> at (…) team=<t> firetime=F` and `sentry kill
client=N victim=M archetype=<a> weapon=<w>`; `gsc log: survival: allies
requested K archetype=<a> free=F`, `requests ally <a> xK owed=O free=F`,
`ally connected client=N archetype=<a> team=allies` and `ally left …`;
`gsc log: survival: perk <ref> set`. The air support recipe in
[`RUN.md`](RUN.md) produces them.
`gsc log: survival: explosive weakness client=N archetype=<a> means=<MOD>
weapon=<w> damage=D scaled=S` an explosive hit on an `explosive` archetype;
`gsc log: survival: martyrdom c4 dropped client=N at (…) weapon=<w>` then
`martyrdom c4 detonated …`; `gsc log: survival: claymore planted client=N at
(…) yaw Y weapon=<w>`, `claymore tripped by client=M planter=N at (…)`,
`claymore detonated planter=N at (…)` and `claymore disarmed planter=N left,
at (…)`; `gsc log: survival: chemical gas at (…) radius=R ms=T client=N`,
`gas shock client=M shock=<shock> distance=D` each interval a player is in
it, and `chemical gas at (…) cleared`. The heavy archetype recipe in
[`RUN.md`](RUN.md) produces them.
`gsc log: survival: chopper path starts=S nodes=N links=L dangling=D` the
map's chopper path as read; `gsc log: survival: chopper spawned path=<first
node> from (…) health=H weapon=<w> type=<t> firetime=F model=<m> team=<t>` a
chopper entering, `chopper heads to <node> at (…)` each move, `chopper fired
at client=N shots=K from (…)` each burst, and `chopper destroyed reward=R
by=<clientN|world> means=<MOD> weapon=<w> at (…) remaining=E` its end. The
chopper recipe in [`RUN.md`](RUN.md) produces them.
