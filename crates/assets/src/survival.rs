use std::collections::BTreeMap;

use crate::ScriptTable;

mod aitypes;

pub(crate) use aitypes::is_dress_script;

const SETTINGS_TABLE: &str = "survival/settings.csv";
const SETTINGS: &str = include_str!("survival/settings.csv");

const WEAPON_ALIASES_TABLE: &str = "survival/weapon_aliases.csv";
const ARCHETYPES_TABLE: &str = "survival/archetypes.csv";
/// Written at load from the aitypes of the loaded zones: a character script
/// by number, its bodies, heads and hats (each space separated, one drawn
/// per spawn).
const CHARACTERS_TABLE: &str = "survival/characters.csv";
const AITYPE_FILL_INS_TABLE: &str = "survival/aitype_fill_ins.csv";
const HUD_TABLE: &str = "survival/hud.csv";
const HUD: &str = include_str!("survival/hud.csv");
const CHALLENGES_TABLE: &str = "survival/challenges.csv";
const EOG_TABLE: &str = "survival/eog.csv";
const EOG: &str = include_str!("survival/eog.csv");
const AIRSUPPORT_TABLE: &str = "survival/airsupport.csv";
const HEAVIES_TABLE: &str = "survival/heavies.csv";
const GRANTS_TABLE: &str = "survival/grants.csv";
const PLAYER_DATA_TABLE: &str = "survival/playerdata.csv";
const ACTOR_ANIMS_TABLE: &str = "survival/actor_anims.csv";
const PLAYER_ANIMS_TABLE: &str = "survival/player_anims.csv";

/// The tables survival authors itself, beside the MW3 tables it reads from
/// the user's zones: only what those tables lack.
const TABLES: [(&str, &str); 12] = [
    (
        WEAPON_ALIASES_TABLE,
        include_str!("survival/weapon_aliases.csv"),
    ),
    (ARCHETYPES_TABLE, include_str!("survival/archetypes.csv")),
    (
        AITYPE_FILL_INS_TABLE,
        include_str!("survival/aitype_fill_ins.csv"),
    ),
    (HUD_TABLE, HUD),
    (CHALLENGES_TABLE, include_str!("survival/challenges.csv")),
    (EOG_TABLE, EOG),
    (AIRSUPPORT_TABLE, include_str!("survival/airsupport.csv")),
    (HEAVIES_TABLE, include_str!("survival/heavies.csv")),
    (GRANTS_TABLE, include_str!("survival/grants.csv")),
    (PLAYER_DATA_TABLE, include_str!("survival/playerdata.csv")),
    (ACTOR_ANIMS_TABLE, include_str!("survival/actor_anims.csv")),
    (
        PLAYER_ANIMS_TABLE,
        include_str!("survival/player_anims.csv"),
    ),
];

const MAP_TABLES: [(&str, &str); 1] = [("mp_paris", include_str!("survival/maps/mp_paris.csv"))];

const ARMORY_KINDS: [&str; 3] = ["weapon", "equipment", "airsupport"];

/// What `layer_waves_table` is spelled with where it names a tier of
/// `layer_tiers`.
const LAYER_TIER: &str = "<tier>";

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
    /// The spawner classname that names the archetype's aitype.
    pub(super) const CLASSNAME: usize = 4;
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
/// vertalign, alignx, aligny, x, y, and the name holding its number (a client
/// dvar and a player data field).
const HUD_COLUMNS: usize = 11;
const HUD_DVAR_COLUMN: usize = 10;

/// The script dvar listing the refs of `HUD_TABLE` a HUD menu item draws,
/// whose hudelems the gametype script leaves out.
pub const SURVIVAL_MENU_HUD_DVAR: &str = "survival_menu_hud";

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
/// the credits and the experience MW3's armory menus read, and the wave, the
/// enemies left and the last credit change its HUD menus read.
const PLAYER_DATA_SCALARS: [&str; 5] = [
    "surHUD_credits",
    "experience",
    "surHUD_credits_delta",
    "surHUD_wave",
    "surHUD_enemy",
];

/// The field of `PLAYER_DATA_TABLE` keyed by the HUD menus' elements (c2),
/// each shown while its key is set.
const PLAYER_DATA_HUD: &str = "surHUD";
const PLAYER_DATA_KEYS_COLUMN: usize = 2;

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
/// detonator it is held as (c1) and the missile it launches (c2). The ammo refill
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

/// `ARCHETYPES_TABLE`: ref, weapon, character ids (rows of
/// `CHARACTERS_TABLE`, space separated, one drawn per spawn; written at load),
/// trait, move speed in percent, and for an actor its accuracy in percent and
/// burst length in shots.
mod fill_in_column {
    pub(super) const WEAPON: usize = 1;
    pub(super) const CHARACTERS: usize = 2;
    pub(super) const TRAIT: usize = 3;
    pub(super) const SPEED: usize = 4;
    pub(super) const ACCURACY: usize = 5;
    pub(super) const BURST: usize = 6;
}

/// `ACTOR_ANIMS_TABLE` and `PLAYER_ANIMS_TABLE`: state, stance (empty for
/// any), direction (empty for any), anim names (space separated, one drawn
/// per play).
mod actor_anim_column {
    pub(super) const SET: usize = 0;
    pub(super) const STATE: usize = 1;
    pub(super) const STANCE: usize = 2;
    pub(super) const DIRECTION: usize = 3;
    pub(super) const ANIMS: usize = 4;
}

/// The anim sets with the states their script picks anims for, and the
/// stances and push directions it picks by. A dog's skeleton has no human
/// bones, so it never plays the human set.
const ACTOR_ANIM_SETS: [(&str, &[&str]); 2] = [
    (
        "human",
        &[
            "idle", "run", "walk", "aim", "fire", "pain", "death", "explode",
        ],
    ),
    ("dog", &["idle", "run", "attack", "pain", "death"]),
];
const ACTOR_STANCES: [&str; 2] = ["stand", "crouch"];
const ACTOR_DIRECTIONS: [&str; 4] = ["f", "b", "l", "r"];

/// The states and stances a player's third-person body is picked by; its
/// directions are the actor's.
const PLAYER_ANIM_STATES: [&str; 7] = ["idle", "walk", "run", "sprint", "fire", "reload", "death"];
const PLAYER_STANCES: [&str; 3] = ["stand", "crouch", "prone"];

/// Settings rows the actor script reads as whole numbers, and those of them
/// that are percents.
const ACTOR_NUMBERS: [&str; 15] = [
    "actor_budget",
    "actor_spawn_clearance",
    "actor_think_ms",
    "actor_blend_ms",
    "actor_goal_radius",
    "actor_engage_range",
    "actor_walk_range",
    "actor_walk_rate_percent",
    "actor_crouch_percent",
    "actor_pain_percent",
    "actor_burst_rest_ms",
    "actor_miss_spread",
    "actor_eye_height",
    "actor_crouch_eye_height",
    "actor_corpse_ms",
];
const ACTOR_PERCENTS: [&str; 3] = [
    "actor_walk_rate_percent",
    "actor_crouch_percent",
    "actor_pain_percent",
];
/// Settings rows the dog script reads as whole numbers.
const DOG_NUMBERS: [&str; 5] = [
    "dog_attack_range",
    "dog_bite_radius",
    "dog_bite_damage",
    "dog_bite_ms",
    "dog_attack_rest_ms",
];

mod character_column {
    pub(super) const BODIES: usize = 1;
    pub(super) const HEADS: usize = 2;
}

/// A body model whose name starts so carries its own head.
const FULLBODY_PREFIX: &str = "fullbody_";

/// What the gametype script does for an archetype beyond its weapon and
/// health (`ARCHETYPES_TABLE` c3), and the `HEAVIES_TABLE` rows each reads.
const TRAITS: [(&str, &[&str]); 6] = [
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
            "chopper_model",
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
    (
        "splode",
        &[
            "splode_fx",
            "splode_means",
            "splode_radius",
            "splode_damage_max",
            "splode_damage_min",
        ],
    ),
];

/// `HEAVIES_TABLE` rows that name a weapon ref (through `WEAPON_ALIASES_TABLE`),
/// an effect, a shellshock, means of death, a vehicle or a map key value,
/// rather than a whole number.
const HEAVIES_WEAPONS: [&str; 2] = ["martyrdom_weapon", "claymore_weapon"];
const HEAVIES_MEANS: [&str; 3] = ["explosive_means", "gas_means", "splode_means"];
const HEAVIES_NAMES: [&str; 10] = [
    "gas_fx",
    "gas_shock_high",
    "gas_shock_med",
    "gas_shock_low",
    "chopper_vehicle",
    "chopper_model",
    "chopper_path_noteworthy",
    "chopper_start_targetname",
    "chopper_death_fx",
    "splode_fx",
];

/// Action slots run 1 to 4.
const ACTION_SLOTS: std::ops::RangeInclusive<u32> = 1..=4;

/// An IW5 runtime carries compiled scripts only, so every module the engine
/// starts or calls by name is ours.
const MODULES: [(&str, &str); 7] = [
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
    (
        "maps/mp/_survival_actor",
        include_str!("survival/actor.gsc.txt"),
    ),
    (
        "maps/mp/_survival_dog",
        include_str!("survival/dog.gsc.txt"),
    ),
];

const GAMETYPE: &str = include_str!("survival/survival.gsc.txt");

/// The menus that draw the survival HUD (`hud_menus`).
pub fn survival_hud_menus() -> impl Iterator<Item = &'static str> {
    survival_setting("hud_menus")
        .unwrap_or("")
        .split_whitespace()
}

/// Resolves MW3's Survival menus in `catalog`: the HUD menus, the summary
/// (`eog_menu`) and the armory menus (`armory_menu_prefix`). Returns a line
/// per menu and per row of `HUD_TABLE`, and the refs of the rows whose dvar
/// the text of a HUD menu item reads.
pub fn survival_menu_hud(
    catalog: Option<&asset_game::MenuCatalog>,
) -> (Vec<String>, Vec<&'static str>) {
    let Some(catalog) = catalog else {
        return (
            vec!["survival menus: no menu catalog; the hudelems draw the survival HUD".into()],
            Vec::new(),
        );
    };
    let mut lines = Vec::new();
    let mut resolve = |role: &str, name: &str| {
        lines.push(match catalog.get(name) {
            Some(menu) => format!("survival menus: {role} `{name}` {} items", menu.items.len()),
            None => format!("survival menus: {role} `{name}` is in no zone"),
        });
    };
    for name in survival_hud_menus() {
        resolve("hud", name);
    }
    resolve("eog", survival_setting("eog_menu").unwrap_or(""));
    let prefix = survival_setting("armory_menu_prefix").unwrap_or("");
    let armories: Vec<&str> = catalog
        .menus
        .keys()
        .filter(|name| {
            !prefix.is_empty()
                && name
                    .get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        })
        .map(String::as_str)
        .collect();
    for name in &armories {
        resolve("armory", name);
    }
    if armories.is_empty() {
        lines.push(format!("survival menus: no armory menu `{prefix}*`"));
    }
    let mut drawn = Vec::new();
    for row in HUD.lines().filter(|line| !line.trim().is_empty()) {
        let cells: Vec<&str> = row.split(',').map(str::trim).collect();
        let (reference, dvar) = (
            cells.first().copied().unwrap_or(""),
            cells.get(HUD_DVAR_COLUMN).copied().unwrap_or(""),
        );
        let drawer = survival_hud_menus().find_map(|menu| {
            catalog
                .get(menu)?
                .items
                .iter()
                .position(|item| !dvar.is_empty() && item.text_reads(dvar))
                .map(|index| (menu, index))
        });
        lines.push(match drawer {
            Some((menu, item)) => {
                drawn.push(reference);
                format!(
                    "survival hud: `{reference}` ({dvar}) drawn by `{menu}` item {item}; no hudelem"
                )
            }
            None => format!(
                "survival hud: `{reference}` ({dvar}) drawn by no HUD menu item; hudelem kept"
            ),
        });
    }
    (lines, drawn)
}

/// The localize references the end-of-run summary shows: what the summary
/// menu (`eog_menu`) names in its items' text and its handlers, and what the
/// gametype script feeds its dvars, `EOG_TABLE`'s labels and
/// `eog_deadquote`. Returns each reference
/// with where it comes from, split into those `strings` holds and those it
/// lacks, which the summary paints as gaps.
pub fn survival_eog_strings(
    catalog: Option<&asset_game::MenuCatalog>,
    strings: &asset_game::LocalizeCatalog,
) -> (Vec<String>, Vec<String>) {
    let menu = survival_setting("eog_menu").unwrap_or("");
    let mut refs: Vec<(String, String)> = Vec::new();
    let mut add = |reference: String, source: String| {
        if !refs
            .iter()
            .any(|(known, _)| known.eq_ignore_ascii_case(&reference))
        {
            refs.push((reference, source));
        }
    };
    for (reference, source) in catalog
        .and_then(|catalog| catalog.get(menu))
        .into_iter()
        .flat_map(asset_game::MenuDef::loc_refs)
    {
        add(reference, format!("{menu} {source}"));
    }
    for row in EOG.lines().filter(|line| !line.trim().is_empty()) {
        let cells: Vec<&str> = row.split(',').map(str::trim).collect();
        let (line, label) = (
            cells.first().copied().unwrap_or(""),
            cells.get(1).copied().unwrap_or(""),
        );
        add(label.to_owned(), format!("{EOG_TABLE} row {line}"));
    }
    add(
        survival_setting("eog_deadquote").unwrap_or("").to_owned(),
        "eog_deadquote".into(),
    );
    let (mut resolved, mut missing) = (Vec::new(), Vec::new());
    for (reference, source) in refs {
        let found = strings.text(reference.trim_start_matches('@')).is_some();
        let entry = format!("`{reference}` ({source})");
        if found {
            resolved.push(entry);
        } else {
            missing.push(entry);
        }
    }
    (resolved, missing)
}

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

/// The MW3 zones survival loads right after the map, in load order, ahead of
/// its mode zones. A settings file that names none is refused rather than
/// read as the multiplayer default.
pub fn survival_base_zones() -> Result<asset_transport::BaseZones, String> {
    let zones = asset_transport::BaseZones::parse(required_setting("base_zones")?);
    if zones.is_empty() {
        return Err(format!("{SETTINGS_TABLE}: `base_zones` names no zone"));
    }
    Ok(zones)
}

/// The MW3 zones survival loads after its base zones, in load order, over
/// those base zones, then the Special Ops layers (`layer_zones`) the user's
/// data has. Survival reads its tables, menus, weapons and sounds from the
/// mode's zones, so a settings file that names none is refused rather than
/// read as a mode without zones.
pub fn survival_mode_zones() -> Result<asset_transport::ModeZones, String> {
    let zones = asset_transport::ModeZones::parse(required_setting("mode_zones")?)
        .with_layers(required_setting("layer_zones")?);
    if zones.is_empty() {
        return Err(format!("{SETTINGS_TABLE}: `mode_zones` names no zone"));
    }
    Ok(zones.with_base(survival_base_zones()?))
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

/// `WEAPON_ALIASES_TABLE`'s weapon columns: the weapon a ref is held as and,
/// for the predator, the missile it launches. Each cell lists candidates, SP
/// names first and the MP name last.
mod alias_column {
    pub(super) const HELD: usize = 1;
    pub(super) const MISSILE: usize = 2;
    pub(super) const WEAPONS: [usize; 2] = [HELD, MISSILE];
}

/// Narrows every `WEAPON_ALIASES_TABLE` cell to its first candidate
/// `is_loaded` knows by its exact script name, so the script and
/// `survival_data_check` read one weapon per ref. A cell with no loaded
/// candidate is left whole for the check to refuse.
pub fn survival_pick_weapons(
    scripts: &mut crate::ScriptSources,
    is_loaded: impl Fn(&str) -> bool,
) -> Vec<String> {
    let Some(aliases) = scripts.table_mut(WEAPON_ALIASES_TABLE) else {
        return Vec::new();
    };
    let columns = aliases.columns.max(1);
    let mut picks = Vec::new();
    for row in aliases.cells.chunks_exact_mut(columns).take(aliases.rows) {
        for column in alias_column::WEAPONS.into_iter().filter(|&c| c < columns) {
            let Some(picked) = row[column]
                .split_whitespace()
                .find(|weapon| is_loaded(weapon))
                .map(str::to_owned)
            else {
                continue;
            };
            picks.push(format!("{}->{picked}", row[0]));
            row[column] = picked;
        }
    }
    picks
}

/// Narrows every anim cell of `ACTOR_ANIMS_TABLE` and `PLAYER_ANIMS_TABLE`
/// to the anims `is_loaded` knows. A cell left with none takes its state's
/// first row, so actors and players play only loaded anims; a state whose
/// first row has none plays nothing. Returns the anims missing, each once.
pub fn survival_pick_anims(
    scripts: &mut crate::ScriptSources,
    is_loaded: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut missing = std::collections::BTreeSet::new();
    for name in [ACTOR_ANIMS_TABLE, PLAYER_ANIMS_TABLE] {
        if let Some(anims) = scripts.table_mut(name) {
            missing.extend(pick_table_anims(anims, &is_loaded));
        }
    }
    missing.into_iter().collect()
}

fn pick_table_anims(
    anims: &mut ScriptTable,
    is_loaded: &impl Fn(&str) -> bool,
) -> std::collections::BTreeSet<String> {
    let columns = anims.columns.max(1);
    let mut missing = std::collections::BTreeSet::new();
    if columns <= actor_anim_column::ANIMS {
        return missing;
    }
    let mut first: BTreeMap<(String, String), String> = BTreeMap::new();
    for row in anims.cells.chunks_exact_mut(columns).take(anims.rows) {
        let state = (
            row[actor_anim_column::SET].clone(),
            row[actor_anim_column::STATE].clone(),
        );
        let cell = &mut row[actor_anim_column::ANIMS];
        let (loaded, absent): (Vec<&str>, Vec<&str>) =
            cell.split_whitespace().partition(|name| is_loaded(name));
        missing.extend(absent.into_iter().map(str::to_owned));
        let mut narrowed = loaded.join(" ");
        match first.get(&state) {
            Some(fallback) if narrowed.is_empty() => narrowed.clone_from(fallback),
            Some(_) => {}
            None => {
                first.insert(state, narrowed.clone());
            }
        }
        *cell = narrowed;
    }
    missing
}

/// The archetypes of the waves table, by ref, with the characters their
/// aitype draws or why it draws none. An archetype's classname is the one a
/// script of the loaded zones pairs with its ref (a map's override), else the
/// waves table's (c4), else `AITYPE_FILL_INS_TABLE`'s; one with none is
/// dressed by nothing.
type Dresses = Vec<(String, Result<aitypes::Dress, String>)>;

fn archetype_dresses(
    tables: &BTreeMap<String, ScriptTable>,
    scripts: &BTreeMap<String, Vec<u8>>,
) -> Result<Dresses, String> {
    let table = |name: &str| {
        tables
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| format!("no loaded zone holds `{name}`"))
    };
    let waves = table(required_setting("waves_table")?)?;
    let fill_ins = table(AITYPE_FILL_INS_TABLE)?;
    let archetypes: Vec<&[String]> = waves.rows().filter(|row| is_archetype_row(row)).collect();
    let refs: Vec<&str> = archetypes.iter().map(|row| cell(row, 1)).collect();
    let overrides = aitypes::overrides(scripts, &refs);
    Ok(archetypes
        .into_iter()
        .filter_map(|row| {
            let reference = cell(row, 1);
            let fill_in = fill_ins
                .rows()
                .find(|fill| cell(fill, 0).eq_ignore_ascii_case(reference))
                .map_or("", |fill| cell(fill, 1));
            let (classname, overridden_by) = match overrides.get(&reference.to_ascii_lowercase()) {
                Some((classname, script)) => (classname.as_str(), Some(script.clone())),
                None => match cell(row, archetype_column::CLASSNAME) {
                    "" if fill_in.is_empty() => return None,
                    "" => (fill_in, None),
                    classname => (classname, None),
                },
            };
            Some((
                reference.to_owned(),
                aitypes::dress(classname, overridden_by, scripts),
            ))
        })
        .collect())
}

/// Dresses the archetypes as MW3 does, from the aitypes of the loaded zones:
/// writes `CHARACTERS_TABLE` and each archetype's characters into
/// `ARCHETYPES_TABLE`, before the models the tables name are kept. An
/// archetype that cannot be dressed is left without characters for
/// `survival_data_check` to refuse. Returns a line per archetype.
pub(crate) fn survival_dress_archetypes(scripts: &mut crate::ScriptSources) -> Vec<String> {
    let mut lines = Vec::new();
    let mut characters = BTreeMap::new();
    match archetype_dresses(scripts.tables(), scripts.dress_scripts()) {
        Err(error) => lines.push(format!("survival dress gap: {error}")),
        Ok(dresses) => {
            for (reference, dress) in dresses {
                let dress = match dress {
                    Ok(dress) => dress,
                    Err(error) => {
                        lines.push(format!("survival dress gap: {reference}: {error}"));
                        continue;
                    }
                };
                let ids: Vec<&str> = dress.characters.iter().map(|c| c.id.as_str()).collect();
                let written = scripts.set_table_cells(
                    ARCHETYPES_TABLE,
                    &reference,
                    &[(fill_in_column::CHARACTERS, ids.join(" "))],
                );
                lines.push(format!(
                    "survival dress: {reference} -> {}{} characters={}{}",
                    dress.aitype,
                    dress
                        .overridden_by
                        .as_ref()
                        .map_or(String::new(), |script| format!(" (override in {script})")),
                    ids.len(),
                    if written {
                        String::new()
                    } else {
                        format!(" but no row of {ARCHETYPES_TABLE} takes them")
                    },
                ));
                for character in dress.characters {
                    let row = [
                        character.id.clone(),
                        character.bodies.join(" "),
                        character.heads.join(" "),
                        character.hats.join(" "),
                    ]
                    .join(",");
                    characters.insert(character.id, row);
                }
            }
        }
    }
    let csv: Vec<String> = characters.into_values().collect();
    scripts.insert_table(CHARACTERS_TABLE, &csv.join("\n"));
    lines
}

fn is_archetype_row(row: &[String]) -> bool {
    cell(row, 0)
        .parse::<u32>()
        .is_ok_and(|index| ARCHETYPE_ROWS.contains(&index))
}

/// The rows of `PLAYER_ANIMS_TABLE` as the match narrowed them: state,
/// stance (empty for any), direction (empty for any) and the anims, one
/// drawn per play.
pub fn survival_player_anims(
    scripts: &crate::ScriptSources,
) -> Vec<(String, String, String, Vec<String>)> {
    use actor_anim_column as c;
    let Some(anims) = scripts.table(PLAYER_ANIMS_TABLE) else {
        return Vec::new();
    };
    anims
        .rows()
        .map(|row| {
            (
                cell(row, c::STATE).to_owned(),
                cell(row, c::STANCE).to_owned(),
                cell(row, c::DIRECTION).to_owned(),
                cell(row, c::ANIMS)
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
            )
        })
        .collect()
}

/// Holds survival's own tables to the data loaded with the match: every
/// weapon they name is one of its weapons, every archetype they fill in is
/// one of the waves table's, and every armory placement on `zone` is
/// readable. The gametype script trusts them, so a gap refuses the match.
pub fn survival_data_check(
    scripts: &crate::ScriptSources,
    zone: &str,
    is_weapon: impl Fn(&str) -> bool,
) -> Result<(), String> {
    let (tables, schemas) = (scripts.tables(), scripts.schemas());
    let table = |name: &str| {
        tables
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| format!("no loaded zone holds `{name}`"))
    };
    let mut gaps = Vec::new();
    let aliases = table(WEAPON_ALIASES_TABLE)?;
    for row in aliases.rows() {
        let reference = cell(row, 0);
        if cell(row, alias_column::HELD).is_empty() {
            gaps.push(format!(
                "{WEAPON_ALIASES_TABLE}: `{reference}` names no weapon"
            ));
        }
        for candidates in alias_column::WEAPONS
            .map(|column| cell(row, column))
            .into_iter()
            .filter(|candidates| !candidates.is_empty() && !is_weapon(candidates))
        {
            gaps.push(format!(
                "{WEAPON_ALIASES_TABLE}: `{reference}` -> `{candidates}` names no weapon of the loaded zones"
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
    let characters = table(CHARACTERS_TABLE)?;
    check_characters(characters, &mut gaps);
    let dresses = archetype_dresses(tables, scripts.dress_scripts())?;
    for row in table(AITYPE_FILL_INS_TABLE)?.rows() {
        let reference = cell(row, 0);
        match waves
            .rows()
            .find(|wave| is_archetype_row(wave) && cell(wave, 1).eq_ignore_ascii_case(reference))
            .map(|wave| cell(wave, archetype_column::CLASSNAME))
        {
            None => gaps.push(format!(
                "{AITYPE_FILL_INS_TABLE}: `{reference}` is no archetype of {waves_name}"
            )),
            Some("") => {}
            Some(classname) => gaps.push(format!(
                "{AITYPE_FILL_INS_TABLE}: {waves_name} names `{reference}`'s classname: `{classname}`"
            )),
        }
    }
    let player = required_setting("player_archetype")?;
    if !dresses
        .iter()
        .any(|(reference, dress)| reference.eq_ignore_ascii_case(player) && dress.is_ok())
    {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `player_archetype` `{player}` is no archetype dressed from the loaded zones"
        ));
    }
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
        let accuracy = cell(row, fill_in_column::ACCURACY);
        if !accuracy.is_empty() && !accuracy.parse::<u32>().is_ok_and(|percent| percent <= 100) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` accuracy `{accuracy}` is not a whole percent from 0 to 100"
            ));
        }
        let burst = cell(row, fill_in_column::BURST);
        if !burst.is_empty() && !burst.parse::<u32>().is_ok_and(|shots| shots > 0) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` burst `{burst}` is not a whole number of shots above zero"
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
        let resolved = alias_weapon(aliases, weapon);
        if !weapon.is_empty() && !is_weapon(resolved) {
            gaps.push(format!(
                "{ARCHETYPES_TABLE}: `{archetype}` -> `{weapon}` (`{resolved}`) is no weapon of the loaded zones"
            ));
        }
        for character in cell(row, fill_in_column::CHARACTERS).split_whitespace() {
            if !characters.rows().any(|row| cell(row, 0) == character) {
                gaps.push(format!(
                    "{ARCHETYPES_TABLE}: `{archetype}` character `{character}` is no row of {CHARACTERS_TABLE}"
                ));
            }
        }
    }
    let (list_name, list) = match layer_waves_tables(tables)?[..] {
        [] => (waves_name.to_owned(), waves),
        [ref name] => (name.clone(), table(name)?),
        ref several => {
            gaps.push(format!(
                "{SETTINGS_TABLE}: the loaded zones hold more than one `layer_waves_table`: {several:?}"
            ));
            (waves_name.to_owned(), waves)
        }
    };
    check_waves(
        table(ARCHETYPES_TABLE)?,
        table(WEAPON_ALIASES_TABLE)?,
        (waves_name, waves),
        (&list_name, list),
        &dresses,
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
    check_actor(table(ACTOR_ANIMS_TABLE)?, &mut gaps)?;
    check_player_anims(table(PLAYER_ANIMS_TABLE)?, &mut gaps)?;
    let def_name = required_setting("player_data_def")?;
    let schema = schemas
        .get(&def_name.to_ascii_lowercase())
        .and_then(|set| set.definitions.iter().max_by_key(|d| d.version))
        .map(|definition| (def_name, definition));
    check_player_data(
        table(PLAYER_DATA_TABLE)?,
        schema,
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

/// The map layer's waves tables the loaded zones hold: `layer_waves_table`
/// for each of `layer_tiers`. The script plays the first; more than one is a
/// gap.
fn layer_waves_tables(tables: &BTreeMap<String, ScriptTable>) -> Result<Vec<String>, String> {
    let pattern = required_setting("layer_waves_table")?;
    if !pattern.contains(LAYER_TIER) {
        return Err(format!(
            "{SETTINGS_TABLE}: `layer_waves_table` `{pattern}` names no {LAYER_TIER}"
        ));
    }
    Ok(required_setting("layer_tiers")?
        .split_whitespace()
        .map(|tier| pattern.replace(LAYER_TIER, tier))
        .filter(|name| tables.contains_key(&name.to_ascii_lowercase()))
        .collect())
}

/// Holds every wave row (of `list`: the map layer's waves table, else the
/// waves table itself) to the archetypes (of `waves`) and armories the
/// script plays it with: waves numbered 1 to N with a repeat block, refs that
/// name archetypes, parallel special counts, and for every archetype an actor
/// plays a health, a dress of the loaded zones' aitypes unless it is a
/// vehicle, and, unless it is one of `dog_archetypes`, a weapon of the loaded
/// zones.
fn check_waves(
    archetypes: &ScriptTable,
    aliases: &ScriptTable,
    (waves_name, waves): (&str, &ScriptTable),
    (list_name, list): (&str, &ScriptTable),
    dresses: &Dresses,
    is_weapon: &impl Fn(&str) -> bool,
    gaps: &mut Vec<String>,
) -> Result<(), String> {
    use wave_column as w;
    let archetype_row = |reference: &str| {
        waves
            .rows()
            .find(|row| is_archetype_row(row) && cell(row, 1).eq_ignore_ascii_case(reference))
    };
    let mut vehicles = Vec::new();
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
    let dogs: Vec<&str> = required_setting("dog_archetypes")?
        .split_whitespace()
        .collect();
    for reference in &dogs {
        if archetype_row(reference).is_none() {
            gaps.push(format!(
                "{SETTINGS_TABLE}: dog `{reference}` is no archetype of {waves_name}"
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
    for row in list.rows().filter(|row| cell(row, w::MARKER) == "1") {
        let Ok(number) = cell(row, w::NUMBER).parse::<u32>() else {
            gaps.push(format!(
                "{list_name}: wave row `{}` has no wave number in c{}",
                cell(row, 0),
                w::NUMBER
            ));
            continue;
        };
        numbers.push(number);
        let wave = format!("{list_name}: wave {number}");
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
        vehicles.extend(cell(row, w::VEHICLES).split_whitespace());
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
            "{list_name}: waves are not numbered 1 to N: {numbers:?}"
        ));
    }
    if repeats == 0 {
        gaps.push(format!(
            "{list_name}: no wave is marked in c{} to repeat past the last",
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
    for reference in played {
        let Some(row) = archetype_row(reference) else {
            continue;
        };
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
        if !vehicles.iter().any(|r| r.eq_ignore_ascii_case(reference)) {
            match dresses
                .iter()
                .find(|(dressed, _)| dressed.eq_ignore_ascii_case(reference))
            {
                None => gaps.push(format!(
                    "{waves_name}: archetype `{reference}` names no aitype classname, in c{} or {AITYPE_FILL_INS_TABLE}",
                    archetype_column::CLASSNAME
                )),
                Some((_, Err(error))) => gaps.push(format!(
                    "{waves_name}: archetype `{reference}` is dressed by no aitype of the loaded zones: {error}"
                )),
                Some((_, Ok(_))) => {}
            }
        }
        if dogs.iter().any(|r| r.eq_ignore_ascii_case(reference)) {
            continue;
        }
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
            let resolved = alias_weapon(aliases, named);
            if !is_weapon(resolved) {
                gaps.push(format!(
                    "{waves_name}: archetype `{reference}` -> `{named}` (`{resolved}`) is no weapon of the loaded zones"
                ));
            }
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
                    || (1..8)
                        .chain([HUD_DVAR_COLUMN])
                        .any(|column| cell(row, column).is_empty())
                    || !numbers_read
                {
                    gaps.push(format!(
                        "{HUD_TABLE}: `{reference}` needs a label, a font, a positive font scale in percent, four alignments, whole x and y and a dvar: {row:?}"
                    ));
                }
            }
        }
    }
    if survival_hud_menus().next().is_none() {
        gaps.push(format!("{SETTINGS_TABLE}: `hud_menus` names nothing"));
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

/// Holds `CHARACTERS_TABLE` to what `setcharacter` takes: one row per id,
/// each with a body, and no head on a `fullbody_` body (it draws its own).
fn check_characters(characters: &ScriptTable, gaps: &mut Vec<String>) {
    let mut seen = std::collections::BTreeSet::new();
    for row in characters.rows() {
        let (id, bodies) = (cell(row, 0), cell(row, character_column::BODIES));
        if id.is_empty() || !seen.insert(id) {
            gaps.push(format!(
                "{CHARACTERS_TABLE}: `{id}` is empty or names more than one row"
            ));
        }
        if bodies.is_empty() {
            gaps.push(format!("{CHARACTERS_TABLE}: `{id}` has no body"));
        }
        if let Some(body) = bodies
            .split_whitespace()
            .find(|body| body.starts_with(FULLBODY_PREFIX))
            && !cell(row, character_column::HEADS).is_empty()
        {
            gaps.push(format!(
                "{CHARACTERS_TABLE}: `{id}` has heads on its full body `{body}`"
            ));
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
                let weapon = alias_weapon(aliases, text);
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
        let alias = aliases.rows().find(|alias| cell(alias, 0) == reference);
        let weapons: Vec<&str> = alias.map_or_else(
            || vec![reference],
            |alias| {
                alias_column::WEAPONS
                    .map(|column| cell(alias, column))
                    .into_iter()
                    .filter(|weapon| !weapon.is_empty())
                    .collect()
            },
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
        if how == grant::PREDATOR
            && alias.is_none_or(|alias| cell(alias, alias_column::MISSILE).is_empty())
        {
            gaps.push(format!(
                "{GRANTS_TABLE}: predator `{reference}` names no missile in {WEAPON_ALIASES_TABLE} c2"
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
        if !is_weapon(alias_weapon(aliases, reference)) {
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

/// Holds `ACTOR_ANIMS_TABLE` and the `actor_*` and `dog_*` settings to what
/// the actor and dog scripts read: every row a set, state, stance and
/// direction they pick by, every state of a set with a row, and their
/// settings whole numbers, percents at most 100. Anims are not checked: a
/// missing one falls back to its state's first row.
fn check_actor(anims: &ScriptTable, gaps: &mut Vec<String>) -> Result<(), String> {
    use actor_anim_column as c;
    for row in anims.rows() {
        let (set, state, stance, direction) = (
            cell(row, c::SET),
            cell(row, c::STATE),
            cell(row, c::STANCE),
            cell(row, c::DIRECTION),
        );
        match ACTOR_ANIM_SETS.iter().find(|(name, _)| *name == set) {
            None => gaps.push(format!(
                "{ACTOR_ANIMS_TABLE}: `{set}` is no anim set ({})",
                ACTOR_ANIM_SETS.map(|(name, _)| name).join(", ")
            )),
            Some((_, states)) if !states.contains(&state) => gaps.push(format!(
                "{ACTOR_ANIMS_TABLE}: `{state}` is no `{set}` state ({})",
                states.join(", ")
            )),
            Some(_) => {}
        }
        if !stance.is_empty() && !ACTOR_STANCES.contains(&stance) {
            gaps.push(format!(
                "{ACTOR_ANIMS_TABLE}: `{state}` stance `{stance}` is none of {}",
                ACTOR_STANCES.join(", ")
            ));
        }
        if !direction.is_empty() && !ACTOR_DIRECTIONS.contains(&direction) {
            gaps.push(format!(
                "{ACTOR_ANIMS_TABLE}: `{state}` direction `{direction}` is none of {}",
                ACTOR_DIRECTIONS.join(", ")
            ));
        }
    }
    for (set, states) in ACTOR_ANIM_SETS {
        for state in states {
            if !anims
                .rows()
                .any(|row| cell(row, c::SET) == set && cell(row, c::STATE) == *state)
            {
                gaps.push(format!(
                    "{ACTOR_ANIMS_TABLE}: no row for `{set}` state `{state}`"
                ));
            }
        }
    }
    for key in ACTOR_NUMBERS.into_iter().chain(DOG_NUMBERS) {
        let value = required_setting(key)?;
        let most = if ACTOR_PERCENTS.contains(&key) {
            100
        } else {
            u32::MAX
        };
        if !value.parse::<u32>().is_ok_and(|number| number <= most) {
            gaps.push(format!(
                "{SETTINGS_TABLE}: `{key}` is not a whole number from 0 to {most}: `{value}`"
            ));
        }
    }
    if required_setting("actor_budget")? == "0" {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `actor_budget` 0 lets no wave enemy in"
        ));
    }
    if required_setting("actor_explosive_means")?.is_empty() {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `actor_explosive_means` names nothing"
        ));
    }
    Ok(())
}

/// Holds `PLAYER_ANIMS_TABLE` and the `player_*` settings to what the
/// player's third-person body is picked and drawn by: every row a state,
/// stance and direction it picks by, `idle` standing covered and a whole
/// blend time. Anims are not checked: a missing one falls back to its state's
/// first row.
fn check_player_anims(anims: &ScriptTable, gaps: &mut Vec<String>) -> Result<(), String> {
    use actor_anim_column as c;
    for row in anims.rows() {
        let (state, stance, direction) = (
            cell(row, c::STATE),
            cell(row, c::STANCE),
            cell(row, c::DIRECTION),
        );
        if !PLAYER_ANIM_STATES.contains(&state) {
            gaps.push(format!(
                "{PLAYER_ANIMS_TABLE}: `{state}` is no player state ({})",
                PLAYER_ANIM_STATES.join(", ")
            ));
        }
        if !stance.is_empty() && !PLAYER_STANCES.contains(&stance) {
            gaps.push(format!(
                "{PLAYER_ANIMS_TABLE}: `{state}` stance `{stance}` is none of {}",
                PLAYER_STANCES.join(", ")
            ));
        }
        if !direction.is_empty() && !ACTOR_DIRECTIONS.contains(&direction) {
            gaps.push(format!(
                "{PLAYER_ANIMS_TABLE}: `{state}` direction `{direction}` is none of {}",
                ACTOR_DIRECTIONS.join(", ")
            ));
        }
    }
    if !anims
        .rows()
        .any(|row| cell(row, c::STATE) == "idle" && matches!(cell(row, c::STANCE), "" | "stand"))
    {
        gaps.push(format!(
            "{PLAYER_ANIMS_TABLE}: no `idle` row for a standing player"
        ));
    }
    let blend = required_setting("player_blend_ms")?;
    if blend.parse::<u32>().is_err() {
        gaps.push(format!(
            "{SETTINGS_TABLE}: `player_blend_ms` is not a whole number: `{blend}`"
        ));
    }
    Ok(())
}

/// Holds `PLAYER_DATA_TABLE` to what the script and the menus read: named
/// fields, none twice, the plain numbers the script writes by name, the HUD
/// field keyed by the keys it lists, and for a field keyed by armory kinds
/// kinds of the armory table, each keying one field. With the player data
/// schema loaded, every field is one of its numbers or numbers by key, keyed
/// exactly where the table lists kinds or keys, and every listed key and
/// item of those kinds is a key of it.
fn check_player_data(
    fields: &ScriptTable,
    schema: Option<(&str, &structured_data_iw4::Definition)>,
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
        let keys: Vec<&str> = cell(row, PLAYER_DATA_KEYS_COLUMN)
            .split_whitespace()
            .collect();
        if !keyed.is_empty() && !keys.is_empty() {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: `{name}` lists both armory kinds {keyed:?} and keys {keys:?}"
            ));
        }
        if let Some((def_name, definition)) = schema {
            check_schema_field(def_name, definition, name, (&keyed, &keys), armory, gaps);
        }
        let scalar = PLAYER_DATA_SCALARS
            .iter()
            .any(|scalar| scalar.eq_ignore_ascii_case(name));
        if scalar && (!keyed.is_empty() || !keys.is_empty()) {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: `{name}` is a plain number the script writes, not keyed by {keyed:?}{keys:?}"
            ));
        }
        if name.eq_ignore_ascii_case(PLAYER_DATA_HUD) && keys.is_empty() {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: `{PLAYER_DATA_HUD}` lists no key (c{PLAYER_DATA_KEYS_COLUMN}), which the script sets"
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
    for scalar in PLAYER_DATA_SCALARS.into_iter().chain([PLAYER_DATA_HUD]) {
        if !names.iter().any(|name| name.eq_ignore_ascii_case(scalar)) {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: no `{scalar}` field, which the script writes"
            ));
        }
    }
}

fn check_schema_field(
    def_name: &str,
    definition: &structured_data_iw4::Definition,
    name: &str,
    (kinds, keys): (&[&str], &[&str]),
    armory: &ScriptTable,
    gaps: &mut Vec<String>,
) {
    use structured_data_iw4::DataType::{Bool, Byte, EnumArray, Int, Short};
    use structured_data_iw4::Key::Name;
    let number = |ty| matches!(ty, Int | Byte | Short | Bool);
    let keyed = match definition.path_type(&[Name(name)]) {
        Ok(ty) if number(ty) => false,
        Ok(EnumArray(array))
            if definition
                .enum_arrays
                .get(array)
                .is_some_and(|array| number(array.element)) =>
        {
            true
        }
        other => {
            gaps.push(format!(
                "{PLAYER_DATA_TABLE}: `{name}` is no number or numbers by key of {def_name}: {other:?}"
            ));
            return;
        }
    };
    if keyed == (kinds.is_empty() && keys.is_empty()) {
        gaps.push(format!(
            "{PLAYER_DATA_TABLE}: `{name}` is {} in {def_name}, the table lists kinds {kinds:?} and keys {keys:?}",
            if keyed { "keyed" } else { "one number" }
        ));
        return;
    }
    let missing: Vec<&str> = armory
        .rows()
        .filter(|item| kinds.contains(&cell(item, ARMORY_KIND_COLUMN)))
        .map(|item| cell(item, 1))
        .chain(keys.iter().copied())
        .filter(|reference| definition.lookup(&[Name(name), Name(reference)]).is_err())
        .collect();
    if !missing.is_empty() {
        gaps.push(format!(
            "{PLAYER_DATA_TABLE}: `{name}` of {def_name} has no key {missing:?}"
        ));
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

/// The weapon `reference` is held as: its `WEAPON_ALIASES_TABLE` row's,
/// else the reference itself.
fn alias_weapon<'a>(aliases: &'a ScriptTable, reference: &'a str) -> &'a str {
    aliases
        .rows()
        .find(|row| cell(row, 0).eq_ignore_ascii_case(reference))
        .map_or(reference, |row| cell(row, alias_column::HELD))
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
