//! The survival gametype's natives, linked only into a survival program
//! (`Catalog::survival`). The wave director asks the session for bots by
//! archetype; the session owns admission, so a request only queues. The
//! script writes the player data MW3's armory menus read into the client's
//! match player data.

use super::super::args::{arg, int, kind, string};
use super::super::tables::{table, table_search};
use super::player::player;
use crate::ClientId;
use crate::frame::FrameWorld;
use crate::script::Namespace::{Function, Method};
use crate::script::{Arc, NativeRegistry, Runtime, Value};
use crate::script_bots::ScriptBotSide::{self, Ally, Enemy};
use crate::script_bots::{ScriptBotRequest, ScriptBots};
use bevy_ecs::prelude::World;

/// Survival's own settings table; its `waves_table` row names MW3's table.
const SETTINGS_TABLE: &str = "survival/settings.csv";

/// The player data fields the armory menus read: c0 the field, c1 the armory
/// kinds whose item refs key it (empty for a plain number).
const PLAYER_DATA_TABLE: &str = "survival/playerdata.csv";

/// In MW3's waves table an archetype is a row whose c0 index lies here; c1 is
/// its ref.
const ARCHETYPE_ROWS: std::ops::Range<u32> = 100..1000;

/// The archetype column read as the bots' aim skill. That c8 is a skill is our
/// reading of the table, not an established fact: it rises from 0.9 (easy) to
/// 1.3 (elite) with the tiers, and no other column does.
const ARCHETYPE_SKILL_COLUMN: usize = 8;

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Function, "survival_requestenemies", |world, _, args| {
        request(world, args, Enemy)
    });
    registry.register(Function, "survival_requestallies", |world, _, args| {
        request(world, args, Ally)
    });
    registry.register(Method, "survival_setplayerdata", set_player_data);
    registry.register(Method, "survival_archetype", |world, receiver, args| {
        if !args.is_empty() {
            return Err("survival_archetype expects no arguments".into());
        }
        let client = player(world, receiver)?;
        Ok(world
            .resource::<ScriptBots>()
            .archetype(ClientId(client))
            .map_or(Value::Undefined, |archetype| Value::string(archetype)))
    });
}

/// `self survival_setplayerdata( field, [key,] value )`: a whole number into
/// the client's match player data, which `PLAYER_DATA_TABLE` declares on the
/// first write of a match. A field it does not declare, or a key on a plain
/// field (or none on a keyed one), faults the script.
fn set_player_data(world: &mut World, receiver: &Value, args: &[Value]) -> Result<Value, String> {
    let client = ClientId(player(world, receiver)?);
    let (name, key, value) = match args.len() {
        2 => (string(args, 0)?, None, 1),
        3 => (string(args, 0)?, Some(string(args, 1)?), 2),
        _ => return Err("expects a field, an optional key and a value".into()),
    };
    let value = match arg(args, value)? {
        Value::Int(value) => *value,
        other => return Err(format!("the value is {}, not an int", kind(other))),
    };
    let declared = {
        let frame = FrameWorld::from_world(world);
        let meta = frame.client_meta(client).ok_or("player has disconnected")?;
        meta.player_data.is_declared()
    };
    let decls = if declared {
        Vec::new()
    } else {
        player_data_fields(world)?
    };
    let mut frame = FrameWorld::from_world(world);
    let data = &mut frame.client_meta_mut(client).player_data;
    if !declared {
        data.declare(decls.iter().map(|(name, keyed)| crate::MatchDataDecl {
            name,
            keyed: *keyed,
        }))
        .map_err(|error| format!("{PLAYER_DATA_TABLE}: {error}"))?;
    }
    data.write(&name, key.as_deref(), value)
        .map_err(|error| error.to_string())?;
    Ok(Value::Undefined)
}

fn player_data_fields(world: &World) -> Result<Vec<(String, bool)>, String> {
    let tables = &world.resource::<Runtime>().tables;
    let fields =
        table(tables, PLAYER_DATA_TABLE).ok_or(format!("{PLAYER_DATA_TABLE} is not loaded"))?;
    (0..fields.rows)
        .map(|row| {
            let name = fields
                .cell(row, 0)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or(format!("{PLAYER_DATA_TABLE}: row {row} names no field"))?;
            let keyed = fields
                .cell(row, 1)
                .is_some_and(|kinds| !kinds.trim().is_empty());
            Ok((name.to_owned(), keyed))
        })
        .collect()
}

fn request(world: &mut World, args: &[Value], side: ScriptBotSide) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("expects a bot count and an archetype".into());
    }
    let count = u32::try_from(int(args, 0)?)
        .ok()
        .filter(|count| *count > 0)
        .ok_or("the bot count must be positive")?;
    let (archetype, skill) = archetype(world, &string(args, 1)?)?;
    world
        .resource_mut::<ScriptBots>()
        .request(ScriptBotRequest {
            count,
            side,
            archetype,
            skill,
        });
    Ok(Value::Undefined)
}

/// The ref as the waves table spells it with its aim skill, or why it names
/// no archetype there or gives it no positive skill.
fn archetype(world: &World, wanted: &str) -> Result<(Arc<str>, f32), String> {
    let tables = &world.resource::<Runtime>().tables;
    let settings =
        table(tables, SETTINGS_TABLE).ok_or(format!("{SETTINGS_TABLE} is not loaded"))?;
    let waves_name = table_search(settings, 0, "waves_table")
        .and_then(|row| settings.cell(row, 1))
        .filter(|name| !name.is_empty())
        .ok_or(format!("{SETTINGS_TABLE} names no waves_table"))?;
    let waves = table(tables, waves_name).ok_or(format!("{waves_name} is not loaded"))?;
    (0..waves.rows)
        .filter(|&row| {
            waves
                .cell(row, 0)
                .and_then(|index| index.parse::<u32>().ok())
                .is_some_and(|index| ARCHETYPE_ROWS.contains(&index))
        })
        .find_map(|row| {
            waves
                .cell(row, 1)
                .filter(|r| r.eq_ignore_ascii_case(wanted))
                .map(|r| (row, r))
        })
        .ok_or_else(|| format!("`{wanted}` is no archetype of {waves_name}"))
        .and_then(|(row, r)| {
            let cell = waves.cell(row, ARCHETYPE_SKILL_COLUMN);
            cell.and_then(|cell| cell.trim().parse::<f32>().ok())
                .filter(|skill| skill.is_finite() && *skill > 0.0)
                .map(|skill| (Arc::from(r), skill))
                .ok_or_else(|| {
                    format!(
                        "archetype `{r}` of {waves_name} has no positive skill in column \
                         {ARCHETYPE_SKILL_COLUMN}: {cell:?}"
                    )
                })
        })
}
