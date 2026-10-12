//! Actor routes over the path nodes the map ships: script node structs, a
//! bounded A* resumed across frames, and the walk along the found route.

use super::super::args::{arg, string};
use super::super::arrays::new_array;
use crate::SimPathGraph;
use crate::bullet_collision::{MASK_PLAYER_SOLID, PLAYER_MAXS};
use crate::frame::FrameWorld;
use crate::script::Namespace::Function;
use crate::script::runtime::raise;
use crate::script::{BTreeMap, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::World;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

const TICK_S: f32 = crate::MATCH_TICK_MS as f32 / 1000.0;
/// A standing player's run speed; `moveplaybackrate` scales it.
const RUN_SPEED: f32 = 190.0;
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
}

#[derive(Clone, Debug)]
enum Plan {
    Search(Search),
    Walk { points: Vec<[f32; 3]>, next: usize },
    NoRoute,
}

#[derive(Clone, Debug)]
struct Search {
    goal: u32,
    g: Vec<f32>,
    parent: Vec<Option<u32>>,
    closed: Vec<bool>,
    open: BinaryHeap<Reverse<(Cost, u32)>>,
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
    fn begin(graph: &SimPathGraph, start: u32, goal: u32) -> Self {
        let n = graph.nodes.len();
        let mut search = Self {
            goal,
            g: vec![f32::INFINITY; n],
            parent: vec![None; n],
            closed: vec![false; n],
            open: BinaryHeap::new(),
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
                let ng = self.g[i as usize] + length.max(0.0);
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

fn node_object(runtime: &mut Runtime, graph: &SimPathGraph, node: u32) -> Result<Value, String> {
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
    for (key, name) in [
        ("targetname", &facts.targetname),
        ("script_noteworthy", &facts.script_noteworthy),
    ] {
        if let Some(name) = name {
            runtime.set_object_field(id, key, Value::string(name));
        }
    }
    runtime.path_nodes.by_node.insert(node, id);
    runtime.path_nodes.by_object.insert(id, node);
    Ok(Value::Object(id))
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
    let field = |node: &crate::SimPathNode| match key.as_str() {
        "targetname" => node.targetname.clone(),
        "script_noteworthy" => node.script_noteworthy.clone(),
        _ => None,
    };
    Ok((0..graph.nodes.len() as u32)
        .filter(|&i| field(&graph.nodes[i as usize]).is_some_and(|name| *name == *value))
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
/// goal is a node. The walk starts once the search finishes.
pub(crate) fn set_goal(world: &mut World, id: u64, goal: [f32; 3], goal_node: Option<u32>) -> Nav {
    let graph = FrameWorld::from_world(world).path_graph();
    let origin = vector_field(&mut world.resource_mut::<Runtime>(), id, "origin");
    let start = nearest_node(world, &graph, origin);
    let end = goal_node.or_else(|| nearest_node(world, &graph, goal));
    let plan = match (start, end) {
        (Some(start), Some(end)) if start == end => Plan::Walk {
            points: vec![graph.nodes[start as usize].origin, goal],
            next: 0,
        },
        (Some(start), Some(end)) => Plan::Search(Search::begin(&graph, start, end)),
        _ => Plan::NoRoute,
    };
    Nav { goal, plan }
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

/// Moves one actor along its route for a frame and returns the notify it
/// raises, if any.
pub(crate) fn advance(
    world: &mut World,
    id: u64,
    nav: &mut Nav,
    alive: bool,
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
                nav.plan = Plan::Walk { points, next: 0 };
            }
        }
    }
    let Plan::Walk { points, next } = &mut nav.plan else {
        return Some("bad_path");
    };
    if !alive {
        return None;
    }
    let rate = number_field(&mut world.resource_mut::<Runtime>(), id, "moveplaybackrate")
        .unwrap_or(1.0)
        .max(0.0);
    let mut left = RUN_SPEED * rate * TICK_S;
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
    if distance_xy(origin, at) > f32::EPSILON {
        let mut angles = vector_field(&mut runtime, id, "angles");
        angles[1] = math_iw4::vect_to_angles([at[0] - origin[0], at[1] - origin[1], 0.0])[1];
        runtime.set_object_field(id, "angles", Value::Vector(angles));
    }
    runtime.set_object_field(id, "origin", Value::Vector(at));
    (done || arrived(&mut runtime, id, at, nav.goal)).then_some("goal")
}

/// Runs every actor's route for one frame under one shared search budget.
pub(crate) fn advance_all(world: &mut World, alive: impl Fn(&mut World, u64) -> bool) {
    let mut budget = ROUTE_BUDGET;
    let ids: Vec<u64> = world
        .resource::<Runtime>()
        .actors
        .iter()
        .filter(|(_, actor)| actor.nav.is_some())
        .map(|(id, _)| *id)
        .collect();
    for id in ids {
        let Some(mut nav) = world
            .resource_mut::<Runtime>()
            .actors
            .get_mut(&id)
            .and_then(|actor| actor.nav.take())
        else {
            continue;
        };
        let living = alive(world, id);
        let notify = advance(world, id, &mut nav, living, &mut budget);
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
