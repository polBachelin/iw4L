//! The chance a shot is aimed to hit, and the hit or miss aim it gets.

use super::profile::AiProfile;
use crate::weapon_content::AccuracyGraph;
use glam::Vec3;

#[derive(Clone, Debug, Default)]
pub(crate) struct ShotState {
    pub last_fired_ms: Option<i64>,
    pub last_target: Option<u64>,
}

/// What the target contributes to a shot's chance.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TargetFacts {
    pub player: bool,
    pub stance: &'static str,
    pub velocity: [f32; 3],
    /// Qualifying attackers on the target, the shooter included.
    pub attackers: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Chance {
    pub x: f32,
    pub weapon: f32,
    pub actor: f32,
    pub stance: f32,
    pub motion: f32,
    pub attackers: f32,
    pub p: f32,
}

/// A graph's accuracy at `x`, the distance fraction. Below the first knot
/// the first knot's accuracy holds, past the last the last's; one knot is
/// flat. Knots sharing a distance make a step, the later knot's accuracy
/// holding from that distance on. A graph with no knots, a non-finite
/// value or a decreasing distance has no answer.
pub(crate) fn interpolate(graph: &AccuracyGraph, x: f32) -> Result<f32, String> {
    let knots = &graph.knots;
    let Some(first) = knots.first() else {
        return Err(format!("accuracy graph `{}` has no knots", graph.name));
    };
    if knots.iter().flatten().any(|v| !v.is_finite()) {
        return Err(format!(
            "accuracy graph `{}` has a non-finite knot",
            graph.name
        ));
    }
    if knots.windows(2).any(|pair| pair[1][0] < pair[0][0]) {
        return Err(format!(
            "accuracy graph `{}` has decreasing distances",
            graph.name
        ));
    }
    let Some(at) = knots.iter().rposition(|knot| knot[0] <= x) else {
        return Ok(first[1]);
    };
    let Some(next) = knots.get(at + 1) else {
        return Ok(knots[at][1]);
    };
    let [x0, y0] = knots[at];
    let t = (x - x0) / (next[0] - x0);
    Ok(y0 + (next[1] - y0) * t)
}

pub(crate) fn attacker_factor(count: usize, decrease: f32, max: u32) -> f32 {
    if count <= 1 {
        return 1.0;
    }
    let count = count.min(max.max(1) as usize);
    decrease.powi(count as i32 - 1)
}

/// Accuracy lost to the target crossing the line of fire: its speed across
/// `dir`, capped, over the profile's divisor. No divisor, no loss.
pub(crate) fn motion_factor(profile: &AiProfile, dir: [f32; 3], velocity: [f32; 3]) -> f32 {
    if profile.motion_speed_divisor <= 0.0 {
        return 1.0;
    }
    let Some(lateral) = Vec3::new(-dir[1], dir[0], 0.0).try_normalize() else {
        return 1.0;
    };
    let across = lateral
        .dot(Vec3::from_array(velocity))
        .abs()
        .min(profile.motion_speed_cap);
    (1.0 - across / profile.motion_speed_divisor).max(profile.motion_factor_floor)
}

pub(crate) fn stance_factor(profile: &AiProfile, target: &TargetFacts) -> f32 {
    match (target.player, target.stance) {
        (true, "crouch") => profile.target_crouch_factor,
        (true, "prone") => profile.target_prone_factor,
        _ => 1.0,
    }
}

/// The chance a shot along `dir` at a target `distance` away is aimed to
/// hit. The profile's own factors are the only ones in the product; a
/// factor with no source yet is left out, not guessed.
pub(crate) fn chance(
    profile: &AiProfile,
    graph: &AccuracyGraph,
    actor: f32,
    dir: [f32; 3],
    distance: f32,
    target: &TargetFacts,
) -> Result<Chance, String> {
    let scaled = distance * profile.accuracy_distance_scale;
    let x = (scaled / profile.accuracy_graph_range).min(1.0);
    let weapon = interpolate(graph, x)?;
    let stance = stance_factor(profile, target);
    let motion = motion_factor(profile, dir, target.velocity);
    let attackers = attacker_factor(
        target.attackers,
        profile.attacker_count_decrease,
        profile.attacker_count_max,
    );
    let product = actor * stance * motion * attackers * weapon;
    let p = if product.is_nan() {
        0.0
    } else {
        product.clamp(0.0, 1.0)
    };
    Ok(Chance {
        x,
        weapon,
        actor,
        stance,
        motion,
        attackers,
        p,
    })
}

/// Where a missed shot is aimed: beside the target's bounds, on `side` of
/// the line of fire, far enough out that the line from `from` passes every
/// corner, `clearance` wide at the target's center, at height fraction
/// `height` of the bounds.
pub(crate) fn miss_point(
    from: [f32; 3],
    bounds: ([f32; 3], [f32; 3]),
    clearance: f32,
    side: f32,
    height: f32,
) -> [f32; 3] {
    let (mins, maxs) = (Vec3::from_array(bounds.0), Vec3::from_array(bounds.1));
    let center = (mins + maxs) * 0.5;
    let from = Vec3::from_array(from);
    let flat = Vec3::new(center.x - from.x, center.y - from.y, 0.0);
    let forward = flat.try_normalize().unwrap_or(Vec3::X);
    let lateral = Vec3::new(-forward.y, forward.x, 0.0) * side.signum();
    let depth = flat.length();
    let mut slope = 0.0f32;
    for corner in 0..4 {
        let at = Vec3::new(
            if corner & 1 == 0 { mins.x } else { maxs.x },
            if corner & 2 == 0 { mins.y } else { maxs.y },
            0.0,
        ) - Vec3::new(from.x, from.y, 0.0);
        let ahead = at.dot(forward);
        if ahead > f32::EPSILON {
            slope = slope.max(at.dot(lateral) / ahead);
        }
    }
    let offset = depth * slope + clearance;
    let z = mins.z + (maxs.z - mins.z) * height.clamp(0.0, 1.0);
    let point = Vec3::new(center.x, center.y, z) + lateral * offset;
    point.to_array()
}

/// Whether the segment from `start` to `end` passes through the box.
pub(crate) fn segment_hits_box(
    start: [f32; 3],
    end: [f32; 3],
    bounds: ([f32; 3], [f32; 3]),
) -> bool {
    let (mut enter, mut exit) = (0.0f32, 1.0f32);
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        let (lo, hi) = (bounds.0[axis], bounds.1[axis]);
        if delta.abs() <= f32::EPSILON {
            if start[axis] < lo || start[axis] > hi {
                return false;
            }
            continue;
        }
        let (a, b) = ((lo - start[axis]) / delta, (hi - start[axis]) / delta);
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
        if enter > exit {
            return false;
        }
    }
    true
}
