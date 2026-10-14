//! An actor's shot: whether it fires now, and where its bullet goes.

use super::super::natives::actor::{actor, alive, held, number_field};
use super::super::natives::engine::collider_entity;
use super::super::turrets::{BulletOutcome, fire_bullet, ignore_self};
use super::accuracy::{TargetFacts, chance, miss_point, segment_hits_box};
use crate::bullet_collision::{MASK_SHOT, PLAYER_MAXS, PLAYER_MINS, TraceOutcome};
use crate::frame::FrameWorld;
use crate::script::Namespace::Method;
use crate::script::{Arc, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;
use glam::Vec3;

/// What `shoot` did, as the script reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShotOutcome {
    Fired,
    Turning,
    Blocked,
    InvalidTarget,
    Empty,
    Unsupported,
    Held,
}

impl ShotOutcome {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fired => "fired",
            Self::Turning => "turning",
            Self::Blocked => "blocked",
            Self::InvalidTarget => "invalid_target",
            Self::Empty => "empty",
            Self::Unsupported => "unsupported",
            Self::Held => "held",
        }
    }
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Method, "shoot", |world, receiver, args| {
        shoot(world, receiver, args).map(|outcome| Value::string(outcome.name()))
    });
}

/// A target as a shot sees it this tick.
struct Target {
    id: u64,
    client: Option<u32>,
    /// Players are aimed at the eye, anything else at the middle of its
    /// bounds.
    aim: [f32; 3],
    bounds: ([f32; 3], [f32; 3]),
    stance: &'static str,
    velocity: [f32; 3],
}

/// `self shoot( [point | target] )`: one shot of the actor's weapon from its
/// muzzle. At a target entity the actor's accuracy decides whether it is
/// aimed to hit or to miss; at a point, or along its facing, it goes where
/// it is told. Only a bullet weapon shoots at a target; a point or facing
/// shot traces a bullet whatever the weapon, until actors can own a
/// projectile.
fn shoot(world: &mut World, receiver: &Value, args: &[Value]) -> Result<ShotOutcome, String> {
    let id = actor(world, receiver)?;
    let Some(weapon) = world.resource::<Runtime>().actors[&id].weapon else {
        return Ok(ShotOutcome::Empty);
    };
    if !alive(world, id) {
        return Err("actor is dead".into());
    }
    if held(world) {
        return Ok(ShotOutcome::Held);
    }
    let kind = FrameWorld::from_world(world)
        .combat_facts_for(weapon)
        .and_then(|f| weapon_iw4::fire_weapon_kind(f.weap_type, f.weap_class));
    let from = muzzle(world, id);
    let (angles, tolerance) = {
        let mut runtime = world.resource_mut::<Runtime>();
        let angles = match runtime.object_field(id, "angles") {
            Value::Vector(angles) => angles,
            _ => [0.0; 3],
        };
        (angles, number_field(&mut runtime, id, "aimtolerance"))
    };
    let turning = |dir: [f32; 3]| {
        tolerance.is_some_and(|tolerance| {
            math_iw4::angle_subtract(math_iw4::vect_to_angles(dir)[1], angles[1]).abs() > tolerance
        })
    };
    let (dir, target) = match args.first() {
        None | Some(Value::Undefined) => (math_iw4::angle_vectors(angles).0, None),
        Some(Value::Vector(point)) => match direction(from, *point) {
            Some(dir) => (dir, None),
            None => return Ok(ShotOutcome::InvalidTarget),
        },
        Some(value @ Value::Object(_)) => {
            if kind != Some(weapon_iw4::FireWeaponKind::Bullet) {
                diag::debug!(
                    Sim,
                    "shoot: actor {id} weapon {} fires {kind:?}, which an actor cannot launch",
                    weapon_label(world, weapon)
                );
                return Ok(ShotOutcome::Unsupported);
            }
            let Some(target) = target(world, id, value) else {
                return Ok(ShotOutcome::InvalidTarget);
            };
            let now = super::super::players::now_ms(world);
            if world.resource::<Runtime>().actors[&id]
                .ai
                .fire_block
                .blocked_until_ms
                > now
            {
                return Ok(ShotOutcome::Blocked);
            }
            let Some(toward) = direction(from, target.aim) else {
                return Ok(ShotOutcome::InvalidTarget);
            };
            if turning(toward) {
                return Ok(ShotOutcome::Turning);
            }
            if friend_in_line(world, id, from, target.aim, target.id) {
                return Ok(ShotOutcome::Blocked);
            }
            let aim = aim(world, id, weapon, from, toward, &target)?;
            match direction(from, aim) {
                Some(dir) => (dir, Some(target.id)),
                None => return Ok(ShotOutcome::InvalidTarget),
            }
        }
        Some(other) => return Err(format!("cannot shoot at {other:?}")),
    };
    if target.is_none() && turning(dir) {
        return Ok(ShotOutcome::Turning);
    }
    let number = world.resource::<Runtime>().entities[&id].number;
    let tick = world.resource::<crate::step::StepRequest>().tick;
    let mut frame = FrameWorld::from_world(world);
    let correlation = frame.alloc_shot_id().0;
    frame.push_entity_event(
        tick,
        crate::EventAudience::All,
        entity_iw4::predicted_weapon_fire_event(0, false),
        crate::EntityEventPayload {
            number,
            weapon,
            correlation,
            origin: from,
            direction: math_iw4::vect_to_angles(dir),
            ..Default::default()
        },
    );
    let bullet = fire_bullet(world, id, from, dir, weapon, None);
    diag::debug!(Sim, "shoot: actor {id} target {target:?} fired {bullet:?}");
    if bullet == BulletOutcome::NoFacts {
        return Ok(ShotOutcome::Unsupported);
    }
    let now = super::super::players::now_ms(world);
    if let Some(actor) = world.resource_mut::<Runtime>().actors.get_mut(&id) {
        actor.ai.shot.last_fired_ms = Some(now);
        actor.ai.shot.last_target = target;
    }
    Ok(ShotOutcome::Fired)
}

/// Rolls the actor's chance against `target` and returns the point the shot
/// is aimed at: the target's aim point on a hit, a point clear of its
/// bounds on a miss.
fn aim(
    world: &mut World,
    id: u64,
    weapon: u32,
    from: [f32; 3],
    dir: [f32; 3],
    target: &Target,
) -> Result<[f32; 3], String> {
    let frame = FrameWorld::from_world(world);
    let name = crate::script_player::weapon_name(&frame, weapon);
    let graphs = frame.accuracy_graphs_for(weapon);
    let (channel, graph) = match target.client {
        Some(_) => (
            "vs_player",
            graphs.as_ref().and_then(|g| g.vs_player.clone()),
        ),
        None => ("vs_ai", graphs.as_ref().and_then(|g| g.vs_ai.clone())),
    };
    let graph = graph.ok_or_else(|| format!("weapon `{name}` has no {channel} accuracy graph"))?;
    let mut runtime = world.resource_mut::<Runtime>();
    let profile = super::profile::profile(&mut runtime)?;
    let actor_factor = number_field(&mut runtime, id, "accuracy")
        .filter(|v| v.is_finite())
        .ok_or("actor has no accuracy")?;
    let attackers = runtime
        .ai
        .attackers
        .by_target
        .get(&target.id)
        .map_or(1, |set| set.len() + usize::from(!set.contains(&id)));
    let facts = TargetFacts {
        player: target.client.is_some(),
        stance: target.stance,
        velocity: target.velocity,
        attackers,
    };
    let distance = Vec3::from_array(from).distance(Vec3::from_array(target.aim));
    let chance = chance(&profile, &graph, actor_factor, dir, distance, &facts)
        .map_err(|e| format!("weapon `{name}`: {e}"))?;
    let roll = super::random_unit(&mut runtime);
    let hit = roll < chance.p;
    let point = if hit {
        target.aim
    } else {
        let side = if super::random(&mut runtime) & 1 == 0 {
            1.0
        } else {
            -1.0
        };
        let height = super::random_unit(&mut runtime);
        let miss =
            |side: f32| miss_point(from, target.bounds, profile.miss_clearance, side, height);
        let first = miss(side);
        if segment_hits_box(from, first, target.bounds) {
            let other = miss(-side);
            if segment_hits_box(from, other, target.bounds) {
                diag::debug!(
                    Sim,
                    "shoot: actor {id} has no miss line clear of {}",
                    target.id
                );
            }
            other
        } else {
            first
        }
    };
    diag::debug!(
        Sim,
        "shoot: actor {id} target {} weapon {name} graph `{}` {channel} x {:.3} weapon {:.3} \
         actor {:.3} stance {:.3} motion {:.3} attackers {} ({:.4}) p {:.4} roll {:.4} {}",
        target.id,
        graph.name,
        chance.x,
        chance.weapon,
        chance.actor,
        chance.stance,
        chance.motion,
        attackers,
        chance.attackers,
        chance.p,
        roll,
        if hit { "hit" } else { "miss" }
    );
    Ok(point)
}

/// The target `value` names, if it is one `id` may shoot: a live,
/// alive entity other than the actor, not on its team.
fn target(world: &mut World, id: u64, value: &Value) -> Option<Target> {
    let target = world.resource::<Runtime>().entity(value).map(|(t, _)| t)?;
    if target == id || !world.resource::<Runtime>().live(&target) {
        return None;
    }
    let client = world.resource::<Runtime>().player_client(target);
    let own = team(world, id);
    if own.is_some() && own == team(world, target) {
        return None;
    }
    let origin = match super::super::players::entity_field(world, target, "origin") {
        Value::Vector(origin) => origin,
        _ => return None,
    };
    if let Some(client) = client {
        let frame = FrameWorld::from_world(world);
        let meta = frame.client_meta(crate::ClientId(client))?;
        let ps = frame.player(crate::ClientId(client))?;
        if meta.lifecycle != crate::match_state::ClientLifecycle::Alive || ps.health <= 0 {
            return None;
        }
        let at = Vec3::from_array(ps.origin);
        return Some(Target {
            id: target,
            client: Some(client),
            aim: (at + Vec3::Z * ps.view_height_current).to_array(),
            bounds: (
                (at + Vec3::from_array(PLAYER_MINS)).to_array(),
                (at + Vec3::from_array(PLAYER_MAXS)).to_array(),
            ),
            stance: crate::script_player::stance(&frame, crate::ClientId(client)),
            velocity: ps.velocity,
        });
    }
    let is_actor = world.resource::<Runtime>().actors.contains_key(&target);
    if is_actor && !alive(world, target) {
        return None;
    }
    let bounds = body_bounds(world, target, origin);
    let now = super::super::players::now_ms(world);
    let velocity = world
        .resource::<super::super::mechanics::Mechanics>()
        .velocity(target, now);
    Some(Target {
        id: target,
        client: None,
        aim: std::array::from_fn(|i| (bounds.0[i] + bounds.1[i]) * 0.5),
        bounds,
        stance: "stand",
        velocity,
    })
}

/// The world box around an entity's hit volumes; an actor without posed
/// volumes yet stands in a player's box.
fn body_bounds(world: &mut World, object: u64, origin: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let presence = world.resource::<Runtime>().entities[&object].presence;
    let frame = FrameWorld::from_world(world);
    let bones = presence.and_then(|presence| {
        frame
            .entity_collision_capabilities()
            .iter()
            .find(|row| row.owner.script_model() == Some(presence))?
            .dobj
            .as_ref()?
            .current_collision
            .as_ref()
            .map(|collision| collision.bones.clone())
    });
    let mut mins = Vec3::splat(f32::MAX);
    let mut maxs = Vec3::splat(f32::MIN);
    for bone in bones.iter().flatten() {
        let extent = Vec3::from_array(std::array::from_fn(|axis| {
            (0..3)
                .map(|i| bone.axes[i][axis].abs() * bone.half_size[i])
                .sum::<f32>()
        }));
        let center = Vec3::from_array(bone.center);
        mins = mins.min(center - extent);
        maxs = maxs.max(center + extent);
    }
    if mins.cmple(maxs).all() {
        return (mins.to_array(), maxs.to_array());
    }
    if world.resource::<Runtime>().actors.contains_key(&object) {
        let at = Vec3::from_array(origin);
        return (
            (at + Vec3::from_array(PLAYER_MINS)).to_array(),
            (at + Vec3::from_array(PLAYER_MAXS)).to_array(),
        );
    }
    let (lo, hi) = super::super::triggers::entity_bounds(world, object);
    (
        std::array::from_fn(|i| origin[i] + lo[i]),
        std::array::from_fn(|i| origin[i] + hi[i]),
    )
}

fn team(world: &mut World, object: u64) -> Option<Arc<str>> {
    if let Some(actor) = world.resource::<Runtime>().actors.get(&object) {
        return Some(actor.team.clone());
    }
    match super::super::players::entity_field(world, object, "team") {
        Value::String(team) if !team.is_empty() => Some(Arc::from(&*team)),
        _ => None,
    }
}

/// Whether the first thing on the line from the muzzle to `to` is someone
/// on the actor's own team rather than `target`.
fn friend_in_line(world: &mut World, id: u64, from: [f32; 3], to: [f32; 3], target: u64) -> bool {
    let ignore = ignore_self(world, id);
    let TraceOutcome::Hit { collider, .. } =
        super::super::natives::engine::entity_trace(world, from, to, MASK_SHOT, ignore)
    else {
        return false;
    };
    let Value::Object(hit) = collider_entity(world, collider) else {
        return false;
    };
    if hit == target || hit == id {
        return false;
    }
    let own = team(world, id);
    let blocked = own.is_some() && own == team(world, hit);
    if blocked {
        diag::debug!(Sim, "shoot: actor {id} holds fire, friend {hit} in line");
    }
    blocked
}

fn direction(from: [f32; 3], to: [f32; 3]) -> Option<[f32; 3]> {
    (Vec3::from_array(to) - Vec3::from_array(from))
        .try_normalize()
        .map(|dir| dir.to_array())
}

fn weapon_label(world: &mut World, weapon: u32) -> String {
    crate::script_player::weapon_name(&FrameWorld::from_world(world), weapon)
}

pub(crate) fn muzzle(world: &mut World, id: u64) -> [f32; 3] {
    if let Some((origin, _)) = super::super::presence::tag_world(world, id, "tag_flash") {
        return origin;
    }
    let origin = match world.resource_mut::<Runtime>().object_field(id, "origin") {
        Value::Vector(origin) => origin,
        _ => [0.0; 3],
    };
    let eye = movement_iw4::view_height::STAND as f32;
    [origin[0], origin[1], origin[2] + eye]
}
