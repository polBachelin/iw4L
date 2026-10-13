use std::collections::{BTreeSet, HashSet};

use asset_core::AssetRef;

/// The asset kinds whose cross-zone references a load counts.
#[derive(Clone, Copy)]
pub(crate) enum RefKind {
    Model,
    Material,
    Fx,
    Sound,
}

const KINDS: [(RefKind, &str); 4] = [
    (RefKind::Model, "models"),
    (RefKind::Material, "materials"),
    (RefKind::Fx, "fx"),
    (RefKind::Sound, "sounds"),
];

/// What one zone holds of each kind, and what it names by reference to
/// another zone. Names are compared without case.
#[derive(Clone, Default)]
pub struct ZoneRefs {
    held: [HashSet<String>; KINDS.len()],
    named: [BTreeSet<String>; KINDS.len()],
}

impl ZoneRefs {
    pub(crate) fn note(&mut self, kind: RefKind, raw: &str) {
        let slot = kind as usize;
        match AssetRef::decode(&raw.to_ascii_lowercase()) {
            AssetRef::Reference(name) if !name.is_empty() => {
                self.named[slot].insert(name);
            }
            AssetRef::Real(name) if !name.is_empty() => {
                self.held[slot].insert(name);
            }
            _ => {}
        }
    }
}

/// Each walked zone's references, in load order.
#[derive(Clone, Default)]
pub struct ZoneRefCensus(Vec<(String, ZoneRefs)>);

impl ZoneRefCensus {
    pub(crate) fn zone(zone: &str, refs: ZoneRefs) -> Self {
        Self(vec![(zone.to_owned(), refs)])
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn extend(&mut self, later: Self) {
        self.0.extend(later.0);
    }

    /// One line: per zone, how many of its references no zone of the census
    /// holds, then the total. The names behind each count go to the debug log.
    pub(crate) fn unresolved_line(&self) -> String {
        let mut held: [HashSet<&str>; KINDS.len()] = Default::default();
        for (_, refs) in &self.0 {
            for (set, names) in held.iter_mut().zip(&refs.held) {
                set.extend(names.iter().map(String::as_str));
            }
        }
        let mut total = 0;
        let mut zones = Vec::with_capacity(self.0.len());
        for (zone, refs) in &self.0 {
            let mut counts = Vec::with_capacity(KINDS.len());
            for (kind, label) in KINDS {
                let slot = kind as usize;
                let missing: Vec<&str> = refs.named[slot]
                    .iter()
                    .map(String::as_str)
                    .filter(|name| !held[slot].contains(name))
                    .collect();
                if !missing.is_empty() {
                    diag::debug!(Zone, "{zone} unresolved {label}: {}", missing.join(" "));
                }
                total += missing.len();
                counts.push(format!("{label}={}", missing.len()));
            }
            zones.push(format!("{zone} {}", counts.join(" ")));
        }
        format!(
            "cross-zone references unresolved: {total} ({})",
            zones.join("; ")
        )
    }
}
