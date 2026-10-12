use super::args::{arg, float, kind, optional, string};
use super::natives::engine::entity_id;
use crate::frame::FrameWorld;
use crate::script::runtime::raise;
use crate::script::{Arc, Namespace, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;

const DEFAULT_BLEND_TIME: f32 = 0.2;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AnimBlend {
    tree: Arc<str>,
    leaves: Vec<Leaf>,
    revision: u32,
    pub(crate) dirty: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Leaf {
    anim: Arc<str>,
    clip: Clip,
    weight: f32,
    goal_weight: f32,
    goal_time: f32,
    rate: f32,
    time: f32,
    old_time: f32,
    cycle: i16,
    old_cycle: i16,
    fresh: bool,
    flag: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq)]
struct Clip {
    looping: bool,
    frequency: f32,
    notetracks: Arc<[(Arc<str>, f32)]>,
}

struct Goal {
    weight: f32,
    blend_time: f32,
    rate: f32,
    flag: Option<Arc<str>>,
    knob: bool,
}

impl AnimBlend {
    fn new(tree: Arc<str>) -> Self {
        Self {
            tree,
            leaves: Vec::new(),
            revision: 1,
            dirty: true,
        }
    }

    /// A blend no script reaches: it names no anim tree.
    pub(crate) fn unscripted() -> Self {
        Self::new(Arc::from(""))
    }

    /// Ramps `anim` to full weight over `blend_time` and every other leaf to
    /// none, raising no notetracks.
    pub(crate) fn play(&mut self, anim: Arc<str>, looping: bool, frequency: f32, blend_time: f32) {
        let clip = Clip {
            looping,
            frequency,
            notetracks: Arc::from([]),
        };
        let goal = Goal {
            weight: 1.0,
            blend_time,
            rate: 1.0,
            flag: None,
            knob: true,
        };
        self.set(anim, clip, goal);
    }

    fn leaf_mut(&mut self, anim: &str) -> Option<&mut Leaf> {
        self.leaves.iter_mut().find(|leaf| &*leaf.anim == anim)
    }

    fn set(&mut self, anim: Arc<str>, clip: Clip, goal: Goal) {
        if goal.knob {
            for leaf in self.leaves.iter_mut().filter(|leaf| leaf.anim != anim) {
                leaf.ramp(0.0, goal.blend_time);
            }
        }
        if self.leaf_mut(&anim).is_none() {
            self.leaves.push(Leaf {
                anim: Arc::clone(&anim),
                clip,
                weight: 0.0,
                goal_weight: 0.0,
                goal_time: 0.0,
                rate: 1.0,
                time: 0.0,
                old_time: 0.0,
                cycle: 0,
                old_cycle: 0,
                fresh: true,
                flag: None,
            });
            self.revision = self.revision.wrapping_add(1);
        }
        let leaf = self.leaf_mut(&anim).expect("leaf inserted above");
        leaf.ramp(goal.weight, goal.blend_time);
        leaf.rate = goal.rate;
        leaf.flag = goal.flag;
        self.prune();
        self.dirty = true;
    }

    fn clear(&mut self, anim: Option<&str>, blend_time: f32) {
        for leaf in &mut self.leaves {
            if anim.is_none_or(|anim| &*leaf.anim == anim) {
                leaf.ramp(0.0, blend_time);
            }
        }
        self.prune();
        self.dirty = true;
    }

    fn set_time(&mut self, anim: &str, time: f32) {
        if let Some(leaf) = self.leaf_mut(anim) {
            leaf.time = time;
            leaf.old_time = time;
            leaf.fresh = true;
            self.dirty = true;
        }
    }

    fn time(&self, anim: &str) -> f32 {
        self.leaves
            .iter()
            .find(|leaf| &*leaf.anim == anim)
            .map_or(0.0, |leaf| leaf.time)
    }

    fn prune(&mut self) {
        let before = self.leaves.len();
        self.leaves
            .retain(|leaf| leaf.weight != 0.0 || leaf.goal_weight != 0.0);
        if self.leaves.len() != before {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub(crate) fn advance(&mut self, dt: f32, fired: &mut Vec<(Arc<str>, Arc<str>)>) {
        if self.leaves.is_empty() {
            return;
        }
        for leaf in &mut self.leaves {
            leaf.advance(dt, fired);
        }
        self.prune();
        self.dirty = true;
    }

    pub(crate) fn snapshot(&self) -> Option<xmodel_runtime::XAnimTreeSnapshot> {
        if self.leaves.is_empty() {
            return None;
        }
        let root = xmodel_runtime::XAnimSemanticNode {
            parent: None,
            kind: xmodel_runtime::XAnimSemanticNodeKind::Blend,
            clip: None,
            parts: None,
            state: xmodel_runtime::XAnimNodeState {
                weight: 1.0,
                goal_weight: 1.0,
                rate: 1.0,
                ..xmodel_runtime::XAnimNodeState::default()
            },
        };
        let leaves = self
            .leaves
            .iter()
            .map(|leaf| xmodel_runtime::XAnimSemanticNode {
                parent: Some(xmodel_runtime::XAnimNodeId(0)),
                kind: xmodel_runtime::XAnimSemanticNodeKind::Leaf,
                clip: Some(leaf.anim.to_string()),
                parts: None,
                state: xmodel_runtime::XAnimNodeState {
                    time: leaf.time,
                    old_time: leaf.old_time,
                    cycle_count: leaf.cycle,
                    old_cycle_count: leaf.old_cycle,
                    goal_time: leaf.goal_time,
                    goal_weight: leaf.goal_weight,
                    weight: leaf.weight,
                    rate: leaf.rate,
                },
            });
        Some(xmodel_runtime::XAnimTreeSnapshot {
            definition_revision: self.revision,
            state_revision: 0,
            nodes: std::iter::once(root).chain(leaves).collect(),
        })
    }
}

impl Leaf {
    fn ramp(&mut self, goal_weight: f32, blend_time: f32) {
        self.goal_weight = goal_weight;
        if blend_time > 0.0 {
            self.goal_time = blend_time;
        } else {
            self.goal_time = 0.0;
            self.weight = goal_weight;
        }
    }

    fn advance(&mut self, dt: f32, fired: &mut Vec<(Arc<str>, Arc<str>)>) {
        (self.weight, self.goal_time) =
            anim_iw4::advance_goal_weight(self.weight, self.goal_weight, self.goal_time, dt, true);
        self.old_time = self.time;
        self.old_cycle = self.cycle;
        if self.weight == 0.0 {
            return;
        }
        (self.time, self.cycle) = anim_iw4::advance_leaf_time(
            self.old_time,
            self.cycle,
            self.rate,
            self.clip.frequency,
            dt,
            self.clip.looping,
        );
        let fresh = std::mem::replace(&mut self.fresh, false);
        let Some(flag) = &self.flag else {
            return;
        };
        let park = anim_iw4::XANIM_NONLOOP_END_PARK;
        if !self.clip.looping && self.old_time >= park {
            return;
        }
        let from = self.old_time;
        let wrapped = self.cycle != self.old_cycle;
        let ended = !self.clip.looping && self.time >= park;
        let to = if ended { 1.0 } else { self.time };
        let notes = || {
            self.clip
                .notetracks
                .iter()
                .filter(|(note, _)| !note.eq_ignore_ascii_case("end"))
        };
        let after_start = |at: f32| at > from || (fresh && at == from);
        if wrapped {
            for (note, _) in notes().filter(|(_, at)| after_start(*at)) {
                fired.push((Arc::clone(flag), Arc::clone(note)));
            }
            fired.push((Arc::clone(flag), "end".into()));
            for (note, _) in notes().filter(|(_, at)| *at <= to) {
                fired.push((Arc::clone(flag), Arc::clone(note)));
            }
            return;
        }
        for (note, _) in notes().filter(|(_, at)| after_start(*at) && *at <= to) {
            fired.push((Arc::clone(flag), Arc::clone(note)));
        }
        if ended {
            fired.push((Arc::clone(flag), "end".into()));
        }
    }
}

pub(crate) fn advance(world: &mut World, dt: f32) {
    let mut fired = Vec::new();
    let mut runtime = world.resource_mut::<Runtime>();
    for (object, entity) in runtime.entities.iter_mut() {
        let Some(blend) = entity.anim_blend.as_deref_mut() else {
            continue;
        };
        let mut notes = Vec::new();
        blend.advance(dt, &mut notes);
        fired.extend(notes.into_iter().map(|(flag, note)| (*object, flag, note)));
    }
    for (object, flag, note) in fired {
        raise(
            world,
            Value::Object(object),
            &flag,
            vec![Value::string(&note)],
        );
    }
}

fn clip(world: &mut World, name: &str) -> Option<Clip> {
    let frame = FrameWorld::from_world(world);
    let facts = frame.script_model_anim(name)?;
    Some(Clip {
        looping: facts.looping,
        frequency: facts.frequency,
        notetracks: frame.script_anim_notetracks(name),
    })
}

pub(crate) fn length(world: &mut World, name: &str) -> Option<f32> {
    let frequency = clip(world, name)?.frequency;
    (frequency > 0.0).then(|| 1.0 / frequency)
}

/// An animation literal, or an animation name the entity's own tree resolves.
fn animation(args: &[Value], index: usize) -> Result<(Option<Arc<str>>, Arc<str>), String> {
    match arg(args, index)? {
        Value::Animation { tree, name } => Ok((Some(Arc::clone(tree)), Arc::clone(name))),
        Value::String(name) => Ok((None, name.to_ascii_lowercase().into())),
        other => Err(format!(
            "parameter {} is {}, not an animation",
            index + 1,
            kind(other)
        )),
    }
}

fn with_blend<T>(
    world: &mut World,
    receiver: &Value,
    tree: Option<&str>,
    change: impl FnOnce(&mut AnimBlend) -> T,
) -> Result<T, String> {
    let id = entity_id(world, receiver)?;
    let mut runtime = world.resource_mut::<Runtime>();
    let blend = runtime
        .entities
        .get_mut(&id)
        .and_then(|entity| entity.anim_blend.as_deref_mut())
        .ok_or("entity has no animation tree; call useanimtree first")?;
    if let Some(tree) = tree.filter(|tree| **tree != *blend.tree) {
        return Err(format!(
            "animation is from animtree '{tree}', the entity uses '{}'",
            blend.tree
        ));
    }
    Ok(change(blend))
}

fn set_anim(
    world: &mut World,
    receiver: &Value,
    args: &[Value],
    flagged: bool,
    knob: bool,
) -> Result<Value, String> {
    let (flag, first) = if flagged {
        (Some(Arc::<str>::from(string(args, 0)?)), 1)
    } else {
        (None, 0)
    };
    let (tree, name) = animation(args, first)?;
    let goal = Goal {
        weight: optional(args, first + 1, float)?.unwrap_or(1.0),
        blend_time: optional(args, first + 2, float)?.unwrap_or(DEFAULT_BLEND_TIME),
        rate: optional(args, first + 3, float)?.unwrap_or(1.0),
        flag,
        knob,
    };
    if !goal.weight.is_finite() || !goal.blend_time.is_finite() || !goal.rate.is_finite() {
        return Err("animation weight, blend time and rate must be finite".into());
    }
    with_blend(world, receiver, tree.as_deref(), |_| ())?;
    let clip = clip(world, &name)
        .ok_or_else(|| format!("animation '{name}' is not loaded in the simulation"))?;
    with_blend(world, receiver, None, |blend| blend.set(name, clip, goal))?;
    Ok(Value::Undefined)
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    use Namespace::Method;

    registry.register(Method, "useanimtree", |world, receiver, args| {
        let tree = match arg(args, 0)? {
            Value::AnimationTree(tree) => Arc::clone(tree),
            other => return Err(format!("{} is not an animtree", kind(other))),
        };
        let id = entity_id(world, receiver)?;
        let mut runtime = world.resource_mut::<Runtime>();
        let entity = runtime.entities.get_mut(&id).unwrap();
        if entity.presence.is_none() {
            return Err("entity has no model to animate".into());
        }
        if entity
            .anim_blend
            .as_ref()
            .is_none_or(|blend| blend.tree != tree)
        {
            entity.anim_blend = Some(Box::new(AnimBlend::new(tree)));
        }
        Ok(Value::Undefined)
    });
    registry.register(Method, "setanim", |world, receiver, args| {
        set_anim(world, receiver, args, false, false)
    });
    registry.register(Method, "setanimknob", |world, receiver, args| {
        set_anim(world, receiver, args, false, true)
    });
    registry.register(Method, "setflaggedanim", |world, receiver, args| {
        set_anim(world, receiver, args, true, false)
    });
    registry.register(Method, "setflaggedanimknob", |world, receiver, args| {
        set_anim(world, receiver, args, true, true)
    });
    registry.register(Method, "clearanim", |world, receiver, args| {
        let (tree, name) = animation(args, 0)?;
        let blend_time = float(args, 1)?;
        if !blend_time.is_finite() {
            return Err("blend time must be finite".into());
        }
        let leaf = clip(world, &name).is_some();
        with_blend(world, receiver, tree.as_deref(), |blend| {
            blend.clear(leaf.then_some(&*name), blend_time)
        })?;
        Ok(Value::Undefined)
    });
    registry.register(Method, "setanimtime", |world, receiver, args| {
        let (tree, name) = animation(args, 0)?;
        let time = float(args, 1)?;
        if !(0.0..=1.0).contains(&time) {
            return Err(format!("animation time {time} is not between 0 and 1"));
        }
        with_blend(world, receiver, tree.as_deref(), |blend| {
            blend.set_time(&name, time)
        })?;
        Ok(Value::Undefined)
    });
    registry.register(Method, "getanimtime", |world, receiver, args| {
        let (tree, name) = animation(args, 0)?;
        with_blend(world, receiver, tree.as_deref(), |blend| {
            Value::Float(blend.time(&name))
        })
    });
}
