use std::{collections::BTreeMap, sync::Arc};

use structured_data_iw4::{DataType, Definition, DefinitionSet, Error, Key, Lookup, Value};

use crate::PersistentDataError;

#[derive(Clone, Debug)]
pub struct PlayerDataDefaults {
    /// One raw name per element of the schema's `customClasses` array.
    pub class_names: Vec<Vec<u8>>,
    pub config: String,
}

/// Why a new account's `customClasses` names cannot be laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomClassesError {
    /// The zones carry no player-data schema, so there are no accounts.
    MissingSchema,
    /// The player-data schema is loaded but has no definition with an
    /// indexed `customClasses` array to name.
    Unlaid(PersistentDataError),
}

impl std::fmt::Display for CustomClassesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let schema = crate::persistent_data::PLAYER_SCHEMA;
        match self {
            Self::MissingSchema => write!(f, "no `{schema}` schema in the zones"),
            Self::Unlaid(error) => write!(
                f,
                "`{schema}` cannot lay out an indexed `customClasses` array ({error:?})"
            ),
        }
    }
}

/// The element count of the `customClasses` array in the schema a new
/// account is initialized from.
pub fn custom_class_count(
    schemas: &BTreeMap<String, Arc<DefinitionSet>>,
) -> Result<usize, CustomClassesError> {
    let definition =
        crate::persistent_data::initial_definition(schemas).map_err(|error| match error {
            PersistentDataError::MissingSchema => CustomClassesError::MissingSchema,
            error => CustomClassesError::Unlaid(error),
        })?;
    crate::persistent_data::array_count(definition, &[Key::Name("customClasses")])
        .map_err(CustomClassesError::Unlaid)
}

pub(crate) fn initialize(
    definition: &Definition,
    defaults: &PlayerDataDefaults,
) -> Result<Vec<u8>, PersistentDataError> {
    if definition.size < 8 {
        return Err(PersistentDataError::InvalidBuffer);
    }
    let mut bytes = vec![0; definition.size];
    bytes[..4].copy_from_slice(&definition.version.to_le_bytes());
    bytes[4..8].copy_from_slice(&definition.checksum.to_le_bytes());
    let mut dirty = crate::persistent_data::dirty_mask(definition);
    for (index, name) in defaults.class_names.iter().enumerate() {
        let keys = [
            Key::Name("customClasses"),
            Key::Index(index as i32),
            Key::Name("name"),
        ];
        definition.write(
            &mut bytes,
            &mut dirty,
            definition.lookup(&keys)?,
            Value::Bytes(name),
        )?;
    }
    for command in commands(&defaults.config)? {
        let Some(root) = command
            .first()
            .and_then(|verb| command_root(definition, verb))
            .filter(|_| command.len() >= 3)
        else {
            return Err(PersistentDataError::InvalidDefaults);
        };
        let path: Vec<String> = root
            .map(str::to_owned)
            .into_iter()
            .chain(command[1..command.len() - 1].iter().cloned())
            .collect();
        let lookup = lookup(definition, &path)?;
        let text = command.last().unwrap().split('\0').next().unwrap();
        let value = match lookup.ty {
            DataType::String(_) | DataType::Enum(_) => Value::String(text),
            DataType::Int | DataType::Byte | DataType::Short => Value::Int(
                text.parse()
                    .map_err(|_| PersistentDataError::InvalidDefaults)?,
            ),
            DataType::Bool => Value::Bool(
                text.parse::<i32>()
                    .map_err(|_| PersistentDataError::InvalidDefaults)?
                    != 0,
            ),
            DataType::Float => Value::Float(
                text.parse()
                    .map_err(|_| PersistentDataError::InvalidDefaults)?,
            ),
            _ => return Err(PersistentDataError::InvalidDefaults),
        };
        definition.write(&mut bytes, &mut dirty, lookup, value)?;
    }
    Ok(bytes)
}

/// The schema path a defaults verb writes under: `setPlayerData` addresses
/// the root, and `set<Field>` (MW3's `setCustomClasses`,
/// `setPrivateMatchCustomClasses`) addresses the root field it names.
fn command_root<'a>(definition: &'a Definition, verb: &str) -> Option<Option<&'a str>> {
    if verb.eq_ignore_ascii_case("setPlayerData") {
        return Some(None);
    }
    let field = verb
        .get(..3)?
        .eq_ignore_ascii_case("set")
        .then(|| &verb[3..])?;
    let DataType::Struct(root) = definition.root else {
        return None;
    };
    definition
        .structs
        .get(root)?
        .properties
        .iter()
        .find(|property| property.name.eq_ignore_ascii_case(field))
        .map(|property| Some(property.name.as_str()))
}

fn lookup(definition: &Definition, tokens: &[String]) -> Result<Lookup, Error> {
    let mut keys = Vec::with_capacity(tokens.len());
    for token in tokens {
        keys.push(Key::Name(token));
        if definition.lookup(&keys) == Err(Error::WrongKey) {
            let index = token
                .split('\0')
                .next()
                .unwrap()
                .parse()
                .map_err(|_| Error::WrongKey)?;
            *keys.last_mut().unwrap() = Key::Index(index);
        }
        match definition.lookup(&keys) {
            Ok(_) | Err(Error::MissingKey) => {}
            Err(error) => return Err(error),
        }
    }
    definition.lookup(&keys)
}

fn commands(source: &str) -> Result<Vec<Vec<String>>, PersistentDataError> {
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut present = false;
    let mut quoted = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            match ch {
                '"' => quoted = false,
                '\\' if chars.peek().is_some_and(|ch| matches!(ch, '\\' | '"')) => {
                    word.push(chars.next().unwrap())
                }
                _ => word.push(ch),
            }
            continue;
        }
        match ch {
            '"' => {
                quoted = true;
                present = true;
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                while chars.peek().is_some_and(|ch| !matches!(ch, '\n' | '\r')) {
                    chars.next();
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                if present {
                    words.push(std::mem::take(&mut word));
                    present = false;
                }
                chars.next();
                let mut closed = false;
                while let Some(ch) = chars.next() {
                    if ch == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err(PersistentDataError::InvalidDefaults);
                }
            }
            ch if ch.is_whitespace() || ch == ';' => {
                if present {
                    words.push(std::mem::take(&mut word));
                    present = false;
                }
                if matches!(ch, '\n' | '\r' | ';') && !words.is_empty() {
                    commands.push(std::mem::take(&mut words));
                }
            }
            _ => {
                word.push(ch);
                present = true;
            }
        }
    }
    if quoted {
        return Err(PersistentDataError::InvalidDefaults);
    }
    if present {
        words.push(word);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    Ok(commands)
}
