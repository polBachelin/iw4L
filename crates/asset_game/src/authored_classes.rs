//! The default classes a game authors in `mp/classTable.csv`.
//!
//! IW4 and IW5 lay the table out the same way: column 0 names a loadout field
//! (`loadoutPrimary`, `loadoutPerk1`, …) and every further column is one
//! class. The game's own class script reads it with
//! `tableLookup(table, 0, field, classIndex + 1)`.

use crate::{AssetNamespace, CapturedStringTable, FamilyKey};

#[must_use]
pub fn is_class_table_name(name: &str) -> bool {
    let n = name.replace('\\', "/");
    let leaf = n.rsplit('/').next().unwrap_or(&n);
    leaf.eq_ignore_ascii_case("classTable.csv")
}

/// One class column, with weapons as registry asset keys of the table's game.
/// An absent slot (`none`, `specialty_null`, empty) is an empty string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredClass {
    pub namespace: AssetNamespace,
    pub name: String,
    pub primary: String,
    pub primary_attachments: Vec<String>,
    pub secondary: String,
    pub secondary_attachments: Vec<String>,
    pub lethal: String,
    pub tactical: String,
    pub perks: [String; 3],
    pub deathstreak: String,
    /// Primary and secondary weapon buff (MW3 proficiency); IW4 tables have
    /// no buff rows.
    pub weapon_buffs: [String; 2],
}

const NAME: &str = "loadoutName";
const PRIMARY: &str = "loadoutPrimary";
const PRIMARY_ATTACHMENTS: [&str; 2] = ["loadoutPrimaryAttachment", "loadoutPrimaryAttachment2"];
const SECONDARY: &str = "loadoutSecondary";
const SECONDARY_ATTACHMENTS: [&str; 2] =
    ["loadoutSecondaryAttachment", "loadoutSecondaryAttachment2"];
const LETHAL: &str = "loadoutEquipment";
const TACTICAL: &str = "loadoutOffhand";
const PERKS: [&str; 3] = ["loadoutPerk1", "loadoutPerk2", "loadoutPerk3"];
const DEATHSTREAK: &str = "loadoutDeathStreak";
const WEAPON_BUFFS: [&str; 2] = ["loadoutPrimaryBuff", "loadoutSecondaryBuff"];

fn authored(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && !value.eq_ignore_ascii_case("none")
        && !value.eq_ignore_ascii_case("specialty_null"))
    .then_some(value)
}

#[must_use]
pub fn read_authored_classes(
    namespace: AssetNamespace,
    table: &CapturedStringTable,
) -> Vec<AuthoredClass> {
    let field = |name: &str, col: i32| -> Option<&str> {
        table
            .lookup_row(name)
            .and_then(|row| authored(table.cell(row, col)))
    };
    let weapon = |name: &str, col: i32| {
        field(name, col).map_or_else(String::new, |cell| {
            FamilyKey::new(namespace, cell).asset_key()
        })
    };
    let attachments = |names: [&str; 2], col: i32| -> Vec<String> {
        names
            .iter()
            .filter_map(|name| field(name, col))
            .map(str::to_ascii_lowercase)
            .collect()
    };
    let text = |name: &str, col: i32| field(name, col).map_or_else(String::new, str::to_owned);
    (1..table.columns as i32)
        .filter(|&col| field(PRIMARY, col).is_some())
        .map(|col| AuthoredClass {
            namespace,
            name: text(NAME, col),
            primary: weapon(PRIMARY, col),
            primary_attachments: attachments(PRIMARY_ATTACHMENTS, col),
            secondary: weapon(SECONDARY, col),
            secondary_attachments: attachments(SECONDARY_ATTACHMENTS, col),
            lethal: weapon(LETHAL, col),
            tactical: weapon(TACTICAL, col),
            perks: PERKS.map(|name| text(name, col)),
            deathstreak: text(DEATHSTREAK, col),
            weapon_buffs: WEAPON_BUFFS.map(|name| text(name, col)),
        })
        .collect()
}
