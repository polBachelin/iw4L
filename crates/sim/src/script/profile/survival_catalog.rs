use super::catalog::{Builtin, Namespace::*, Owner::*};

/// Builtins only the survival gametype links, over the IW4 list. Survival's
/// wave director asks the session for bots through them, and the script
/// keeps the armory menus' player data with `survival_setplayerdata`.
pub(super) const SURVIVAL: &[Builtin] = &[
    Builtin::new(Function, "survival_requestallies", Script, false),
    Builtin::new(Function, "survival_requestenemies", Script, false),
    Builtin::new(Method, "survival_archetype", Player, false),
    Builtin::new(Method, "survival_setplayerdata", Player, false),
];
