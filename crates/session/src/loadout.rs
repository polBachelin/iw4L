use asset_game::WeaponRegistry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoritativeClassProjection {
    pub def: sim::ClassDef,
    pub lock_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassRow {
    pub weapons: [String; 4],
    pub attachments: [Vec<String>; 2],
    pub perks: [String; 3],
    pub deathstreak: String,
    pub camos: [String; 2],
    pub weapon_buffs: [String; 2],
}

/// The attachment rules for the weapon in `slot` (0 primary, 1 secondary) of
/// a class with perk 1 `perk1` and that weapon's buff `buff`; the sim checks
/// the same rule when the class is chosen.
pub fn weapon_loadout_rules(perk1: &str, slot: usize, buff: &str) -> asset_game::LoadoutRules {
    asset_game::LoadoutRules {
        max_attachments: sim::match_state::weapon_attachment_limit(perk1, slot, buff),
    }
}

pub fn resolve_class_weapon(
    weapons: &WeaponRegistry,
    name: &str,
    attachments: &[String],
    rules: asset_game::LoadoutRules,
) -> Result<u32, String> {
    if name.is_empty() {
        return if attachments.is_empty() {
            Ok(0)
        } else {
            Err("weapon.unknown_family".to_owned())
        };
    }
    let resolve = |selection: asset_game::WeaponSelection| {
        weapons
            .resolve_configuration(&selection, rules)
            .map(|resolved| {
                weapons
                    .bind(resolved.handle())
                    .expect("resolved in this registry")
                    .wire_id()
            })
            .map_err(|refusal| format!("{name}:{} — {refusal}", refusal.code()))
    };
    if let Some(family) = asset_game::FamilyKey::parse(name)
        && weapons.weapon_families().family(&family).is_some()
    {
        return resolve(asset_game::WeaponSelection::with(family, attachments));
    }
    let id = match weapons.resolve_index(name) {
        Ok(Some(id)) => id,
        Ok(None) | Err(_) => return Err(format!("{name}:catalog.unknown")),
    };
    match weapons.describe_configuration(id) {
        Some(described) if described.family.is_some() => {
            let mut selection = described.clone();
            for extra in attachments {
                if !selection.attachments.contains(extra) {
                    selection.attachments.push(extra.clone());
                }
            }
            resolve(selection)
        }
        _ if attachments.is_empty() => weapons
            .configuration_admission(id)
            .map(|()| id)
            .map_err(|refusal| format!("{name}:{} — {refusal}", refusal.code())),
        _ => Err(format!("{name}:weapon.unknown_family")),
    }
}

pub(crate) fn authoritative_class_lock_reason(
    names: [&str; 4],
    ids: [u32; 4],
    content: &sim::SimWeaponContent,
) -> Option<String> {
    let mut reason = None;
    for (name, id) in names.iter().zip(ids) {
        if id != 0 && !content.is_runnable(id) {
            append_lock_reason(
                &mut reason,
                format!(
                    "{name}:execution.refused: {}",
                    content
                        .execution_refusal(id)
                        .unwrap_or("weapon content missing")
                ),
            );
        }
    }
    let combat = content.combat();
    let equipment = content.equipment();
    for (name, id) in names[..2].iter().zip(&ids[..2]) {
        if *id == 0 {
            continue;
        }
        if !combat
            .get(*id as usize)
            .copied()
            .is_some_and(weapon_iw4::WeaponCombatFacts::is_usable)
        {
            append_lock_reason(&mut reason, format!("{name}:validated.missing_profile"));
        }
    }
    for (slot, (name, id)) in names[2..].iter().zip(&ids[2..]).enumerate() {
        if *id == 0 {
            continue;
        }
        if !equipment.get(*id as usize).copied().is_some_and(|facts| {
            facts.is_offhand() && matches!((slot, facts.offhand_class), (0, 1 | 4 | 5) | (1, 2 | 3))
        }) {
            append_lock_reason(&mut reason, format!("{name}:offhand.missing_runtime_facts"));
        }
    }
    reason
}

fn append_lock_reason(reason: &mut Option<String>, item: String) {
    match reason {
        Some(reason) => {
            reason.push_str("; ");
            reason.push_str(&item);
        }
        None => *reason = Some(item),
    }
}

/// The class catalog of runtime profile `profile`. Under IW5 it is MW3's own
/// perk and camo tables, ids being the tables' row indices; under IW4 it is
/// the built-in MW2 catalog, which no MW2 table on hand has checked.
pub fn class_catalog(
    registry: &WeaponRegistry,
    profile: asset_transport::RuntimeProfile,
) -> sim::ClassCatalog {
    match profile {
        asset_transport::RuntimeProfile::Iw4 => sim::ClassCatalog::iw4(),
        asset_transport::RuntimeProfile::Iw5 => {
            let namespace = asset_core::AssetNamespace::Iw5;
            let Some(tables) = registry.weapon_families().class_tables(namespace) else {
                return sim::ClassCatalog::from_entries("iw5 (no perk table loaded)", [], []);
            };
            let entries = tables.perks.iter().filter_map(|row| {
                let slot = match row.slot {
                    asset_game::CacPerkSlot::Perk1 => sim::ClassCatalogSlot::Perk(0),
                    asset_game::CacPerkSlot::Perk2 => sim::ClassCatalogSlot::Perk(1),
                    asset_game::CacPerkSlot::Perk3 => sim::ClassCatalogSlot::Perk(2),
                    asset_game::CacPerkSlot::Deathstreak => sim::ClassCatalogSlot::Deathstreak,
                    asset_game::CacPerkSlot::Proficiency => {
                        sim::ClassCatalogSlot::WeaponBuff([true; 2])
                    }
                    asset_game::CacPerkSlot::Equipment
                    | asset_game::CacPerkSlot::StrikePackage
                    | asset_game::CacPerkSlot::Upgrade(_)
                    | asset_game::CacPerkSlot::Hidden => return None,
                };
                Some(sim::ClassCatalogEntry {
                    id: row.index,
                    name: row.reference.clone(),
                    slot,
                })
            });
            let source = match tables.camo_table.as_str() {
                "" => format!("{} {}", namespace.as_str(), tables.perk_table),
                camo => format!("{} {} + {camo}", namespace.as_str(), tables.perk_table),
            };
            sim::ClassCatalog::from_entries(source, entries, tables.camos.iter().cloned())
        }
    }
}

/// A game's authored default class as a host class slot, with the perks and
/// deathstreak the class catalog has no entry for in that slot. Those are
/// left empty in the slot and named in the gaps for the caller to report:
/// the catalog is the ruleset's, so a perk outside it cannot be carried.
pub fn authored_class_slot(
    class: &asset_game::AuthoredClass,
    catalog: &sim::ClassCatalog,
) -> (frame::HostClassSlot, Vec<String>) {
    let mut gaps = Vec::new();
    let perks = std::array::from_fn(|slot| {
        let name = &class.perks[slot];
        let fits = catalog
            .perk_id(name)
            .is_some_and(|id| catalog.perk_slot(id) == Some(slot));
        if name.is_empty() || fits {
            name.clone()
        } else {
            gaps.push(format!("perk{}={name}", slot + 1));
            String::new()
        }
    });
    let deathstreak =
        if class.deathstreak.is_empty() || catalog.deathstreak_id(&class.deathstreak).is_some() {
            class.deathstreak.clone()
        } else {
            gaps.push(format!("deathstreak={}", class.deathstreak));
            String::new()
        };
    let weapon_buffs = std::array::from_fn(|slot| {
        let name = &class.weapon_buffs[slot];
        if name.is_empty() || catalog.weapon_buff_id(slot, name).is_some() {
            name.clone()
        } else {
            gaps.push(format!("buff{}={name}", slot + 1));
            String::new()
        }
    });
    let slot = frame::HostClassSlot {
        name: class.name.clone(),
        primary: class.primary.clone(),
        primary_attachments: class.primary_attachments.clone(),
        secondary: class.secondary.clone(),
        secondary_attachments: class.secondary_attachments.clone(),
        lethal: class.lethal.clone(),
        tactical: class.tactical.clone(),
        perks,
        deathstreak,
        camos: Default::default(),
        weapon_buffs,
    };
    (slot, gaps)
}

pub fn project_class(
    class_id: u32,
    row: &ClassRow,
    weapons: &WeaponRegistry,
    compiled: &crate::PreparedSimWeapons,
    catalog: &sim::ClassCatalog,
) -> AuthoritativeClassProjection {
    let content = compiled.content();
    let resolved = if compiled.owned_by(weapons) {
        resolve_personal_class(row, weapons, catalog)
    } else {
        Err("weapon.foreign_content".to_owned())
    }
    .and_then(|loadout| {
        loadout
            .definition(sim::ClassId(class_id), 1, catalog)
            .ok_or_else(|| "class.invalid_definition".to_owned())
    });
    let (mut def, mut lock_reason) = match resolved {
        Ok(def) => (def, None),
        Err(reason) => (
            sim::ClassDef::primary_secondary(sim::ClassId(class_id), 1, 0, 0),
            Some(reason),
        ),
    };
    let names = row.weapons.each_ref().map(String::as_str);
    if let Some(reason) = authoritative_class_lock_reason(names, def.weapon_slot_ids(), content) {
        append_lock_reason(&mut lock_reason, reason);
    }
    def.locked = lock_reason.is_some();
    AuthoritativeClassProjection { def, lock_reason }
}

impl From<&frame::HostClassSlot> for ClassRow {
    fn from(slot: &frame::HostClassSlot) -> Self {
        Self {
            weapons: [
                slot.primary.clone(),
                slot.secondary.clone(),
                slot.lethal.clone(),
                slot.tactical.clone(),
            ],
            attachments: [
                slot.primary_attachments.clone(),
                slot.secondary_attachments.clone(),
            ],
            perks: slot.perks.clone(),
            deathstreak: slot.deathstreak.clone(),
            camos: slot.camos.clone(),
            weapon_buffs: slot.weapon_buffs.clone(),
        }
    }
}

pub fn resolve_personal_class(
    row: &ClassRow,
    registry: &WeaponRegistry,
    catalog: &sim::ClassCatalog,
) -> Result<sim::PersonalClass, String> {
    let mut loadout = sim::PersonalClass::default();
    for (slot, name) in row.weapon_buffs.iter().enumerate() {
        if !name.is_empty() {
            loadout.weapon_buffs[slot] = catalog
                .weapon_buff_id(slot, name)
                .ok_or_else(|| format!("{name}:buff.unknown_catalog_entry"))?;
        }
    }
    for (slot, weapon) in loadout.weapons.iter_mut().enumerate() {
        let buff = row.weapon_buffs.get(slot).map_or("", String::as_str);
        *weapon = resolve_class_weapon(
            registry,
            &row.weapons[slot],
            row.attachments.get(slot).map_or(&[], Vec::as_slice),
            weapon_loadout_rules(&row.perks[0], slot, buff),
        )?;
    }
    for (slot, camo) in row.camos.iter().enumerate() {
        loadout.camos[slot] = registry
            .camouflage_slot(loadout.weapons[slot], camo)
            .ok_or_else(|| format!("{camo}:camouflage.unavailable"))?;
    }
    for (slot, name) in row.perks.iter().enumerate() {
        if name.is_empty() || name == "specialty_null" {
            continue;
        }
        let id = catalog
            .perk_id(name)
            .ok_or_else(|| format!("{name}:perk.unknown_catalog_entry"))?;
        if catalog.perk_slot(id) != Some(slot) {
            return Err(format!("{name}:perk.invalid_slot"));
        }
        loadout.perks[slot] = id;
    }
    if !row.deathstreak.is_empty() && row.deathstreak != "specialty_null" {
        loadout.deathstreak = catalog
            .deathstreak_id(&row.deathstreak)
            .ok_or_else(|| format!("{}:deathstreak.unknown_catalog_entry", row.deathstreak))?;
    }
    Ok(loadout)
}
