//! Player data a gametype keeps for one client for the length of a match.
//!
//! Survival's armory menus read the player through `getPlayerData`, as MW3's
//! Special Ops did from SP player data. The user's zones define no such
//! schema, so the survival script declares the fields the menus read
//! (`survival/playerdata.csv`) and writes them here. The store lives in the
//! client's match state, travels in its snapshot meta like its client dvars,
//! and is never written to disk: it starts empty on connect and on every map
//! restart.

/// One client's fields, in declaration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchPlayerData {
    pub fields: Vec<MatchDataField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchDataField {
    pub name: String,
    pub value: MatchDataValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchDataValue {
    /// A whole number the script has not written yet.
    Unset,
    Int(i32),
    /// Whole numbers by a second key (an armory item's ref). Only non-zero
    /// entries are kept: a key with no entry reads 0.
    ByKey(Vec<(String, i32)>),
}

/// A field as `survival/playerdata.csv` declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchDataDecl<'a> {
    pub name: &'a str,
    pub keyed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchDataError {
    /// No field of the store has this name.
    UnknownField(String),
    /// The path's length is not the field's: one name for a number, a name
    /// and a key for a keyed field.
    Arity { field: String, keyed: bool },
    /// The script has not written this number yet.
    Unset(String),
    /// The store holds no fields: nothing declared them for this client.
    Undeclared,
}

impl std::fmt::Display for MatchDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownField(name) => write!(f, "`{name}` is no field of the match player data"),
            Self::Arity { field, keyed: true } => {
                write!(f, "`{field}` is keyed: it takes one key")
            }
            Self::Arity {
                field,
                keyed: false,
            } => write!(f, "`{field}` takes no key"),
            Self::Unset(name) => write!(f, "`{name}` has not been set"),
            Self::Undeclared => f.write_str("no match player data is declared for this client"),
        }
    }
}

impl MatchPlayerData {
    pub fn is_declared(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Declares the fields, all unset or empty. A name declared twice is
    /// refused, so a path always names one field.
    pub fn declare<'a>(
        &mut self,
        decls: impl IntoIterator<Item = MatchDataDecl<'a>>,
    ) -> Result<(), String> {
        let mut fields: Vec<MatchDataField> = Vec::new();
        for decl in decls {
            if decl.name.is_empty() {
                return Err("a match player data field has no name".into());
            }
            if fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(decl.name))
            {
                return Err(format!(
                    "match player data field `{}` is declared twice",
                    decl.name
                ));
            }
            fields.push(MatchDataField {
                name: decl.name.to_owned(),
                value: if decl.keyed {
                    MatchDataValue::ByKey(Vec::new())
                } else {
                    MatchDataValue::Unset
                },
            });
        }
        if fields.is_empty() {
            return Err("no match player data field is declared".into());
        }
        self.fields = fields;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.fields.clear();
    }

    fn index(&self, name: &str) -> Result<usize, MatchDataError> {
        if !self.is_declared() {
            return Err(MatchDataError::Undeclared);
        }
        self.fields
            .iter()
            .position(|field| field.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| MatchDataError::UnknownField(name.to_owned()))
    }

    /// The number at `name` (and `key`, for a keyed field).
    pub fn read(&self, name: &str, key: Option<&str>) -> Result<i32, MatchDataError> {
        let field = &self.fields[self.index(name)?];
        match (&field.value, key) {
            (MatchDataValue::Unset, None) => Err(MatchDataError::Unset(field.name.clone())),
            (MatchDataValue::Int(value), None) => Ok(*value),
            (MatchDataValue::ByKey(entries), Some(key)) => Ok(entries
                .iter()
                .find(|(at, _)| at.eq_ignore_ascii_case(key))
                .map_or(0, |(_, value)| *value)),
            (value, _) => Err(MatchDataError::Arity {
                field: field.name.clone(),
                keyed: matches!(value, MatchDataValue::ByKey(_)),
            }),
        }
    }

    pub fn write(
        &mut self,
        name: &str,
        key: Option<&str>,
        value: i32,
    ) -> Result<(), MatchDataError> {
        let index = self.index(name)?;
        let field = &mut self.fields[index];
        match (&mut field.value, key) {
            (slot @ (MatchDataValue::Unset | MatchDataValue::Int(_)), None) => {
                *slot = MatchDataValue::Int(value);
            }
            (MatchDataValue::ByKey(entries), Some(key)) => {
                let at = entries
                    .iter()
                    .position(|(entry, _)| entry.eq_ignore_ascii_case(key));
                match (at, value) {
                    (Some(at), 0) => {
                        entries.remove(at);
                    }
                    (Some(at), value) => entries[at].1 = value,
                    (None, 0) => {}
                    (None, value) => entries.push((key.to_owned(), value)),
                }
            }
            (value, _) => {
                return Err(MatchDataError::Arity {
                    field: field.name.clone(),
                    keyed: matches!(value, MatchDataValue::ByKey(_)),
                });
            }
        }
        Ok(())
    }
}
