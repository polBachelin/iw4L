//! What each actor knows of its enemies and which one it fights.

use super::super::natives::actor::{actor, alive};
use super::super::natives::engine::trace_ignore;
use super::super::players::{entity_field, now_ms};
use super::super::presence::settle_collision;
use super::super::tables::table;
use super::profile::{AiProfile, profile};
use crate::bullet_collision::{BulletTraceQuery, ColliderId, MASK_SHOT, TraceOutcome};
use crate::frame::FrameWorld;
use crate::script::Namespace::Method;
use crate::script::{Arc, BTreeMap, NativeRegistry, Runtime, Value};
use crate::{ClientId, ScriptModelId};
use bevy_ecs::prelude::World;

#[derive(Clone, Debug, Default)]
pub(crate) struct ActorThreat {
    pub target: Option<u64>,
    pub memories: BTreeMap<u64, TargetMemory>,
    pub next_update_ms: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TargetMemory {
    pub last_seen_ms: Option<i64>,
    pub last_known: [f32; 3],
    pub visible: bool,
    pub checked_ms: i64,
    pub acquired_ms: i64,
}

/// Per target, the actors attacking it this tick, from one snapshot so the
/// order actors are visited in never biases accuracy.
#[derive(Clone, Debug, Default)]
pub(crate) struct AttackerSnapshot {
    pub by_target: BTreeMap<u64, Vec<u64>>,
}

const SETTINGS_TABLE: &str = "survival/settings.csv";

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Method, "getthreat", get_threat);
}

/// `self getthreat()`: the enemy the actor fights, as it knows it, or
/// undefined. `position` is where it was last seen, never where it is.
fn get_threat(world: &mut World, receiver: &Value, _: &[Value]) -> Result<Value, String> {
    let id = actor(world, receiver)?;
    let now = now_ms(world);
    let mut runtime = world.resource_mut::<Runtime>();
    let reaction_ms = profile(&mut runtime)?.reaction_ms;
    let threat = &runtime.actors[&id].ai.threat;
    let Some((target, memory)) = threat
        .target
        .filter(|target| runtime.live(target))
        .and_then(|target| Some((target, threat.memories.get(&target)?.clone())))
    else {
        return Ok(Value::Undefined);
    };
    let object = runtime.new_object()?;
    let fields = [
        ("entity", Value::Object(target)),
        ("position", Value::Vector(memory.last_known)),
        ("visible", Value::Int(memory.visible.into())),
        (
            "seen",
            Value::Int(memory.last_seen_ms.map_or(-1, |ms| ms as i32)),
        ),
        (
            "reacted",
            Value::Int((now >= memory.acquired_ms + reaction_ms).into()),
        ),
    ];
    for (name, value) in fields {
        runtime.set_object_field(object, name, value);
    }
    Ok(Value::Object(object))
}

/// Which collider counts as the candidate's own body when a sight trace
/// stops on it.
#[derive(Clone, Copy, Debug)]
enum Body {
    Player(u32),
    Model(Option<ScriptModelId>),
}

impl Body {
    fn struck(self, collider: ColliderId) -> bool {
        match (self, collider) {
            (Self::Player(client), ColliderId::Player { client: hit, .. }) => hit.0 == client,
            (
                Self::Model(Some(model)),
                ColliderId::EntityDObjBone { owner, .. }
                | ColliderId::EntityLinkedBrush { owner, .. },
            ) => owner.script_model() == Some(model),
            _ => false,
        }
    }
}

/// A live combatant this tick: where it stands and the points sight is
/// traced to.
#[derive(Clone, Debug)]
struct Combatant {
    team: Arc<str>,
    origin: [f32; 3],
    head: [f32; 3],
    body: [f32; 3],
    hitbox: Body,
}

struct EyeHeights {
    stand: f32,
    crouch: f32,
}

fn eye_heights(runtime: &Runtime) -> Result<EyeHeights, String> {
    let rows = table(&runtime.tables, SETTINGS_TABLE)
        .ok_or_else(|| format!("{SETTINGS_TABLE} is not loaded"))?;
    let number = |key: &str| -> Result<f32, String> {
        (0..rows.rows)
            .find(|&row| {
                rows.cell(row, 0)
                    .is_some_and(|cell| cell.eq_ignore_ascii_case(key))
            })
            .and_then(|row| rows.cell(row, 1))
            .and_then(|text| text.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("{SETTINGS_TABLE} has no numeric `{key}` row"))
    };
    Ok(EyeHeights {
        stand: number("actor_eye_height")?,
        crouch: number("actor_crouch_eye_height")?,
    })
}

fn vector_field(runtime: &mut Runtime, id: u64, name: &str) -> [f32; 3] {
    match runtime.object_field(id, name) {
        Value::Vector(v) => v,
        _ => [0.0; 3],
    }
}

fn combatants(world: &mut World, eyes: &EyeHeights) -> BTreeMap<u64, Combatant> {
    let mut out = BTreeMap::new();
    let players: Vec<(u32, u64)> = world
        .resource::<Runtime>()
        .players
        .iter()
        .filter(|(_, slot)| &*slot.sessionstate == "playing")
        .map(|(client, slot)| (*client, slot.object))
        .collect();
    for (client, id) in players {
        if !world.resource::<Runtime>().live(&id) {
            continue;
        }
        let health = match entity_field(world, id, "health") {
            Value::Int(n) => n as f32,
            Value::Float(n) => n,
            _ => 0.0,
        };
        let Value::String(team) = entity_field(world, id, "team") else {
            continue;
        };
        let Some((origin, height)) = FrameWorld::from_world(world)
            .player(ClientId(client))
            .map(|ps| (ps.origin, ps.view_height_current))
        else {
            continue;
        };
        if health <= 0.0 {
            continue;
        }
        out.insert(
            id,
            Combatant {
                team: Arc::from(&*team),
                origin,
                head: add_z(origin, height),
                body: add_z(origin, height * 0.5),
                hitbox: Body::Player(client),
            },
        );
    }
    let actors: Vec<u64> = world.resource::<Runtime>().actors.keys().copied().collect();
    for id in actors {
        if !alive(world, id) {
            continue;
        }
        let mut runtime = world.resource_mut::<Runtime>();
        let origin = vector_field(&mut runtime, id, "origin");
        let height = if runtime.object_field(id, "pose") == Value::string("crouch") {
            eyes.crouch
        } else {
            eyes.stand
        };
        let hitbox = Body::Model(runtime.entities.get(&id).and_then(|e| e.presence));
        out.insert(
            id,
            Combatant {
                team: runtime.actors[&id].team.clone(),
                origin,
                head: add_z(origin, height),
                body: add_z(origin, height * 0.5),
                hitbox,
            },
        );
    }
    out
}

fn add_z(v: [f32; 3], z: f32) -> [f32; 3] {
    [v[0], v[1], v[2] + z]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// The range and cone gates sight must pass before any trace is spent.
pub(crate) fn in_sight_cone(
    profile: &AiProfile,
    eye: [f32; 3],
    forward: [f32; 3],
    point: [f32; 3],
) -> bool {
    let d = sub(point, eye);
    let dist_sq = dot(d, d);
    if profile.sight_range > 0.0 && dist_sq > profile.sight_range * profile.sight_range {
        return false;
    }
    if profile.sight_cone_cos > -1.0 {
        let along = dot(forward, d);
        if along < 0.0 || along * along < profile.sight_cone_cos * profile.sight_cone_cos * dist_sq
        {
            return false;
        }
    }
    true
}

/// The next due time after an update that was due at `deadline`. Catching up
/// lands on the same phase within the interval instead of replaying every
/// missed update.
pub(crate) fn next_deadline(deadline: i64, now: i64, interval: i64) -> i64 {
    let next = deadline + interval;
    if next > now {
        return next;
    }
    (now.div_euclid(interval) + 1) * interval + next.rem_euclid(interval)
}

/// The first deadline of an actor not yet scheduled: spread over one
/// interval by id, so a cohort spawned on one tick does not trace together.
fn first_deadline(id: u64, now: i64, interval: i64) -> i64 {
    let spread = (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as i64;
    now + spread.rem_euclid(interval)
}

/// One candidate's score, term by term.
#[derive(Clone, Copy, Debug, Default)]
struct Score {
    evidence: i64,
    current: i64,
    distance: i64,
    attackers: i64,
}

impl Score {
    fn total(self) -> i64 {
        self.evidence + self.current + self.distance + self.attackers
    }
}

fn score(
    profile: &AiProfile,
    now: i64,
    origin: [f32; 3],
    memory: &TargetMemory,
    current: bool,
    attackers: usize,
) -> Score {
    let d = distance(origin, memory.last_known);
    let remembered = memory
        .last_seen_ms
        .is_some_and(|seen| now - seen <= profile.threat_memory_ms);
    let evidence = if memory.visible {
        profile.threat_visible_bonus
    } else if remembered && d < profile.threat_recent_distance {
        profile.threat_recent_bonus
    } else if remembered {
        profile.threat_remembered_bonus
    } else {
        0.0
    };
    let current = if current {
        profile
            .threat_current_target_bonus
            .clamp(profile.threat_bonus_min, profile.threat_bonus_max)
    } else {
        0.0
    };
    let distance = if d < profile.threat_distance_range {
        let r = profile.threat_distance_range - d;
        (profile.threat_distance_scale * r * r).round()
    } else {
        0.0
    };
    let attackers = (-profile.threat_attacker_penalty * attackers as f32)
        .max(profile.threat_attacker_penalty_floor);
    Score {
        evidence: evidence.round() as i64,
        current: current.round() as i64,
        distance: distance as i64,
        attackers: attackers.round() as i64,
    }
}

/// Clears the target and every shot state aimed at it.
fn set_target(runtime: &mut Runtime, id: u64, target: Option<u64>) {
    let ai = &mut runtime.actors.get_mut(&id).expect("actor").ai;
    if ai.threat.target != target {
        ai.threat.target = target;
        ai.shot = Default::default();
    }
}

pub(crate) fn update(world: &mut World) {
    let now = now_ms(world);
    let (profile, eyes) = {
        let mut runtime = world.resource_mut::<Runtime>();
        match profile(&mut runtime).and_then(|p| Ok((p, eye_heights(&runtime)?))) {
            Ok(found) => found,
            Err(error) => {
                diag::debug!(Sim, "threat: {error}");
                return;
            }
        }
    };
    let live = combatants(world, &eyes);
    let interval = profile.threat_interval_ms;

    let mut due = Vec::new();
    {
        let mut runtime = world.resource_mut::<Runtime>();
        let ids: Vec<u64> = runtime.actors.keys().copied().collect();
        for id in ids {
            if !live.contains_key(&id) {
                set_target(&mut runtime, id, None);
                runtime
                    .actors
                    .get_mut(&id)
                    .expect("actor")
                    .ai
                    .threat
                    .memories
                    .clear();
                continue;
            }
            let threat = &runtime.actors[&id].ai.threat;
            let stale = threat.target.filter(|target| !live.contains_key(target));
            if stale.is_some() {
                set_target(&mut runtime, id, None);
            }
            let threat = &mut runtime.actors.get_mut(&id).expect("actor").ai.threat;
            threat
                .memories
                .retain(|target, _| live.contains_key(target));
            if interval <= 0 {
                due.push(id);
                continue;
            }
            let deadline = *threat
                .next_update_ms
                .get_or_insert_with(|| first_deadline(id, now, interval));
            if deadline <= now {
                threat.next_update_ms = Some(next_deadline(deadline, now, interval));
                due.push(id);
            }
        }
    }
    if !due.is_empty() {
        settle_collision(world);
    }
    for id in due {
        think(world, &profile, &live, id, now);
    }
    publish_attackers(world, &profile, now);
}

/// Sight, memory and choice for one due actor.
fn think(
    world: &mut World,
    profile: &AiProfile,
    live: &BTreeMap<u64, Combatant>,
    id: u64,
    now: i64,
) {
    let me = &live[&id];
    let forward = {
        let mut runtime = world.resource_mut::<Runtime>();
        math_iw4::angle_vectors(vector_field(&mut runtime, id, "angles")).0
    };
    let ignore = trace_ignore(world, Some(&Value::Object(id)));
    let mut sighted = Vec::new();
    {
        let frame = FrameWorld::from_world(world);
        let sees =
            |point: [f32; 3], hitbox: Body| match frame.current_sensor_trace(BulletTraceQuery {
                start: me.head,
                end: point,
                mask: MASK_SHOT,
                ignore: ignore.client,
                ignore_hit: ignore.other_client,
                ignore_model: ignore.model,
            }) {
                TraceOutcome::Miss { .. } => true,
                TraceOutcome::Hit { collider, .. } => hitbox.struck(collider),
                _ => false,
            };
        for (&other, them) in live {
            if other == id || them.team == me.team {
                continue;
            }
            let visible = in_sight_cone(profile, me.head, forward, them.head)
                && (sees(them.head, them.hitbox) || sees(them.body, them.hitbox));
            sighted.push((other, visible));
        }
    }

    let mut runtime = world.resource_mut::<Runtime>();
    let attackers: BTreeMap<u64, usize> = runtime
        .actors
        .iter()
        .filter(|(other, _)| **other != id)
        .filter_map(|(_, a)| a.ai.threat.target)
        .fold(BTreeMap::new(), |mut counts, target| {
            *counts.entry(target).or_default() += 1;
            counts
        });
    let threat = &mut runtime.actors.get_mut(&id).expect("actor").ai.threat;
    for (other, visible) in sighted {
        if visible {
            let memory = threat.memories.entry(other).or_default();
            if !memory.visible {
                memory.acquired_ms = now;
            }
            memory.visible = true;
            memory.last_seen_ms = Some(now);
            memory.last_known = live[&other].origin;
            memory.checked_ms = now;
        } else if let Some(memory) = threat.memories.get_mut(&other) {
            memory.visible = false;
            memory.checked_ms = now;
        }
    }
    threat.memories.retain(|_, memory| {
        memory.visible
            || memory
                .last_seen_ms
                .is_some_and(|seen| now - seen <= profile.threat_memory_ms)
    });

    let mut best: Option<(u64, i64)> = None;
    let mut log = String::new();
    for (&other, memory) in &threat.memories {
        let terms = score(
            profile,
            now,
            me.origin,
            memory,
            threat.target == Some(other),
            attackers.get(&other).copied().unwrap_or(0),
        );
        let total = terms.total();
        log.push_str(&format!(
            " {other}[evidence={} current={} distance={} attackers={} total={total}]",
            terms.evidence, terms.current, terms.distance, terms.attackers
        ));
        if best.is_none_or(|(_, top)| total > top) {
            best = Some((other, total));
        }
    }
    let before = threat.target;
    let chosen = best.map(|(other, _)| other);
    set_target(&mut runtime, id, chosen);
    if !log.is_empty() || before != chosen {
        diag::debug!(Sim, "threat {id}:{log} target {before:?} -> {chosen:?}");
    }
}

/// Who attacks whom, from this tick's targets and recent shots only.
fn publish_attackers(world: &mut World, profile: &AiProfile, now: i64) {
    let mut runtime = world.resource_mut::<Runtime>();
    let mut by_target: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for (&id, actor) in &runtime.actors {
        let (Some(target), Some(fired)) = (actor.ai.threat.target, actor.ai.shot.last_fired_ms)
        else {
            continue;
        };
        if now - fired <= profile.attacker_recent_ms {
            by_target.entry(target).or_default().push(id);
        }
    }
    runtime.ai.attackers.by_target = by_target;
}

pub(crate) fn forget(runtime: &mut Runtime, id: u64) {
    for actor in runtime.actors.values_mut() {
        actor.ai.threat.memories.remove(&id);
        if actor.ai.threat.target == Some(id) {
            actor.ai.threat.target = None;
            actor.ai.shot = Default::default();
        }
    }
    runtime.ai.attackers.by_target.remove(&id);
}
