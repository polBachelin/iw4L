//! Cover nodes: scoring, claims and danger.

use super::super::natives::actor::{actor, alive};
use super::super::natives::actor_path::{cover_stance, is_cover, node_arg, node_object};
use super::super::players::{entity_field, now_ms};
use super::profile::{AiProfile, profile};
use crate::SimPathGraph;
use crate::frame::FrameWorld;
use crate::script::Namespace::Method;
use crate::script::{Arc, BTreeMap, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;
use movement_iw4::view_height;

/// Exclusive node ownership. A node has at most one owner; a release names
/// the revision it holds, so it can never clear a newer claim.
#[derive(Clone, Debug, Default)]
pub(crate) struct NodeClaims {
    pub owner: BTreeMap<u32, (u64, u32)>,
    pub next_revision: u32,
    pub danger_until_ms: BTreeMap<(u32, u32), i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActorCover {
    pub claim: Option<(u32, u32)>,
    pub previous: Option<u32>,
    pub next_search_ms: i64,
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Method, "findcovernode", |world, receiver, args| {
        let id = actor(world, receiver)?;
        match find(world, id, args)? {
            Some(node) => node_value(world, node),
            None => Ok(Value::Undefined),
        }
    });
    registry.register(Method, "claimnode", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let node = node_arg(world, args, 0)?;
        Ok(Value::Int(claim(world, id, node) as i32))
    });
    registry.register(Method, "claimednode", |world, receiver, _| {
        let id = actor(world, receiver)?;
        reap(world);
        match held(world.resource::<Runtime>(), id) {
            Some(node) => node_value(world, node),
            None => Ok(Value::Undefined),
        }
    });
    registry.register(Method, "releasenode", |world, receiver, _| {
        let id = actor(world, receiver)?;
        release_all(&mut world.resource_mut::<Runtime>(), id);
        Ok(Value::Undefined)
    });
}

/// Drops the actor's claim, if it still holds it, and remembers the node so
/// the next search skips it. A claim released by a dead actor marks its node
/// dangerous for the actor's team; the deadline outlives the release.
pub(crate) fn release_all(runtime: &mut Runtime, id: u64) {
    let Some(actor) = runtime.actors.get_mut(&id) else {
        return;
    };
    let Some(claim) = actor.ai.cover.claim.take() else {
        return;
    };
    actor.ai.cover.previous = Some(claim.0);
    let team = actor.team.clone();
    if runtime.ai.claims.owner.get(&claim.0) == Some(&(id, claim.1)) {
        runtime.ai.claims.owner.remove(&claim.0);
    }
    if !died(runtime, id) {
        return;
    }
    let (Ok(profile), Some(tick)) = (profile(runtime), runtime.last_tick) else {
        return;
    };
    if profile.cover_danger_ms <= 0 {
        return;
    }
    let until = i64::from(tick.0) * i64::from(crate::MATCH_TICK_MS) + profile.cover_danger_ms;
    let key = (claim.0, runtime.symbol(&team));
    let deadline = runtime
        .ai
        .claims
        .danger_until_ms
        .entry(key)
        .or_insert(until);
    *deadline = (*deadline).max(until);
}

fn died(runtime: &mut Runtime, id: u64) -> bool {
    match runtime.object_field(id, "health") {
        Value::Int(health) => health <= 0,
        Value::Float(health) => health <= 0.0,
        _ => true,
    }
}

/// The node the actor holds, while the claim table still names it.
fn held(runtime: &Runtime, id: u64) -> Option<u32> {
    let (node, revision) = runtime.actors.get(&id)?.ai.cover.claim?;
    (runtime.ai.claims.owner.get(&node) == Some(&(id, revision))).then_some(node)
}

/// Releases every claim a dead actor still holds, so its node frees up and
/// its danger starts before anyone reads the table.
fn reap(world: &mut World) {
    let owners: Vec<u64> = {
        let runtime = world.resource::<Runtime>();
        let mut owners: Vec<u64> = runtime
            .ai
            .claims
            .owner
            .values()
            .map(|(id, _)| *id)
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    };
    let dead: Vec<u64> = owners.into_iter().filter(|&id| !alive(world, id)).collect();
    let now = now_ms(world);
    let mut runtime = world.resource_mut::<Runtime>();
    for id in dead {
        release_all(&mut runtime, id);
        runtime.ai.claims.owner.retain(|_, (owner, _)| *owner != id);
    }
    runtime
        .ai
        .claims
        .danger_until_ms
        .retain(|_, until| *until > now);
}

fn claim(world: &mut World, id: u64, node: u32) -> bool {
    if !alive(world, id) {
        return false;
    }
    reap(world);
    let mut runtime = world.resource_mut::<Runtime>();
    if let Some(&(owner, revision)) = runtime.ai.claims.owner.get(&node) {
        if owner != id {
            return false;
        }
        if let Some(actor) = runtime.actors.get_mut(&id) {
            actor.ai.cover.claim = Some((node, revision));
        }
        return true;
    }
    release_all(&mut runtime, id);
    let revision = runtime.ai.claims.next_revision.wrapping_add(1);
    runtime.ai.claims.next_revision = revision;
    runtime.ai.claims.owner.insert(node, (id, revision));
    if let Some(actor) = runtime.actors.get_mut(&id) {
        actor.ai.cover.claim = Some((node, revision));
    }
    true
}

fn node_value(world: &mut World, node: u32) -> Result<Value, String> {
    let graph = FrameWorld::from_world(world).path_graph();
    node_object(&mut world.resource_mut::<Runtime>(), &graph, node)
}

/// The eye height an actor holds at a cover node, or `None` when the node
/// is not cover. Corner nodes are held standing.
fn cover_eye(node_type: u32) -> Option<f32> {
    let height = match cover_stance(node_type)? {
        "crouch" => view_height::CROUCH,
        "prone" => view_height::PRONE,
        _ => view_height::STAND,
    };
    Some(height as f32)
}

/// A point to fight toward, and the entity it belongs to if any.
type Fight = ([f32; 3], Option<u64>);

/// Where the actor's fight is: the passed point or entity, else the last
/// known position of its threat target.
fn target(world: &mut World, id: u64, args: &[Value]) -> Result<Option<Fight>, String> {
    match args.first() {
        Some(Value::Vector(point)) => Ok(Some((*point, None))),
        Some(Value::Object(other)) if world.resource::<Runtime>().entities.contains_key(other) => {
            Ok(Some((eye(world, *other), Some(*other))))
        }
        None | Some(Value::Undefined) => {
            let runtime = world.resource::<Runtime>();
            let threat = &runtime.actors[&id].ai.threat;
            Ok(threat
                .target
                .and_then(|t| threat.memories.get(&t).map(|m| (m.last_known, Some(t)))))
        }
        Some(_) => Err("findcovernode target is not a point or an entity".into()),
    }
}

fn origin(world: &mut World, id: u64) -> [f32; 3] {
    match world.resource_mut::<Runtime>().object_field(id, "origin") {
        Value::Vector(origin) => origin,
        _ => [0.0; 3],
    }
}

fn eye(world: &mut World, id: u64) -> [f32; 3] {
    if let Some(client) = world.resource::<Runtime>().player_client(id)
        && let Some(eye) = player_eye(world, client)
    {
        return eye;
    }
    let at = origin(world, id);
    [at[0], at[1], at[2] + view_height::STAND as f32]
}

/// A living player's view point.
fn player_eye(world: &mut World, client: u32) -> Option<[f32; 3]> {
    let frame = FrameWorld::from_world(world);
    let ps = frame.player(crate::ClientId(client))?;
    (ps.health > 0).then(|| {
        [
            ps.origin[0],
            ps.origin[1],
            ps.origin[2] + ps.view_height_current,
        ]
    })
}

struct Enemy {
    id: u64,
    eye: [f32; 3],
    player: bool,
}

/// Every live player and actor not on the actor's team.
fn enemies(world: &mut World, id: u64, team: &str) -> Vec<Enemy> {
    let (players, actors): (Vec<(u32, u64)>, Vec<u64>) = {
        let runtime = world.resource::<Runtime>();
        (
            runtime
                .players
                .iter()
                .filter(|(_, slot)| &*slot.sessionstate == "playing")
                .map(|(client, slot)| (*client, slot.object))
                .collect(),
            runtime
                .actors
                .iter()
                .filter(|(other, actor)| **other != id && &*actor.team != team)
                .map(|(other, _)| *other)
                .collect(),
        )
    };
    let mut enemies = Vec::new();
    for (client, object) in players {
        if entity_field(world, object, "team") == Value::string(team) {
            continue;
        }
        let Some(eye) = player_eye(world, client) else {
            continue;
        };
        enemies.push(Enemy {
            id: object,
            eye,
            player: true,
        });
    }
    for other in actors {
        if alive(world, other) {
            enemies.push(Enemy {
                id: other,
                eye: eye(world, other),
                player: false,
            });
        }
    }
    enemies
}

#[derive(Clone, Copy, Debug, Default)]
struct Features {
    distance: f32,
    engagement: f32,
    target_direction: f32,
    visibility: f32,
    cover_type: f32,
    node_angle: f32,
    relations: f32,
    player_los: f32,
    priority: f32,
    dangerous: f32,
}

impl Features {
    fn score(&self, p: &AiProfile) -> f32 {
        p.cover_weight_distance * self.distance
            + p.cover_weight_engagement * self.engagement
            + p.cover_weight_target_direction * self.target_direction
            + p.cover_weight_visibility * self.visibility
            + p.cover_weight_type * self.cover_type
            + p.cover_weight_angle * self.node_angle
            + p.cover_weight_relations * self.relations
            + p.cover_weight_player_los * self.player_los
            + p.cover_weight_priority * self.priority
            - p.cover_weight_danger * self.dangerous
    }
}

fn find(world: &mut World, id: u64, args: &[Value]) -> Result<Option<u32>, String> {
    if !alive(world, id) {
        return Ok(None);
    }
    let profile = profile(&mut world.resource_mut::<Runtime>())?;
    reap(world);
    let now = now_ms(world);
    let target = target(world, id, args)?;
    let (holding, previous, next_search_ms) = {
        let runtime = world.resource::<Runtime>();
        let cover = &runtime.actors[&id].ai.cover;
        (held(runtime, id), cover.previous, cover.next_search_ms)
    };
    if now < next_search_ms {
        return Ok(holding);
    }
    let found = search(world, id, &profile, target, [holding, previous], now);
    if found.is_none()
        && let Some(actor) = world.resource_mut::<Runtime>().actors.get_mut(&id)
    {
        actor.ai.cover.next_search_ms = now + profile.cover_search_interval_ms.max(0);
    }
    Ok(found.or(holding))
}

/// The nearest unclaimed cover nodes around the actor, nearest first.
fn candidates(
    graph: &SimPathGraph,
    runtime: &Runtime,
    id: u64,
    at: [f32; 3],
    p: &AiProfile,
    skip: [Option<u32>; 2],
) -> Vec<(f32, u32)> {
    let max2 = p.cover_distance_max * p.cover_distance_max;
    let mut near: Vec<(f32, u32)> = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| is_cover(node.node_type))
        .map(|(i, node)| (distance2(at, node.origin), i as u32))
        .filter(|&(d2, i)| {
            d2 <= max2
                && !skip.contains(&Some(i))
                && runtime
                    .ai
                    .claims
                    .owner
                    .get(&i)
                    .is_none_or(|(owner, _)| *owner == id)
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    near.truncate(p.cover_candidates as usize);
    near
}

fn search(
    world: &mut World,
    id: u64,
    p: &AiProfile,
    target: Option<Fight>,
    skip: [Option<u32>; 2],
    now: i64,
) -> Option<u32> {
    let graph = FrameWorld::from_world(world).path_graph();
    let at = origin(world, id);
    let team: Arc<str> = world.resource::<Runtime>().actors[&id].team.clone();
    let enemies = enemies(world, id, &team);
    let (near, team_key) = {
        let mut runtime = world.resource_mut::<Runtime>();
        let team_key = runtime.symbol(&team);
        (candidates(&graph, &runtime, id, at, p, skip), team_key)
    };
    if near.is_empty() {
        diag::debug!(
            Sim,
            "cover: actor {id} has no cover candidate within {}",
            p.cover_distance_max
        );
        return None;
    }
    let danger: Vec<bool> = {
        let runtime = world.resource::<Runtime>();
        near.iter()
            .map(|(_, node)| {
                runtime
                    .ai
                    .claims
                    .danger_until_ms
                    .get(&(*node, team_key))
                    .is_some_and(|until| *until > now)
            })
            .collect()
    };
    let max2 = p.cover_distance_max * p.cover_distance_max;
    let frame = super::super::presence::settled(world);
    let clear = |from: [f32; 3], to: [f32; 3]| {
        let trace = frame.trace_world(
            from,
            to,
            [0.0; 3],
            [0.0; 3],
            crate::bullet_collision::MASK_SHOT,
        );
        trace.fraction >= 1.0 && trace.startsolid == 0
    };
    let mut best: Option<(f32, u32)> = None;
    for (&(d2, index), dangerous) in near.iter().zip(danger) {
        let node = &graph.nodes[index as usize];
        let lift = cover_eye(node.node_type).unwrap_or(0.0);
        let node_eye = [node.origin[0], node.origin[1], node.origin[2] + lift];
        let (sin, cos) = node.angles[1].to_radians().sin_cos();
        let facing = |point: [f32; 3]| {
            let (dx, dy) = (point[0] - node.origin[0], point[1] - node.origin[1]);
            let length = dx.hypot(dy);
            (length > 0.0).then(|| ((dx * cos + dy * sin) / length, length))
        };
        let mut f = Features {
            distance: if max2 > 0.0 {
                1.0 - d2.clamp(0.0, max2) / max2
            } else {
                0.0
            },
            cover_type: 1.0,
            priority: (u32::from(node.spawnflags) & p.cover_priority_flag != 0) as u32 as f32,
            dangerous: dangerous as u32 as f32,
            ..Default::default()
        };
        if let Some((point, _)) = target {
            let reach = distance2(node.origin, point).sqrt();
            f.engagement = (p.cover_engage_min..=p.cover_engage_max).contains(&reach) as u32 as f32;
            if let Some((dot, length)) = facing(point) {
                f.node_angle = (dot >= 0.0) as u32 as f32;
                if length > p.cover_direction_min {
                    f.target_direction = if dot > p.cover_direction_dot {
                        1.0
                    } else if dot < -p.cover_direction_dot {
                        -1.0
                    } else {
                        0.0
                    };
                }
            }
            f.visibility = clear(node_eye, point) as u32 as f32;
        }
        let target_id = target.and_then(|(_, t)| t);
        let others: Vec<&Enemy> = enemies.iter().filter(|e| Some(e.id) != target_id).collect();
        if !others.is_empty() {
            let faced = others
                .iter()
                .filter(|e| facing(e.eye).is_some_and(|(dot, _)| dot >= 0.0))
                .count();
            f.relations = faced as f32 / others.len() as f32;
        }
        let watcher = enemies.iter().filter(|e| e.player).min_by(|a, b| {
            distance2(a.eye, node_eye)
                .total_cmp(&distance2(b.eye, node_eye))
                .then(a.id.cmp(&b.id))
        });
        f.player_los = watcher.is_none_or(|e| !clear(e.eye, node_eye)) as u32 as f32;
        let score = f.score(p);
        diag::debug!(Sim, "cover: actor {id} node {index} score {score:.2} {f:?}");
        if score > 0.0 && best.is_none_or(|(s, n)| score > s || score == s && index < n) {
            best = Some((score, index));
        }
    }
    diag::debug!(
        Sim,
        "cover: actor {id} chose {:?} from {} candidates",
        best,
        near.len()
    );
    best.map(|(_, node)| node)
}

fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}
