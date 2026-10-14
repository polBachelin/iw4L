//! Actor routes over the path nodes the map ships: script node structs, a
//! bounded A* resumed across frames, and the walk along the found route.

use super::super::ai;
use super::super::args::{arg, string};
use super::super::arrays::new_array;
use crate::SimPathGraph;
use crate::SimPathNode;
use crate::bullet_collision::{MASK_PLAYER_SOLID, PLAYER_MAXS};
use crate::frame::FrameWorld;
use crate::script::Namespace::Function;
use crate::script::runtime::raise;
use crate::script::{BTreeMap, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

pub(super) const TICK_S: f32 = crate::MATCH_TICK_MS as f32 / 1000.0;
/// Node expansions shared by every actor's search in one frame.
const ROUTE_BUDGET: u32 = 512;
const ENDPOINT_CANDIDATES: usize = 8;
const SIGHT_HEIGHT: f32 = 32.0;
const STEP_HEIGHT: f32 = 18.0;
const GROUND_DROP: f32 = 64.0;
const ARRIVED: f32 = 1.0;

const NODE_TYPES: [&str; 20] = [
    "BAD NODE",
    "Path",
    "Cover Stand",
    "Cover Crouch",
    "Cover Crouch Window",
    "Cover Prone",
    "Cover Right",
    "Cover Left",
    "Ambush",
    "Exposed",
    "Conceal Stand",
    "Conceal Crouch",
    "Conceal Prone",
    "Door",
    "Door Interior",
    "Scripted",
    "Begin",
    "End",
    "Turret",
    "Guard",
];

/// What a node is for, by its raw type. Only the kinds the maps were seen to
/// ship are named; the rest of the type table is unverified for these maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NodeKind {
    Path,
    CoverStand,
    CoverCrouch,
    CoverCrouchWindow,
    CoverProne,
    CoverRight,
    CoverLeft,
    Begin,
    End,
    Unknown(u32),
}

impl NodeKind {
    pub(crate) fn of(node_type: u32) -> Self {
        match node_type {
            1 => Self::Path,
            2 => Self::CoverStand,
            3 => Self::CoverCrouch,
            4 => Self::CoverCrouchWindow,
            5 => Self::CoverProne,
            6 => Self::CoverRight,
            7 => Self::CoverLeft,
            SimPathNode::BEGIN => Self::Begin,
            SimPathNode::END => Self::End,
            other => Self::Unknown(other),
        }
    }
}

pub(crate) fn is_cover(node_type: u32) -> bool {
    cover_stance(node_type).is_some()
}

/// How an actor holds a cover node: the stance it hides in, or the side of
/// the corner it leans out of.
pub(crate) fn cover_stance(node_type: u32) -> Option<&'static str> {
    match NodeKind::of(node_type) {
        NodeKind::CoverStand => Some("stand"),
        NodeKind::CoverCrouch | NodeKind::CoverCrouchWindow => Some("crouch"),
        NodeKind::CoverProne => Some("prone"),
        NodeKind::CoverLeft => Some("left"),
        NodeKind::CoverRight => Some("right"),
        NodeKind::Path | NodeKind::Begin | NodeKind::End | NodeKind::Unknown(_) => None,
    }
}

/// The script struct of each node handed out, so a node compares equal to
/// itself across calls and `setgoalnode` can find its index.
#[derive(Clone, Debug, Default)]
pub(crate) struct NodeObjects {
    by_node: BTreeMap<u32, u64>,
    by_object: BTreeMap<u64, u32>,
}

impl NodeObjects {
    pub(crate) fn roots(&self, pending: &mut Vec<Value>) {
        pending.extend(self.by_object.keys().map(|id| Value::Object(*id)));
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Nav {
    goal: [f32; 3],
    plan: Plan,
    /// The yaw the actor walked this frame along, for it to turn toward.
    pub(super) heading: Option<f32>,
    /// The unit xy direction that yaw faces, carried into the next frame.
    direction: Option<[f32; 2]>,
}

#[derive(Clone, Debug)]
enum Plan {
    Search(Search),
    Walk(Walk),
    NoRoute,
}

/// A found route: the points walked through, the node each stands on
/// (none for the goal), and the point walked toward.
#[derive(Clone, Debug)]
struct Walk {
    points: Vec<[f32; 3]>,
    nodes: Vec<Option<u32>>,
    next: usize,
}

impl Walk {
    /// The traversal the actor is on: it left the start node for the
    /// landing node and has not reached it.
    fn traversal(&self, graph: &SimPathGraph) -> Option<(u32, u32)> {
        let from = (*self.nodes.get(self.next.checked_sub(1)?)?)?;
        let to = (*self.nodes.get(self.next)?)?;
        (graph.nodes.get(from as usize)?.traverse_end == Some(to)).then_some((from, to))
    }

    fn ahead(&self) -> impl Iterator<Item = u32> + '_ {
        self.nodes.iter().skip(self.next).flatten().copied()
    }
}

impl Nav {
    fn walk(&self) -> Option<&Walk> {
        match &self.plan {
            Plan::Walk(walk) => Some(walk),
            Plan::Search(_) | Plan::NoRoute => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Search {
    goal: u32,
    g: Vec<f32>,
    parent: Vec<Option<u32>>,
    closed: Vec<bool>,
    open: BinaryHeap<Reverse<(Cost, u32)>>,
    costs: RouteCosts,
}

/// What a link costs one route request, drawn when the request is made and
/// kept until it ends. Links into each of four node buckets cost their
/// length scaled by that bucket's draw, so requests spread over different
/// routes; links into nodes other actors' routes still cross cost the choke
/// cost in the penalized buckets, never all four; a traversal costs the
/// overlap cost for each actor on it.
#[derive(Clone, Debug, Default)]
struct RouteCosts {
    seed: u32,
    scale: [f32; 4],
    choke_buckets: u8,
    choke_cost: f32,
    busy: BTreeSet<u32>,
    overlap_cost: f32,
    traversing: BTreeMap<(u32, u32), u32>,
}

impl RouteCosts {
    fn draw(runtime: &mut Runtime, profile: &ai::profile::AiProfile) -> Self {
        let percent = profile.path_random_cost_percent.max(0.0) as u32;
        let seed = ai::random(runtime);
        let scale = std::array::from_fn(|_| {
            let roll = ai::random(runtime);
            if percent == 0 {
                1.0
            } else {
                1.0 + (2 * (roll % percent)) as f32 / 100.0
            }
        });
        Self {
            seed,
            scale,
            choke_buckets: (ai::random(runtime) % 15) as u8,
            choke_cost: profile.path_choke_cost.max(0.0),
            busy: BTreeSet::new(),
            overlap_cost: profile.path_overlap_cost.max(0.0),
            traversing: BTreeMap::new(),
        }
    }

    fn bucket(&self, node: u32) -> usize {
        ((node ^ self.seed).wrapping_mul(0x9E37_79B9) >> 30) as usize
    }

    fn link(&self, graph: &SimPathGraph, from: u32, to: u32, length: f32) -> f32 {
        let bucket = self.bucket(to);
        let mut cost = length.max(0.0) * self.scale[bucket];
        if self.choke_buckets & (1 << bucket) != 0 && self.busy.contains(&to) {
            cost += self.choke_cost;
        }
        if graph.nodes[from as usize].traverse_end == Some(to) {
            let users = self.traversing.get(&(from, to)).copied().unwrap_or(0);
            cost += self.overlap_cost * users as f32;
        }
        cost
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cost(f32);

impl Eq for Cost {}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

enum Step {
    Pending,
    Found(Vec<u32>),
    Unreachable,
}

impl Search {
    fn begin(graph: &SimPathGraph, start: u32, goal: u32, costs: RouteCosts) -> Self {
        let n = graph.nodes.len();
        let mut search = Self {
            goal,
            g: vec![f32::INFINITY; n],
            parent: vec![None; n],
            closed: vec![false; n],
            open: BinaryHeap::new(),
            costs,
        };
        search.g[start as usize] = 0.0;
        search
            .open
            .push(Reverse((Cost(search.heuristic(graph, start)), start)));
        search
    }

    fn heuristic(&self, graph: &SimPathGraph, node: u32) -> f32 {
        distance(
            graph.nodes[node as usize].origin,
            graph.nodes[self.goal as usize].origin,
        )
    }

    fn advance(&mut self, graph: &SimPathGraph, budget: &mut u32) -> Step {
        if self.g.len() != graph.nodes.len() {
            return Step::Unreachable;
        }
        while let Some(&Reverse((_, i))) = self.open.peek() {
            if self.closed[i as usize] {
                self.open.pop();
                continue;
            }
            if *budget == 0 {
                return Step::Pending;
            }
            *budget -= 1;
            self.open.pop();
            self.closed[i as usize] = true;
            if i == self.goal {
                let mut route = vec![i];
                let mut at = i;
                while let Some(parent) = self.parent[at as usize] {
                    route.push(parent);
                    at = parent;
                }
                route.reverse();
                return Step::Found(route);
            }
            for &(j, length) in &graph.nodes[i as usize].links {
                let ng = self.g[i as usize] + self.costs.link(graph, i, j, length);
                if !self.closed[j as usize] && ng < self.g[j as usize] {
                    self.g[j as usize] = ng;
                    self.parent[j as usize] = Some(i);
                    let f = ng + self.heuristic(graph, j);
                    self.open.push(Reverse((Cost(f), j)));
                }
            }
        }
        Step::Unreachable
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

fn distance_xy(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn number_field(runtime: &mut Runtime, id: u64, name: &str) -> Option<f32> {
    match runtime.object_field(id, name) {
        Value::Int(v) => Some(v as f32),
        Value::Float(v) => Some(v),
        _ => None,
    }
}

fn vector_field(runtime: &mut Runtime, id: u64, name: &str) -> [f32; 3] {
    match runtime.object_field(id, name) {
        Value::Vector(v) => v,
        _ => [0.0; 3],
    }
}

pub(crate) fn node_object(
    runtime: &mut Runtime,
    graph: &SimPathGraph,
    node: u32,
) -> Result<Value, String> {
    if let Some(id) = runtime.path_nodes.by_node.get(&node) {
        return Ok(Value::Object(*id));
    }
    let facts = &graph.nodes[node as usize];
    let id = runtime.new_object()?;
    runtime.set_object_field(id, "origin", Value::Vector(facts.origin));
    runtime.set_object_field(id, "angles", Value::Vector(facts.angles));
    let kind = NODE_TYPES
        .get(facts.node_type as usize)
        .unwrap_or(&NODE_TYPES[0]);
    runtime.set_object_field(id, "type", Value::string(kind));
    runtime.set_object_field(id, "spawnflags", Value::Int(facts.spawnflags.into()));
    runtime.set_object_field(id, "radius", Value::Float(facts.radius));
    for key in NAME_KEYS {
        if let Some(name) = name_field(facts, key) {
            runtime.set_object_field(id, key, Value::string(name));
        }
    }
    runtime.path_nodes.by_node.insert(node, id);
    runtime.path_nodes.by_object.insert(id, node);
    Ok(Value::Object(id))
}

const NAME_KEYS: [&str; 5] = [
    "targetname",
    "script_noteworthy",
    "target",
    "script_linkname",
    "animscript",
];

fn name_field<'a>(node: &'a SimPathNode, key: &str) -> Option<&'a str> {
    match key {
        "targetname" => node.targetname.as_deref(),
        "script_noteworthy" => node.script_noteworthy.as_deref(),
        "target" => node.target.as_deref(),
        "script_linkname" => node.script_linkname.as_deref(),
        "animscript" => node.animscript.as_deref(),
        _ => None,
    }
}

fn node_objects(world: &mut World, nodes: Vec<u32>) -> Result<Vec<Value>, String> {
    let graph = FrameWorld::from_world(world).path_graph();
    let mut runtime = world.resource_mut::<Runtime>();
    nodes
        .into_iter()
        .map(|node| node_object(&mut runtime, &graph, node))
        .collect()
}

fn matching_nodes(world: &mut World, args: &[Value]) -> Result<Vec<u32>, String> {
    let value = string(args, 0)?;
    let key = string(args, 1)?.to_ascii_lowercase();
    let graph = FrameWorld::from_world(world).path_graph();
    Ok((0..graph.nodes.len() as u32)
        .filter(|&i| name_field(&graph.nodes[i as usize], &key).is_some_and(|name| *name == *value))
        .collect())
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    registry.register(Function, "getnode", |world, _, args| {
        let nodes = matching_nodes(world, args)?;
        if nodes.len() > 1 {
            return Err("getnode used with more than one node".into());
        }
        Ok(node_objects(world, nodes)?
            .pop()
            .unwrap_or(Value::Undefined))
    });
    registry.register(Function, "getnodearray", |world, _, args| {
        let nodes = matching_nodes(world, args)?;
        let values = node_objects(world, nodes)?;
        new_array(world, values)
    });
    registry.register(Function, "getallnodes", |world, _, _| {
        let count = FrameWorld::from_world(world).path_graph().nodes.len() as u32;
        let values = node_objects(world, (0..count).collect())?;
        new_array(world, values)
    });
}

/// The node a script struct stands for, if it is one `getnode` handed out.
pub(crate) fn node_arg(world: &World, args: &[Value], index: usize) -> Result<u32, String> {
    match arg(args, index)? {
        Value::Object(id) => world
            .resource::<Runtime>()
            .path_nodes
            .by_object
            .get(id)
            .copied()
            .ok_or_else(|| "not a path node".into()),
        _ => Err("not a path node".into()),
    }
}

fn clear(world: &mut World, from: [f32; 3], to: [f32; 3]) -> bool {
    let lift = |p: [f32; 3]| [p[0], p[1], p[2] + SIGHT_HEIGHT];
    let trace = FrameWorld::from_world(world).trace_clip(
        lift(from),
        lift(to),
        [0.0; 3],
        [0.0; 3],
        MASK_PLAYER_SOLID,
    );
    trace.fraction >= 1.0 && trace.startsolid == 0
}

/// The nearest node the point sees, else the nearest node.
fn nearest_node(world: &mut World, graph: &SimPathGraph, at: [f32; 3]) -> Option<u32> {
    let mut near: Vec<(f32, u32)> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (distance(at, node.origin), i as u32))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    near.truncate(ENDPOINT_CANDIDATES);
    near.iter()
        .find(|(_, i)| clear(world, at, graph.nodes[*i as usize].origin))
        .or(near.first())
        .map(|(_, i)| *i)
}

/// Plans a route from the actor to `goal`, ending on `goal_node` when the
/// goal is a node. The walk starts once the search finishes; the direction
/// the actor was walking carries over, since scripts re-aim a chase often.
pub(crate) fn set_goal(world: &mut World, id: u64, goal: [f32; 3], goal_node: Option<u32>) -> Nav {
    let graph = FrameWorld::from_world(world).path_graph();
    if graph.nodes.is_empty() {
        static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            diag::warn!(
                Sim,
                "actor path gap: the match has no path graph, so no actor can move (`setgoalpos` answers bad_path)"
            );
        }
    }
    let origin = vector_field(&mut world.resource_mut::<Runtime>(), id, "origin");
    let start = nearest_node(world, &graph, origin);
    let end = goal_node.or_else(|| nearest_node(world, &graph, goal));
    let plan = match (start, end) {
        (Some(start), Some(end)) if start == end => Plan::Walk(Walk {
            points: vec![graph.nodes[start as usize].origin, goal],
            nodes: vec![Some(start), None],
            next: 0,
        }),
        (Some(start), Some(end)) => match route_costs(world, id, &graph) {
            Ok(costs) => Plan::Search(Search::begin(&graph, start, end, costs)),
            Err(error) => {
                static WARNED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    diag::warn!(
                        Sim,
                        "actor path gap: no route costs ({error}), so every searched route answers bad_path"
                    );
                }
                Plan::NoRoute
            }
        },
        _ => Plan::NoRoute,
    };
    let direction = world.resource::<Runtime>().actors[&id]
        .nav
        .as_ref()
        .and_then(|nav| nav.direction);
    Nav {
        goal,
        plan,
        heading: None,
        direction,
    }
}

/// This request's link costs, against where the other live actors' routes
/// go and which traversals they are on now.
fn route_costs(world: &mut World, id: u64, graph: &SimPathGraph) -> Result<RouteCosts, String> {
    let others: Vec<u64> = world
        .resource::<Runtime>()
        .actors
        .iter()
        .filter(|(other, actor)| **other != id && actor.nav.is_some())
        .map(|(other, _)| *other)
        .collect();
    let others: Vec<u64> = others
        .into_iter()
        .filter(|other| super::actor::alive(world, *other))
        .collect();
    let mut runtime = world.resource_mut::<Runtime>();
    let profile = ai::profile::profile(&mut runtime)?;
    let mut costs = RouteCosts::draw(&mut runtime, &profile);
    for other in others {
        let Some(walk) = runtime.actors[&other].nav.as_ref().and_then(Nav::walk) else {
            continue;
        };
        costs.busy.extend(walk.ahead());
        if let Some(edge) = walk.traversal(graph) {
            *costs.traversing.entry(edge).or_default() += 1;
        }
    }
    Ok(costs)
}

fn arrived(runtime: &mut Runtime, id: u64, at: [f32; 3], goal: [f32; 3]) -> bool {
    let radius = number_field(runtime, id, "goalradius").unwrap_or(0.0);
    distance_xy(at, goal) <= radius.max(ARRIVED) && (at[2] - goal[2]).abs() <= PLAYER_MAXS[2]
}

fn snap_to_ground(world: &mut World, at: [f32; 3]) -> [f32; 3] {
    let from = [at[0], at[1], at[2] + STEP_HEIGHT];
    let to = [at[0], at[1], at[2] - GROUND_DROP];
    let trace =
        FrameWorld::from_world(world).trace_clip(from, to, [0.0; 3], [0.0; 3], MASK_PLAYER_SOLID);
    if trace.fraction < 1.0 && trace.startsolid == 0 {
        [at[0], at[1], from[2] + (to[2] - from[2]) * trace.fraction]
    } else {
        at
    }
}

/// Moves one live actor along its route for a frame and returns the notify
/// it raises, if any.
pub(crate) fn advance(
    world: &mut World,
    id: u64,
    nav: &mut Nav,
    budget: &mut u32,
) -> Option<&'static str> {
    let origin = vector_field(&mut world.resource_mut::<Runtime>(), id, "origin");
    if arrived(&mut world.resource_mut::<Runtime>(), id, origin, nav.goal) {
        return Some("goal");
    }
    if let Plan::Search(search) = &mut nav.plan {
        let graph = FrameWorld::from_world(world).path_graph();
        match search.advance(&graph, budget) {
            Step::Pending => return None,
            Step::Unreachable => nav.plan = Plan::NoRoute,
            Step::Found(route) => {
                let mut points: Vec<[f32; 3]> = route
                    .iter()
                    .map(|&node| graph.nodes[node as usize].origin)
                    .collect();
                points.push(nav.goal);
                let mut nodes: Vec<Option<u32>> = route.into_iter().map(Some).collect();
                nodes.push(None);
                // The route starts at the node nearest the actor, which it
                // may already have left for the next one; walking back to it
                // on every replan would hold the actor in place.
                let next = usize::from(
                    distance_xy(origin, points[1]) <= distance_xy(points[0], points[1]),
                );
                nav.plan = Plan::Walk(Walk {
                    points,
                    nodes,
                    next,
                });
            }
        }
    }
    nav.heading = None;
    let Plan::Walk(Walk { points, next, .. }) = &mut nav.plan else {
        return Some("bad_path");
    };
    // The body moves as far as its playing anims carry it, the way an AI
    // moves by its anim's delta; `moveplaybackrate` scales both.
    let mut runtime = world.resource_mut::<Runtime>();
    let rate = number_field(&mut runtime, id, "moveplaybackrate")
        .unwrap_or(1.0)
        .max(0.0);
    let anim_speed = runtime
        .entities
        .get(&id)
        .and_then(|entity| entity.anim_blend.as_deref())
        .map_or(0.0, |blend| blend.move_speed());
    let mut left = step_length(anim_speed, rate);
    let mut at = origin;
    while left > 0.0 && *next < points.len() {
        let to = points[*next];
        let d = distance_xy(at, to);
        if d <= left {
            at = to;
            left -= d;
            *next += 1;
        } else {
            let t = left / d;
            at = std::array::from_fn(|i| at[i] + (to[i] - at[i]) * t);
            left = 0.0;
        }
    }
    let at = snap_to_ground(world, at);
    let done = *next >= points.len();
    let mut runtime = world.resource_mut::<Runtime>();
    let moved = [at[0] - origin[0], at[1] - origin[1]];
    let length = moved[0].hypot(moved[1]);
    nav.direction = (length > f32::EPSILON).then(|| {
        let walked = [moved[0] / length, moved[1] / length];
        let profile = ai::profile::profile(&mut runtime).ok();
        match (nav.direction, profile) {
            (Some(previous), Some(profile))
                if length / TICK_S > profile.path_momentum_min_speed =>
            {
                carry_momentum(walked, previous, profile.path_momentum)
            }
            _ => walked,
        }
    });
    nav.heading = nav
        .direction
        .map(|d| math_iw4::vect_to_angles([d[0], d[1], 0.0])[1]);
    runtime.set_object_field(id, "origin", Value::Vector(at));
    (done || arrived(&mut runtime, id, at, nav.goal)).then_some("goal")
}

/// The direction walked this frame bent toward the one walked before by
/// `momentum`, as a unit vector; the walked one when the two cancel.
fn carry_momentum(walked: [f32; 2], previous: [f32; 2], momentum: f32) -> [f32; 2] {
    let m = momentum.clamp(0.0, 1.0);
    let blend = [
        previous[0] * m + walked[0] * (1.0 - m),
        previous[1] * m + walked[1] * (1.0 - m),
    ];
    let length = blend[0].hypot(blend[1]);
    if length > f32::EPSILON {
        [blend[0] / length, blend[1] / length]
    } else {
        walked
    }
}

/// How far a body moving at `anim_speed` units a second, played at `rate`,
/// goes in one frame.
fn step_length(anim_speed: f32, rate: f32) -> f32 {
    anim_speed * rate * TICK_S
}

/// `yaw` turned toward `goal` by at most `most` degrees, the short way.
pub(super) fn turn_toward(yaw: f32, goal: f32, most: f32) -> f32 {
    let delta = math_iw4::angle_subtract(goal, yaw);
    if delta.abs() <= most {
        goal
    } else {
        yaw + most.copysign(delta)
    }
}

/// Runs every actor's route for one frame under one shared search budget.
/// The actor served first moves one place along each frame, so no actor
/// waits on the same others for the budget every frame. A dead actor's
/// route ends without a notify and spends nothing.
pub(crate) fn advance_all(world: &mut World, alive: impl Fn(&mut World, u64) -> bool) {
    let mut budget = ROUTE_BUDGET;
    let mut ids: Vec<u64> = world
        .resource::<Runtime>()
        .actors
        .iter()
        .filter(|(_, actor)| actor.nav.is_some())
        .map(|(id, _)| *id)
        .collect();
    if !ids.is_empty() {
        let frame = super::super::players::now_ms(world) / i64::from(crate::MATCH_TICK_MS);
        let first = frame.rem_euclid(ids.len() as i64) as usize;
        ids.rotate_left(first);
    }
    for id in ids {
        let Some(mut nav) = world
            .resource_mut::<Runtime>()
            .actors
            .get_mut(&id)
            .and_then(|actor| actor.nav.take())
        else {
            continue;
        };
        if !alive(world, id) {
            continue;
        }
        let notify = advance(world, id, &mut nav, &mut budget);
        if notify.is_none()
            && let Some(actor) = world.resource_mut::<Runtime>().actors.get_mut(&id)
        {
            actor.nav = Some(nav);
        }
        if let Some(notify) = notify {
            raise(world, Value::Object(id), notify, Vec::new());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_path_step_follows_anim_speed() {
        assert_eq!(step_length(0.0, 1.0), 0.0);
        let run = step_length(200.0, 1.0);
        assert!((run - 200.0 * TICK_S).abs() < 1e-4);
        assert!((step_length(200.0, 0.5) - run / 2.0).abs() < 1e-4);
    }

    #[test]
    fn actor_turn_rate_clamps_the_short_way() {
        assert_eq!(turn_toward(0.0, 30.0, 10.0), 10.0);
        assert_eq!(turn_toward(0.0, 5.0, 10.0), 5.0);
        assert_eq!(turn_toward(170.0, -170.0, 10.0), 180.0);
        assert_eq!(turn_toward(-170.0, 170.0, 10.0), -180.0);
    }
}
