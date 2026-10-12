use super::catalog::{Builtin, Namespace::*, Owner::*};

pub(super) const ANIM_TREE: &[Builtin] = &[
    Builtin::new(Method, "clearanim", Entity, false),
    Builtin::new(Method, "getanimtime", Entity, false),
    Builtin::new(Method, "setanim", Entity, false),
    Builtin::new(Method, "setanimknob", Entity, false),
    Builtin::new(Method, "setanimtime", Entity, false),
    Builtin::new(Method, "setflaggedanim", Entity, false),
    Builtin::new(Method, "setflaggedanimknob", Entity, false),
    Builtin::new(Method, "useanimtree", Entity, false),
];
