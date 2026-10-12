use super::catalog::{Builtin, Namespace::*, Owner::*};

/// Builtins only the survival gametype links, over the IW4 list. Survival's
/// wave director asks the session for bots through them, and the script
/// keeps the armory menus' player data with `survival_setplayerdata`.
/// `setcharacter` dresses any entity in named body, head and hat models.
/// The node getters hand out the map's path nodes, where actors walk.
pub(super) const SURVIVAL: &[Builtin] = &[
    Builtin::new(Function, "survival_requestallies", Script, false),
    Builtin::new(Function, "survival_requestenemies", Script, false),
    Builtin::new(Method, "survival_archetype", Player, false),
    Builtin::new(Method, "survival_setplayerdata", Player, false),
    Builtin::new(Method, "setcharacter", Entity, false),
    Builtin::new(Function, "getallnodes", Script, false),
    Builtin::new(Function, "getnode", Script, false),
    Builtin::new(Function, "getnodearray", Script, false),
];
