//! How MW3 dresses an actor, read from the compiled scripts of the loaded
//! zones. A spawner's classname names its aitype (`actor_<name>` is
//! `aitype/<name>`); the aitype's `precache()` calls the `precache()` of each
//! character script it draws from; a character's `precache()` precaches its
//! body, then its heads, then its hat, each either a model named in place or
//! the array an alias script's `main()` returns. Character and alias scripts
//! are named by number.
//!
//! A function's stack opens with its size (u32) and its name; a call to
//! another script's function is that script's number (u16) and the
//! function's name; a string is NUL terminated.

use std::collections::BTreeMap;

/// What a spawner's classname adds to its aitype's name.
pub(super) const CLASSNAME_PREFIX: &str = "actor_";
const AITYPE_PREFIX: &str = "aitype/";

/// The canonical names of the two functions a character script compiles.
const MAIN: [u8; 2] = [0x72, 0x06];
const PRECACHE: [u8; 2] = [0x81, 0x06];
const HEADER: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Character {
    pub(super) id: String,
    pub(super) bodies: Vec<String>,
    pub(super) heads: Vec<String>,
    pub(super) hats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Dress {
    pub(super) aitype: String,
    /// The script that named the classname in place of the archetype's own.
    pub(super) overridden_by: Option<String>,
    pub(super) characters: Vec<Character>,
}

/// Whether a captured script is one the dressing reads: an aitype, a script
/// named by number, or one naming a classname (a map's aitype override).
pub(crate) fn is_dress_script(name: &str, stack: &[u8]) -> bool {
    name.starts_with(AITYPE_PREFIX)
        || is_numbered(name)
        || literals(stack).any(|literal| literal.starts_with(CLASSNAME_PREFIX))
}

fn is_numbered(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit())
}

/// The classname each archetype of `refs` is overridden with, and the script
/// that does it: a classname literal followed by the archetype's ref, when the
/// classname names a loaded aitype.
pub(super) fn overrides(
    scripts: &BTreeMap<String, Vec<u8>>,
    refs: &[&str],
) -> BTreeMap<String, (String, String)> {
    let mut found = BTreeMap::new();
    for (name, stack) in scripts {
        if name.starts_with(AITYPE_PREFIX) || is_numbered(name) {
            continue;
        }
        let literals: Vec<&str> = literals(stack).collect();
        for pair in literals.windows(2) {
            let (classname, reference) = (pair[0], pair[1]);
            if aitype_name(classname).is_some_and(|aitype| scripts.contains_key(&aitype))
                && let Some(reference) = refs.iter().find(|r| r.eq_ignore_ascii_case(reference))
            {
                found
                    .entry(reference.to_ascii_lowercase())
                    .or_insert_with(|| (classname.to_owned(), name.clone()));
            }
        }
    }
    found
}

fn aitype_name(classname: &str) -> Option<String> {
    classname
        .strip_prefix(CLASSNAME_PREFIX)
        .filter(|name| !name.is_empty())
        .map(|name| format!("{AITYPE_PREFIX}{name}"))
}

/// The characters the aitype of `classname` draws from, each with its
/// bodies, heads and hats.
pub(super) fn dress(
    classname: &str,
    overridden_by: Option<String>,
    scripts: &BTreeMap<String, Vec<u8>>,
) -> Result<Dress, String> {
    let aitype = aitype_name(classname)
        .ok_or_else(|| format!("`{classname}` is no `{CLASSNAME_PREFIX}<aitype>` classname"))?;
    let stack = scripts
        .get(&aitype)
        .ok_or_else(|| format!("`{aitype}` is in no loaded zone"))?;
    let mut ids: Vec<String> = Vec::new();
    for id in calls(stack, PRECACHE).map(|id| id.to_string()) {
        if scripts.contains_key(&id) && !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(format!("`{aitype}` precaches no character script"));
    }
    let characters = ids
        .into_iter()
        .map(|id| character(&id, scripts).map_err(|error| format!("`{aitype}` character {error}")))
        .collect::<Result<_, _>>()?;
    Ok(Dress {
        aitype,
        overridden_by,
        characters,
    })
}

fn character(id: &str, scripts: &BTreeMap<String, Vec<u8>>) -> Result<Character, String> {
    let unreadable = || format!("`{id}`: its `precache()` is unreadable");
    let stack = &scripts[id];
    let start = (1..stack.len())
        .find(|&at| {
            stack.get(at + 2..at + 4) == Some(&[0, 0])
                && stack.get(at + 4..at + HEADER) == Some(&PRECACHE)
        })
        .ok_or_else(unreadable)?
        + HEADER;
    let mut items: Vec<Vec<String>> = Vec::new();
    let mut at = start;
    while at < stack.len() {
        let alias = stack
            .get(at..at + 4)
            .filter(|w| w[2..] == MAIN)
            .map(|w| u16::from_le_bytes([w[0], w[1]]).to_string());
        if let Some(alias) = alias {
            let models = scripts
                .get(&alias)
                .ok_or_else(|| format!("`{id}`: alias script `{alias}` is in no loaded zone"))?;
            items.push(alias_models(models).ok_or_else(unreadable)?);
            at += 4;
            if stack
                .get(at..at + 4)
                .is_some_and(|w| !w.iter().all(|b| is_printable(*b)))
            {
                at += 4;
            }
            continue;
        }
        let end = stack[at..]
            .iter()
            .position(|&b| b == 0)
            .map_or(stack.len(), |n| at + n);
        let literal = &stack[at..end];
        if literal.is_empty() || !literal.iter().all(|b| is_printable(*b)) {
            return Err(unreadable());
        }
        items.push(vec![String::from_utf8_lossy(literal).into_owned()]);
        at = end + 1;
    }
    let mut items = items.into_iter();
    let character = Character {
        id: id.to_owned(),
        bodies: items.next().unwrap_or_default(),
        heads: items.next().unwrap_or_default(),
        hats: items.next().unwrap_or_default(),
    };
    if items.next().is_some() {
        return Err(format!(
            "`{id}` precaches more than a body, heads and a hat"
        ));
    }
    if character.bodies.is_empty() {
        return Err(format!("`{id}` precaches no body"));
    }
    Ok(character)
}

/// The models an alias script's `main()` returns.
fn alias_models(stack: &[u8]) -> Option<Vec<String>> {
    if stack.get(4..HEADER) != Some(&MAIN) {
        return None;
    }
    let models: Vec<String> = stack[HEADER..]
        .split(|&b| b == 0)
        .filter(|run| !run.is_empty())
        .map(|run| {
            run.iter()
                .all(|b| is_printable(*b))
                .then(|| String::from_utf8_lossy(run).into_owned())
        })
        .collect::<Option<_>>()?;
    (!models.is_empty()).then_some(models)
}

/// The scripts whose `function` a stack calls, by number.
fn calls(stack: &[u8], function: [u8; 2]) -> impl Iterator<Item = u16> + '_ {
    stack
        .windows(4)
        .filter(move |w| w[2..] == function)
        .map(|w| u16::from_le_bytes([w[0], w[1]]))
}

/// The strings of a stack: what precedes each NUL, from its last unprintable
/// byte on.
fn literals(stack: &[u8]) -> impl Iterator<Item = &str> {
    stack.split(|&b| b == 0).filter_map(|run| {
        let start = run
            .iter()
            .rposition(|b| !is_printable(*b))
            .map_or(0, |at| at + 1);
        std::str::from_utf8(&run[start..])
            .ok()
            .filter(|literal| !literal.is_empty())
    })
}

fn is_printable(b: u8) -> bool {
    (0x20..0x7f).contains(&b)
}
