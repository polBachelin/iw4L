//! Friendly-fire blocking: an actor holds its fire while a friend stands
//! in its line of fire.

use super::super::natives::actor::alive;
use super::super::players::{load_field, now_ms};
use super::shot::muzzle;
use crate::frame::FrameWorld;
use crate::script::{Arc, Runtime, Value};
use bevy_ecs::prelude::World;

#[derive(Clone, Debug, Default)]
pub(crate) struct FireBlock {
    pub blocked_until_ms: i64,
}

/// A live body on some team, at the height its line of fire or sight
/// passes through.
pub(crate) struct Friend {
    pub object: u64,
    pub team: Arc<str>,
    pub point: [f32; 3],
}

/// Every live actor and playing player, each at its eye.
pub(crate) fn friends(world: &mut World) -> Vec<Friend> {
    let eye = movement_iw4::view_height::STAND as f32;
    let actors: Vec<(u64, Arc<str>)> = world
        .resource::<Runtime>()
        .actors
        .iter()
        .map(|(id, actor)| (*id, actor.team.clone()))
        .collect();
    let mut friends = Vec::new();
    for (object, team) in actors {
        if !alive(world, object) {
            continue;
        }
        let origin = match world
            .resource_mut::<Runtime>()
            .object_field(object, "origin")
        {
            Value::Vector(origin) => origin,
            _ => continue,
        };
        friends.push(Friend {
            object,
            team,
            point: [origin[0], origin[1], origin[2] + eye],
        });
    }
    let players: Vec<(u32, u64)> = world
        .resource::<Runtime>()
        .players
        .iter()
        .filter(|(_, slot)| &*slot.sessionstate == "playing")
        .map(|(client, slot)| (*client, slot.object))
        .collect();
    for (client, object) in players {
        let Some(Value::String(team)) = load_field(world, client, "sessionteam") else {
            continue;
        };
        let frame = FrameWorld::from_world(world);
        let Some(ps) = frame.player(crate::ClientId(client)) else {
            continue;
        };
        if ps.health <= 0 {
            continue;
        }
        friends.push(Friend {
            object,
            team: Arc::from(&*team),
            point: [
                ps.origin[0],
                ps.origin[1],
                ps.origin[2] + ps.view_height_current,
            ],
        });
    }
    friends
}

/// Whether `point` lies within `radius` of the segment `from`-`to`, between
/// its two ends: a friend behind the shooter or past the target is not in
/// the way.
pub(crate) fn between(from: [f32; 3], to: [f32; 3], point: [f32; 3], radius: f32) -> bool {
    let (from, to, point) = (
        glam::Vec3::from_array(from),
        glam::Vec3::from_array(to),
        glam::Vec3::from_array(point),
    );
    let line = to - from;
    let length_sq = line.length_squared();
    if length_sq <= f32::EPSILON {
        return false;
    }
    let along = (point - from).dot(line) / length_sq;
    if along <= 0.0 || along >= 1.0 {
        return false;
    }
    point.distance_squared(from + line * along) <= radius * radius
}

/// Stamps or clears each fighting actor's block from where its friends
/// stand now. An actor without a target keeps its stamp until it expires.
pub(crate) fn update(world: &mut World) {
    let Ok(profile) = super::profile::profile(&mut world.resource_mut::<Runtime>()) else {
        return;
    };
    if !profile.friendly_suppression {
        return;
    }
    let shooters: Vec<(u64, Arc<str>, [f32; 3])> = world
        .resource::<Runtime>()
        .actors
        .iter()
        .filter_map(|(id, actor)| {
            let threat = &actor.ai.threat;
            let memory = threat.memories.get(&threat.target?)?;
            Some((*id, actor.team.clone(), memory.last_known))
        })
        .collect();
    if shooters.is_empty() {
        return;
    }
    let friends = friends(world);
    let now = now_ms(world);
    for (id, team, target) in shooters {
        if !alive(world, id) {
            continue;
        }
        let from = muzzle(world, id);
        let blocked = friends.iter().any(|friend| {
            friend.object != id
                && friend.team == team
                && between(
                    from,
                    target,
                    friend.point,
                    profile.friendly_suppression_distance,
                )
        });
        if let Some(actor) = world.resource_mut::<Runtime>().actors.get_mut(&id) {
            actor.ai.fire_block.blocked_until_ms = if blocked {
                now + profile.friendly_fire_block_ms
            } else {
                0
            };
        }
    }
}
