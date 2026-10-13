use super::catalog::{Builtin, Namespace::*, Owner::*};

pub(super) const ACTOR: &[Builtin] = &[
    Builtin::new(Function, "spawnactor", Script, false),
    Builtin::new(Method, "finishactordamage", Actor, false),
    Builtin::new(Method, "dropweapon", Actor, false),
    Builtin::new(Method, "placecharge", Actor, false),
    Builtin::new(Method, "getteam", Actor, false),
    Builtin::new(Method, "isalive", Actor, false),
    Builtin::new(Method, "setgoalnode", Actor, false),
    Builtin::new(Method, "setgoalpos", Actor, false),
    Builtin::new(Method, "setgoalradius", Actor, false),
    Builtin::new(Method, "setweapon", Actor, false),
    Builtin::new(Method, "shoot", Actor, false),
];
