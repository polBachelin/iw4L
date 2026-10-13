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

#[cfg(test)]
mod tests {
    use super::*;

    fn function(name: [u8; 2], body: &[&[u8]]) -> Vec<u8> {
        let mut out = vec![0x20, 0, 0, 0, name[0], name[1]];
        for part in body {
            out.extend_from_slice(part);
        }
        out
    }

    fn string(text: &str) -> Vec<u8> {
        let mut out = text.as_bytes().to_vec();
        out.push(0);
        out
    }

    fn call(id: u16, name: [u8; 2]) -> Vec<u8> {
        let [lo, hi] = id.to_le_bytes();
        vec![lo, hi, name[0], name[1]]
    }

    const HELPER: [u8; 4] = [0x44, 0x01, 0x56, 0x1e];

    fn alias(models: &[&str]) -> Vec<u8> {
        let parts: Vec<Vec<u8>> = models.iter().map(|m| string(m)).collect();
        let parts: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        function(MAIN, &parts)
    }

    /// A character as MW3 compiles one: `main()` sets the body and attaches
    /// a head and a hat with strings of their own, `precache()` lists the
    /// models.
    fn fixture() -> BTreeMap<String, Vec<u8>> {
        let mut scripts = BTreeMap::new();
        scripts.insert("1494".into(), alias(&["body_c", "body_d"]));
        scripts.insert("1495".into(), alias(&["head_a", "head_b"]));
        scripts.insert("1466".into(), alias(&["hat_a", "hat_null"]));
        let mut naval = function(
            MAIN,
            &[
                &call(1494, MAIN),
                &HELPER,
                &call(1495, MAIN),
                &string("alias_heads"),
                &HELPER,
                &string("russian"),
            ],
        );
        naval.extend(function(
            PRECACHE,
            &[&call(1494, MAIN), &HELPER, &call(1495, MAIN), &HELPER],
        ));
        scripts.insert("1496".into(), naval);
        let mut militia = function(
            MAIN,
            &[&string("body_militia"), &call(1466, MAIN), &string("_hat")],
        );
        militia.extend(function(
            PRECACHE,
            &[
                &string("body_militia"),
                &string("head_militia"),
                &call(1466, MAIN),
                &HELPER,
            ],
        ));
        scripts.insert("1469".into(), militia);
        let mut jug = function(MAIN, &[&string("fullbody_jug"), &string("russian")]);
        jug.extend(function(PRECACHE, &[&string("fullbody_jug")]));
        scripts.insert("18380".into(), jug);
        let aitype = |characters: &[u16]| {
            let mut main = vec![call(0, MAIN)];
            main.extend(characters.iter().map(|&id| call(id, MAIN)));
            main.push(call(1474, MAIN));
            let mut stack: Vec<u8> = main.concat();
            stack.extend(string("axis"));
            stack.extend([0x18, 0, 0, 0]);
            stack.extend(PRECACHE);
            for &id in characters {
                stack.extend(call(id, PRECACHE));
            }
            stack
        };
        scripts.insert("1474".into(), function(MAIN, &[&string("generic_human")]));
        scripts.insert("aitype/enemy_so_easy".into(), aitype(&[1469]));
        scripts.insert("aitype/enemy_so_easy_v2".into(), aitype(&[1496]));
        scripts.insert("aitype/enemy_so_elite".into(), aitype(&[18380]));
        scripts.insert(
            "maps/so_survival_mp_x".into(),
            [
                function(MAIN, &[&string("sp/tier_1.csv")]),
                string("actor_enemy_so_easy_v2"),
                string("easy"),
                call(973, MAIN),
            ]
            .concat(),
        );
        scripts
    }

    #[test]
    fn a_character_reads_its_bodies_heads_and_hats_from_its_precache() {
        let scripts = fixture();
        let dress = dress("actor_enemy_so_easy_v2", None, &scripts).unwrap();
        assert_eq!(dress.aitype, "aitype/enemy_so_easy_v2");
        assert_eq!(
            dress.characters,
            vec![Character {
                id: "1496".into(),
                bodies: vec!["body_c".into(), "body_d".into()],
                heads: vec!["head_a".into(), "head_b".into()],
                hats: Vec::new(),
            }]
        );
        let militia = &dress_of(&scripts, "actor_enemy_so_easy").characters[0];
        assert_eq!(militia.bodies, ["body_militia"]);
        assert_eq!(militia.heads, ["head_militia"]);
        assert_eq!(militia.hats, ["hat_a", "hat_null"]);
        let jug = &dress_of(&scripts, "actor_enemy_so_elite").characters[0];
        assert_eq!(jug.bodies, ["fullbody_jug"]);
        assert!(jug.heads.is_empty() && jug.hats.is_empty());
    }

    fn dress_of(scripts: &BTreeMap<String, Vec<u8>>, classname: &str) -> Dress {
        dress(classname, None, scripts).unwrap()
    }

    #[test]
    fn an_aitype_draws_only_the_scripts_it_precaches() {
        let scripts = fixture();
        let ids: Vec<String> = dress_of(&scripts, "actor_enemy_so_easy")
            .characters
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, ["1469"]);
    }

    #[test]
    fn a_map_script_overrides_an_archetype_with_a_loaded_aitype() {
        let scripts = fixture();
        let found = overrides(&scripts, &["easy", "regular"]);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found["easy"],
            (
                "actor_enemy_so_easy_v2".to_owned(),
                "maps/so_survival_mp_x".to_owned()
            )
        );
        let mut unloaded = scripts.clone();
        unloaded.remove("aitype/enemy_so_easy_v2");
        assert!(overrides(&unloaded, &["easy"]).is_empty());
    }

    #[test]
    fn a_missing_aitype_or_character_script_is_an_error() {
        let mut scripts = fixture();
        assert!(
            dress("actor_enemy_juggernaut", None, &scripts)
                .unwrap_err()
                .contains("aitype/enemy_juggernaut")
        );
        assert!(dress("enemy_so_easy", None, &scripts).is_err());
        scripts.remove("1495");
        assert!(
            dress("actor_enemy_so_easy_v2", None, &scripts)
                .unwrap_err()
                .contains("`1495` is in no loaded zone")
        );
        scripts.remove("1496");
        assert!(
            dress("actor_enemy_so_easy_v2", None, &scripts)
                .unwrap_err()
                .contains("precaches no character")
        );
    }

    #[test]
    fn only_dressing_scripts_are_kept() {
        let scripts = fixture();
        assert!(is_dress_script("aitype/enemy_dog", &[]));
        assert!(is_dress_script("1469", &[]));
        assert!(is_dress_script(
            "maps/so_survival_mp_x",
            &scripts["maps/so_survival_mp_x"]
        ));
        assert!(!is_dress_script("maps/mp/_load", &string("compass_map_x")));
    }
}
