//! The survival gametype's natives, linked only into a survival program
//! (`Catalog::survival`). The script writes the player data MW3's armory
//! menus read into the client's match player data, and dresses entities in
//! named characters.

use super::super::args::{arg, kind, optional, string};
use super::super::tables::{table, table_search};
use super::engine::entity_id;
use super::player::player;
use crate::ClientId;
use crate::frame::FrameWorld;
use crate::script::Namespace::Method;
use crate::script::{Arc, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;

/// Survival's own settings table; its `waves_table` row names MW3's table.
const SETTINGS_TABLE: &str = "survival/settings.csv";

/// The player data fields the armory menus read: c0 the field, c1 the armory
/// kinds whose item refs key it (empty for a plain number). With the schema
/// `player_data_def` names loaded, the schema says which field is keyed and
/// by what; c1 is read only without it.
const PLAYER_DATA_TABLE: &str = "survival/playerdata.csv";

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Method, "survival_setplayerdata", set_player_data);
    registry.register(Method, "setcharacter", set_character);
}

/// `self setcharacter( body, [head,] [hat] )`: the models the entity is drawn
/// in instead of its team's kit. Names are checked against the bodies only
/// where they are drawn.
fn set_character(world: &mut World, receiver: &Value, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("expects a body, an optional head and an optional hat".into());
    }
    let name = |index| -> Result<Option<String>, String> {
        let name = optional(args, index, string)?.filter(|name| !name.is_empty());
        match name {
            Some(name) if name.len() > usize::from(u8::MAX) => {
                Err(format!("model name `{name}` is longer than 255 bytes"))
            }
            name => Ok(name),
        }
    };
    let selection = crate::CharacterSelection {
        body: name(0)?.ok_or("the body model name is empty")?,
        head: name(1)?,
        hat: name(2)?,
    };
    let id = entity_id(world, receiver)?;
    super::actor::dress(world, id, &selection);
    let entnum = world.resource::<Runtime>().entities[&id].number;
    FrameWorld::from_world(world).set_character(entnum, selection);
    Ok(Value::Undefined)
}

/// `self survival_setplayerdata( field, [key,] value )`: a whole number into
/// the client's match player data, which `PLAYER_DATA_TABLE` declares on the
/// first write of a match. A field it does not declare, a key on a plain
/// field (or none on a keyed one), or a key the schema's enum does not have,
/// faults the script.
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
    let schema = player_data_schema(world)?;
    if let (Some((def_name, set)), Some(key)) = (&schema, &key) {
        use structured_data_iw4::Key::Name;
        newest(set)
            .ok_or(format!("{def_name} has no definition"))?
            .lookup(&[Name(&name), Name(key)])
            .map_err(|error| format!("`{name}` `{key}` is no number of {def_name}: {error:?}"))?;
    }
    let decls = if declared {
        Vec::new()
    } else {
        let decls = player_data_fields(world, schema.as_ref())?;
        diag::info!(
            Sim,
            "survival: client {} match player data: {} fields, shaped by {}",
            client.0,
            decls.len(),
            schema
                .as_ref()
                .map_or(PLAYER_DATA_TABLE, |(name, _)| name.as_str())
        );
        decls
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

/// The schema `player_data_def` names, by that name, when a loaded zone
/// carries it.
fn player_data_schema(
    world: &World,
) -> Result<Option<(String, Arc<structured_data_iw4::DefinitionSet>)>, String> {
    let tables = &world.resource::<Runtime>().tables;
    let settings =
        table(tables, SETTINGS_TABLE).ok_or(format!("{SETTINGS_TABLE} is not loaded"))?;
    let name = table_search(settings, 0, "player_data_def")
        .and_then(|row| settings.cell(row, 1))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or(format!("{SETTINGS_TABLE} names no player_data_def"))?;
    let schemas = world.resource::<crate::PersistentDataStore>().schemas();
    Ok(schemas
        .get(&name.to_ascii_lowercase())
        .filter(|set| newest(set).is_some())
        .map(|set| (name.to_owned(), set.clone())))
}

fn newest(set: &structured_data_iw4::DefinitionSet) -> Option<&structured_data_iw4::Definition> {
    set.definitions
        .iter()
        .max_by_key(|definition| definition.version)
}

/// Whether the schema keys `field` (numbers by the entries of an enum) or
/// holds it as one number; any other shape is no field the store keeps.
fn schema_keyed(definition: &structured_data_iw4::Definition, field: &str) -> Result<bool, String> {
    use structured_data_iw4::DataType::{Bool, Byte, EnumArray, Int, Short};
    let number = |ty| matches!(ty, Int | Byte | Short | Bool);
    match definition.path_type(&[structured_data_iw4::Key::Name(field)]) {
        Ok(ty) if number(ty) => Ok(false),
        Ok(EnumArray(array))
            if definition
                .enum_arrays
                .get(array)
                .is_some_and(|array| number(array.element)) =>
        {
            Ok(true)
        }
        Ok(ty) => Err(format!(
            "`{field}` is {ty:?}, neither a number nor numbers by key"
        )),
        Err(error) => Err(format!("`{field}` is no field: {error:?}")),
    }
}

fn player_data_fields(
    world: &World,
    schema: Option<&(String, Arc<structured_data_iw4::DefinitionSet>)>,
) -> Result<Vec<(String, bool)>, String> {
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
            let keyed = match schema {
                Some((def_name, set)) => newest(set)
                    .ok_or(format!("{def_name} has no definition"))
                    .and_then(|definition| schema_keyed(definition, name))
                    .map_err(|error| format!("{PLAYER_DATA_TABLE}: {def_name}: {error}"))?,
                None => (1..=2).any(|column| {
                    fields
                        .cell(row, column)
                        .is_some_and(|keys| !keys.trim().is_empty())
                }),
            };
            Ok((name.to_owned(), keyed))
        })
        .collect()
}
