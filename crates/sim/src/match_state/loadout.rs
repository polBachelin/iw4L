use crate::input::ClassId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassRejectReason {
    UnknownOrStaleClass,

    NoSpawnAvailable,

    LockedContent,

    UnknownWeaponId,
}

impl ClassRejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownOrStaleClass => "unknown_or_stale_class",
            Self::NoSpawnAvailable => "no_spawn_available",
            Self::LockedContent => "locked_content",
            Self::UnknownWeaponId => "unknown_weapon_id",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveRejectReason {
    NotAlive,

    InvalidWeapon,

    UnknownWeaponId,

    UnsupportedWeapon,

    EmptyCombatProfile,
}

impl GiveRejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAlive => "not_alive",
            Self::InvalidWeapon => "invalid_weapon",
            Self::UnknownWeaponId => "unknown_weapon_id",
            Self::UnsupportedWeapon => "unsupported_weapon",
            Self::EmptyCombatProfile => "empty_combat_profile",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationChangeRejectReason {
    NotAlive,
    StaleSource,
    InvalidTarget,
    DifferentFamily,
    Busy,
    NoInventorySlot,
    AmmoTableFull,
    SharedAmmoConflict,
}

impl ConfigurationChangeRejectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAlive => "not_alive",
            Self::StaleSource => "stale_source",
            Self::InvalidTarget => "invalid_target",
            Self::DifferentFamily => "different_family",
            Self::Busy => "busy",
            Self::NoInventorySlot => "no_inventory_slot",
            Self::AmmoTableFull => "ammo_table_full",
            Self::SharedAmmoConflict => "shared_ammo_conflict",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadoutSpec {
    pub class_id: ClassId,
    pub revision: u32,
    pub primary: u32,
    pub secondary: u32,
    pub primary_attachments: [u32; 4],
    pub secondary_attachments: [u32; 4],
    pub lethal: u32,
    pub tactical: u32,

    pub perks: [u32; 3],
}

pub const PERSONAL_CLASS_SLOTS: usize = 5;

/// The perk-1 choice that gives both weapons a second attachment (MW2).
const SECOND_ATTACHMENT_PERK1: &str = "specialty_bling";
/// The buff that gives its own weapon a second attachment, per slot (MW3).
const SECOND_ATTACHMENT_BUFFS: [&str; 2] = ["specialty_bling", "specialty_secondarybling"];

/// How many attachments the weapon in `slot` (0 primary, 1 secondary) may
/// carry for a class with perk 1 `perk1` and that weapon's buff `buff`.
/// MW2 grants the second attachment to both weapons through perk 1; MW3
/// grants it per weapon through the weapon's buff. Each game's classes only
/// ever fill the field its own rule reads.
pub fn weapon_attachment_limit(perk1: &str, slot: usize, buff: &str) -> usize {
    let bling = perk1.eq_ignore_ascii_case(SECOND_ATTACHMENT_PERK1)
        || SECOND_ATTACHMENT_BUFFS
            .get(slot)
            .is_some_and(|granting| buff.eq_ignore_ascii_case(granting));
    if bling { 2 } else { 1 }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PersonalClass {
    pub weapons: [u32; 4],
    pub perks: [u32; 3],
    pub deathstreak: u8,
    pub camos: [u8; 2],
    /// Primary and secondary buff, as class catalog ids (0 = none).
    pub weapon_buffs: [u8; 2],
}

impl PersonalClass {
    pub fn definition(
        self,
        id: ClassId,
        revision: u32,
        catalog: &ClassCatalog,
    ) -> Option<ClassDef> {
        if revision == 0 {
            return None;
        }
        let mut def = ClassDef::primary_secondary(id, revision, self.weapons[0], self.weapons[1]);
        def.lethal = self.weapons[2];
        def.tactical = self.weapons[3];
        def.camos = self.camos;
        def.perks = self.perks;
        def.weapon_buffs = self.weapon_buffs;
        def.deathstreak = if self.deathstreak == 0 {
            String::new()
        } else {
            catalog.deathstreak_name(self.deathstreak)?.to_owned()
        };
        Some(def)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassDef {
    pub id: ClassId,
    pub revision: u32,
    pub primary: u32,
    pub secondary: u32,

    pub primary_attachments: [u32; 4],
    pub secondary_attachments: [u32; 4],
    pub lethal: u32,
    pub tactical: u32,
    pub perks: [u32; 3],

    pub deathstreak: String,
    pub camos: [u8; 2],
    /// Primary and secondary buff, as class catalog ids (0 = none).
    pub weapon_buffs: [u8; 2],
    pub locked: bool,
}

impl ClassDef {
    pub fn primary_secondary(id: ClassId, revision: u32, primary: u32, secondary: u32) -> Self {
        Self {
            id,
            revision,
            primary,
            secondary,
            primary_attachments: [0; 4],
            secondary_attachments: [0; 4],
            lethal: 0,
            tactical: 0,
            perks: [0; 3],
            deathstreak: String::new(),
            camos: [0; 2],
            weapon_buffs: [0; 2],
            locked: false,
        }
    }

    pub fn weapon_slot_ids(&self) -> [u32; 4] {
        [self.primary, self.secondary, self.lethal, self.tactical]
    }
}

/// The class-editor slot a class catalog entry fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassCatalogSlot {
    /// Perk 1, 2 or 3, 0-based.
    Perk(u8),
    Deathstreak,
    /// A weapon buff, with the weapon slots (primary, secondary) it may sit in.
    WeaponBuff([bool; 2]),
}

impl ClassCatalogSlot {
    /// A stable byte for the content digest.
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Perk(slot) => slot,
            Self::Deathstreak => 8,
            Self::WeaponBuff([primary, secondary]) => {
                16 | u8::from(primary) | u8::from(secondary) << 1
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassCatalogEntry {
    /// The id a class carries on the wire (0 = none).
    pub id: u32,
    pub name: String,
    pub slot: ClassCatalogSlot,
}

/// The perks, deathstreaks, weapon buffs and camouflages a class may name,
/// with the ids a class carries for them. The ruleset's: a choice outside it
/// has no entry, so a class cannot carry it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassCatalog {
    source: String,
    entries: Vec<ClassCatalogEntry>,
    camos: Vec<(u8, String)>,
}

/// MW2's class perks with their perk slot, in id order from 1.
const IW4_PERKS: [(&str, u8); 16] = [
    ("specialty_bulletdamage", 1),
    ("specialty_fastreload", 0),
    ("specialty_coldblooded", 1),
    ("specialty_lightweight", 1),
    ("specialty_scavenger", 0),
    ("specialty_hardline", 1),
    ("specialty_heartbreaker", 2),
    ("specialty_marathon", 0),
    ("specialty_explosivedamage", 1),
    ("specialty_extendedmelee", 2),
    ("specialty_bulletaccuracy", 2),
    ("specialty_bling", 0),
    ("specialty_onemanarmy", 0),
    ("specialty_localjammer", 2),
    ("specialty_detectexplosive", 2),
    ("specialty_pistoldeath", 2),
];

/// MW2's deathstreaks, in id order from 1.
const IW4_DEATHSTREAKS: [&str; 4] = [
    "specialty_copycat",
    "specialty_combathigh",
    "specialty_grenadepulldeath",
    "specialty_finalstand",
];

impl ClassCatalog {
    /// The MW2 class catalog. Its ids are positions in these lists, not rows
    /// of a table this runtime reads.
    pub fn iw4() -> Self {
        let perks = IW4_PERKS
            .iter()
            .zip(1..)
            .map(|(&(name, slot), id)| (id, name, ClassCatalogSlot::Perk(slot)));
        let deathstreaks = IW4_DEATHSTREAKS
            .iter()
            .zip(1..)
            .map(|(&name, id)| (id, name, ClassCatalogSlot::Deathstreak));
        let buffs = SECOND_ATTACHMENT_BUFFS
            .iter()
            .enumerate()
            .map(|(slot, &name)| {
                let mut weapons = [false; 2];
                weapons[slot] = true;
                (1, name, ClassCatalogSlot::WeaponBuff(weapons))
            });
        Self::from_entries(
            "iw4 built-in",
            perks
                .chain(deathstreaks)
                .chain(buffs)
                .map(|(id, name, slot)| ClassCatalogEntry {
                    id,
                    name: name.to_owned(),
                    slot,
                }),
            weapon_iw4::IW4_CAMOS
                .iter()
                .zip(0..)
                .map(|(&name, id)| (id, name.to_owned())),
        )
    }

    /// A catalog read from a game's tables; `source` names them in reports.
    pub fn from_entries(
        source: impl Into<String>,
        entries: impl IntoIterator<Item = ClassCatalogEntry>,
        camos: impl IntoIterator<Item = (u8, String)>,
    ) -> Self {
        Self {
            source: source.into(),
            entries: entries.into_iter().filter(|entry| entry.id != 0).collect(),
            camos: camos.into_iter().collect(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn entries(&self) -> &[ClassCatalogEntry] {
        &self.entries
    }

    pub fn camos(&self) -> &[(u8, String)] {
        &self.camos
    }

    /// One line for the logs: where the catalog came from and what it holds.
    pub fn summary(&self) -> String {
        let count = |kind: fn(ClassCatalogSlot) -> bool| {
            self.entries.iter().filter(|entry| kind(entry.slot)).count()
        };
        format!(
            "{} perks={} deathstreaks={} buffs={} camos={}",
            self.source,
            count(|slot| matches!(slot, ClassCatalogSlot::Perk(_))),
            count(|slot| slot == ClassCatalogSlot::Deathstreak),
            count(|slot| matches!(slot, ClassCatalogSlot::WeaponBuff(_))),
            self.camos.iter().filter(|(id, _)| *id != 0).count(),
        )
    }

    fn find(&self, id: u32, fits: impl Fn(ClassCatalogSlot) -> bool) -> Option<&ClassCatalogEntry> {
        (id != 0)
            .then(|| {
                self.entries
                    .iter()
                    .find(|entry| entry.id == id && fits(entry.slot))
            })
            .flatten()
    }

    fn find_named(
        &self,
        name: &str,
        fits: impl Fn(ClassCatalogSlot) -> bool,
    ) -> Option<&ClassCatalogEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name) && fits(entry.slot))
    }

    /// The perk `id` names, whichever perk slot it belongs to.
    pub fn perk_name(&self, id: u32) -> Option<&str> {
        self.find(id, |slot| matches!(slot, ClassCatalogSlot::Perk(_)))
            .map(|entry| entry.name.as_str())
    }

    /// The perk slot (0-based) of perk `id`.
    pub fn perk_slot(&self, id: u32) -> Option<usize> {
        match self
            .find(id, |slot| matches!(slot, ClassCatalogSlot::Perk(_)))?
            .slot
        {
            ClassCatalogSlot::Perk(slot) => Some(usize::from(slot)),
            _ => None,
        }
    }

    pub fn perk_id(&self, name: &str) -> Option<u32> {
        self.find_named(name, |slot| matches!(slot, ClassCatalogSlot::Perk(_)))
            .map(|entry| entry.id)
    }

    pub fn deathstreak_name(&self, id: u8) -> Option<&str> {
        self.find(u32::from(id), |slot| slot == ClassCatalogSlot::Deathstreak)
            .map(|entry| entry.name.as_str())
    }

    pub fn deathstreak_id(&self, name: &str) -> Option<u8> {
        self.find_named(name, |slot| slot == ClassCatalogSlot::Deathstreak)
            .and_then(|entry| u8::try_from(entry.id).ok())
    }

    /// The buff `id` names for the weapon in `slot` (0 primary, 1 secondary).
    pub fn weapon_buff_name(&self, slot: usize, id: u8) -> Option<&str> {
        self.find(u32::from(id), |kind| Self::buff_fits(kind, slot))
            .map(|entry| entry.name.as_str())
    }

    pub fn weapon_buff_id(&self, slot: usize, name: &str) -> Option<u8> {
        self.find_named(name, |kind| Self::buff_fits(kind, slot))
            .and_then(|entry| u8::try_from(entry.id).ok())
    }

    fn buff_fits(kind: ClassCatalogSlot, slot: usize) -> bool {
        matches!(kind, ClassCatalogSlot::WeaponBuff(weapons) if weapons.get(slot) == Some(&true))
    }

    /// The camouflage name of camo id `id`; id 0 is no camouflage.
    pub fn camo_name(&self, id: u8) -> Option<&str> {
        self.camos
            .iter()
            .find(|(own, _)| *own == id)
            .map(|(_, name)| name.as_str())
    }
}
