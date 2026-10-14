//! Thrown offhands: whether a throw can reach, and its one release.

use super::super::args::{int, string, vector};
use super::super::natives::actor::{actor, alive};
use super::super::natives::engine::{TraceIgnore, entity_trace};
use super::super::players::now_ms;
use super::profile::AiProfile;
use super::suppression::friends;
use crate::bullet_collision::{MASK_BULLET_WORLD, TraceOutcome};
use crate::frame::FrameWorld;
use crate::script::Namespace::Method;
use crate::script::{NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;
use glam::Vec3;

#[derive(Clone, Debug, Default)]
pub(crate) struct ActorGrenade {
    pub weapon: Option<u32>,
    pub count: u32,
    pub next_throw_ms: i64,
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Method, "setoffhand", set_offhand);
    registry.register(Method, "grenadethrow", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let target = vector(args, 0)?;
        Ok(solve_throw(world, id, target)?.map_or(Value::Undefined, |v| Value::Vector(v.into())))
    });
    registry.register(Method, "throwgrenade", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let velocity = vector(args, 0)?;
        let thrown = throw(world, id, velocity)?;
        Ok(Value::string(if thrown { "thrown" } else { "cancelled" }))
    });
}

/// `self setoffhand( weapon, count )`: the offhand this actor throws and
/// how many it holds.
fn set_offhand(world: &mut World, receiver: &Value, args: &[Value]) -> Result<Value, String> {
    let id = actor(world, receiver)?;
    let name = string(args, 0)?;
    let count = int(args, 1)?.max(0) as u32;
    let weapon = crate::script_player::weapon_named(&FrameWorld::from_world(world), &name)?;
    let mut runtime = world.resource_mut::<Runtime>();
    let grenade = &mut runtime.actors.get_mut(&id).unwrap().ai.grenade;
    grenade.weapon = (weapon != 0).then_some(weapon);
    grenade.count = count;
    Ok(Value::Undefined)
}

/// The offhand weapon when this actor may throw now: alive, holding one,
/// past its cooldown, and throwing allowed at all.
fn ready(world: &mut World, id: u64, profile: &AiProfile) -> Option<u32> {
    if profile.grenade_cooldown_ms <= 0 || !alive(world, id) {
        return None;
    }
    let grenade = world
        .resource::<Runtime>()
        .actors
        .get(&id)?
        .ai
        .grenade
        .clone();
    let weapon = grenade.weapon?;
    if grenade.count == 0 || now_ms(world) < grenade.next_throw_ms {
        return None;
    }
    FrameWorld::from_world(world)
        .equipment_facts_for(weapon)
        .filter(|facts| facts.is_usable())
        .map(|_| weapon)
}

fn release_point(world: &mut World, id: u64, profile: &AiProfile) -> Option<Vec3> {
    match world.resource_mut::<Runtime>().object_field(id, "origin") {
        Value::Vector(origin) => {
            Some(Vec3::from_array(origin) + Vec3::Z * profile.grenade_release_height)
        }
        _ => None,
    }
}

/// `self grenadethrow( point )`: the launch velocity of a throw that lands
/// at `point`, or undefined when no throw can.
fn solve_throw(world: &mut World, id: u64, target: [f32; 3]) -> Result<Option<Vec3>, String> {
    let profile = super::profile::profile(&mut world.resource_mut::<Runtime>())?;
    if ready(world, id, &profile).is_none() {
        return Ok(None);
    }
    let Some(from) = release_point(world, id, &profile) else {
        return Ok(None);
    };
    let target = Vec3::from_array(target);
    let ignore = TraceIgnore {
        model: world.resource::<Runtime>().entities[&id].presence,
        ..Default::default()
    };
    let Some((velocity, landed)) =
        arcs(from, target, &profile)
            .into_iter()
            .flatten()
            .find_map(|flight| {
                landing(world, from, target, flight, ignore, &profile)
                    .map(|at| (flight.velocity, at))
            })
    else {
        return Ok(None);
    };
    let team = world.resource::<Runtime>().actors[&id].team.clone();
    let exposed = friends(world).iter().any(|friend| {
        friend.object != id
            && friend.team == team
            && Vec3::from_array(friend.point)
                .truncate()
                .distance(landed.truncate())
                <= profile.friendly_suppression_distance
    });
    Ok((!exposed).then_some(velocity))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flight {
    pub velocity: Vec3,
    pub flight_s: f32,
}

/// The low and the high arc at the profile's maximum speed from `from` to
/// `to`, each `None` when that speed cannot reach.
pub(crate) fn arcs(from: Vec3, to: Vec3, profile: &AiProfile) -> [Option<Flight>; 2] {
    let (speed, gravity) = (profile.grenade_max_speed, profile.grenade_gravity);
    let delta = to - from;
    let flat = delta.truncate();
    let range = flat.length();
    if speed <= 0.0 || gravity <= 0.0 || range <= f32::EPSILON {
        return [None, None];
    }
    let speed_sq = speed * speed;
    let discriminant =
        speed_sq * speed_sq - gravity * (gravity * range * range + 2.0 * delta.z * speed_sq);
    if discriminant < 0.0 {
        return [None, None];
    }
    let heading = flat / range;
    [-1.0, 1.0].map(|side: f32| {
        let pitch = ((speed_sq + side * discriminant.sqrt()) / (gravity * range)).atan();
        let level = speed * pitch.cos();
        (level > f32::EPSILON).then(|| Flight {
            velocity: (heading * level).extend(speed * pitch.sin()),
            flight_s: range / level,
        })
    })
}

pub(crate) fn position(from: Vec3, flight: Flight, gravity: f32, time_s: f32) -> Vec3 {
    from + flight.velocity * time_s - Vec3::Z * (0.5 * gravity * time_s * time_s)
}

/// Where `flight` lands, when it flies clear to `to`: its flight split into
/// the profile's segments must touch nothing, and the terminal segment must
/// come to rest within the landing error of `to`. Any earlier contact
/// rejects the arc, even one a bouncing grenade would survive.
fn landing(
    world: &mut World,
    from: Vec3,
    to: Vec3,
    flight: Flight,
    ignore: TraceIgnore,
    profile: &AiProfile,
) -> Option<Vec3> {
    let segments = profile.grenade_arc_segments.max(1);
    let step = flight.flight_s / (segments + 1) as f32;
    let point = |i: u32| position(from, flight, profile.grenade_gravity, step * i as f32);
    for i in 0..segments {
        let outcome = entity_trace(
            world,
            point(i).to_array(),
            point(i + 1).to_array(),
            MASK_BULLET_WORLD,
            ignore,
        );
        if !matches!(outcome, TraceOutcome::Miss { .. }) {
            return None;
        }
    }
    let landing = match entity_trace(
        world,
        point(segments).to_array(),
        point(segments + 1).to_array(),
        MASK_BULLET_WORLD,
        ignore,
    ) {
        TraceOutcome::Miss { end } | TraceOutcome::Hit { end, .. } => Vec3::from_array(end),
        TraceOutcome::StartSolid { .. } | TraceOutcome::Invalid { .. } => return None,
    };
    (landing.truncate().distance(to.truncate()) <= profile.grenade_landing_error).then_some(landing)
}

/// `self throwgrenade( velocity )`: releases one offhand at `velocity`.
/// Whatever stopped the throw since it was solved cancels it, and a
/// cancelled throw spends nothing.
fn throw(world: &mut World, id: u64, velocity: [f32; 3]) -> Result<bool, String> {
    let profile = super::profile::profile(&mut world.resource_mut::<Runtime>())?;
    let Some(weapon) = ready(world, id, &profile) else {
        return Ok(false);
    };
    let Some(from) = release_point(world, id, &profile) else {
        return Ok(false);
    };
    let velocity = Vec3::from_array(velocity);
    if !velocity.is_finite() || velocity.length() > profile.grenade_max_speed * (1.0 + 1e-4) {
        return Ok(false);
    }
    if !release(world, id, weapon, from, velocity) {
        return Ok(false);
    }
    let now = now_ms(world);
    let mut runtime = world.resource_mut::<Runtime>();
    let grenade = &mut runtime.actors.get_mut(&id).unwrap().ai.grenade;
    grenade.count -= 1;
    grenade.next_throw_ms = now + profile.grenade_cooldown_ms;
    Ok(true)
}

/// Launches the one projectile of a throw, owned by the actor.
fn release(world: &mut World, id: u64, weapon: u32, from: Vec3, velocity: Vec3) -> bool {
    let Some(presence) = world.resource::<Runtime>().entities[&id].presence else {
        return false;
    };
    let tick = world.resource::<crate::step::StepRequest>().tick;
    let launched = crate::missile::spawn_entity_projectile(
        &mut FrameWorld::from_world(world),
        tick,
        presence,
        weapon,
        from.to_array(),
        velocity.to_array(),
    );
    if let Err(message) = &launched {
        diag::debug!(Sim, "throwgrenade: actor {id} released nothing: {message}");
    }
    launched.is_ok()
}
