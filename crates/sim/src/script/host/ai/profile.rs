//! The tuning every actor decision reads, from `survival/ai_profile.csv`:
//! c0 key, c1 value, c2 units, c3 where the value comes from.

use super::super::tables::table;
use crate::script::{Arc, Runtime};

pub(crate) const PROFILE_TABLE: &str = "survival/ai_profile.csv";

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AiProfile {
    pub threat_interval_ms: i64,
    pub threat_visible_bonus: f32,
    pub threat_recent_bonus: f32,
    pub threat_remembered_bonus: f32,
    pub threat_current_target_bonus: f32,
    pub threat_bonus_min: f32,
    pub threat_bonus_max: f32,
    pub threat_distance_range: f32,
    pub threat_distance_scale: f32,
    pub threat_attacker_penalty: f32,
    pub threat_attacker_penalty_floor: f32,
    pub threat_memory_ms: i64,
    pub threat_recent_distance: f32,
    pub sight_range: f32,
    pub sight_cone_cos: f32,
    pub reaction_ms: i64,
    pub accuracy_distance_scale: f32,
    pub accuracy_graph_range: f32,
    pub attacker_count_decrease: f32,
    pub attacker_count_max: u32,
    pub attacker_recent_ms: i64,
    pub target_crouch_factor: f32,
    pub target_prone_factor: f32,
    pub motion_speed_cap: f32,
    pub motion_factor_floor: f32,
    pub motion_speed_divisor: f32,
    pub miss_clearance: f32,
    pub cover_search_interval_ms: i64,
    pub cover_distance_max: f32,
    pub cover_weight_distance: f32,
    pub cover_weight_engagement: f32,
    pub cover_weight_target_direction: f32,
    pub cover_weight_visibility: f32,
    pub cover_weight_type: f32,
    pub cover_weight_angle: f32,
    pub cover_weight_relations: f32,
    pub cover_weight_player_los: f32,
    pub cover_weight_priority: f32,
    pub cover_weight_danger: f32,
    pub cover_direction_min: f32,
    pub cover_direction_dot: f32,
    pub cover_priority_flag: u32,
    pub cover_engage_min: f32,
    pub cover_engage_max: f32,
    pub cover_candidates: u32,
    pub cover_danger_ms: i64,
    pub path_random_cost_percent: f32,
    pub path_choke_cost: f32,
    pub path_overlap_cost: f32,
    pub path_momentum: f32,
    pub path_momentum_min_speed: f32,
    pub friendly_suppression: bool,
    pub friendly_suppression_distance: f32,
    pub friendly_fire_block_ms: i64,
    pub grenade_max_speed: f32,
    pub grenade_gravity: f32,
    pub grenade_landing_error: f32,
    pub grenade_arc_segments: u32,
    pub grenade_cooldown_ms: i64,
    pub grenade_release_height: f32,
}

impl AiProfile {
    fn parse(value: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let number = |key: &str| -> Result<f32, String> {
            let text = value(key).ok_or_else(|| format!("{PROFILE_TABLE} has no `{key}` row"))?;
            text.trim()
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("{PROFILE_TABLE}: `{key}` is not a number: `{text}`"))
        };
        let ms = |key: &str| number(key).map(|v| v as i64);
        let count = |key: &str| number(key).map(|v| v.max(0.0) as u32);
        Ok(Self {
            threat_interval_ms: ms("threat_interval_ms")?,
            threat_visible_bonus: number("threat_visible_bonus")?,
            threat_recent_bonus: number("threat_recent_bonus")?,
            threat_remembered_bonus: number("threat_remembered_bonus")?,
            threat_current_target_bonus: number("threat_current_target_bonus")?,
            threat_bonus_min: number("threat_bonus_min")?,
            threat_bonus_max: number("threat_bonus_max")?,
            threat_distance_range: number("threat_distance_range")?,
            threat_distance_scale: number("threat_distance_scale")?,
            threat_attacker_penalty: number("threat_attacker_penalty")?,
            threat_attacker_penalty_floor: number("threat_attacker_penalty_floor")?,
            threat_memory_ms: ms("threat_memory_ms")?,
            threat_recent_distance: number("threat_recent_distance")?,
            sight_range: number("sight_range")?,
            sight_cone_cos: number("sight_cone_cos")?,
            reaction_ms: ms("reaction_ms")?,
            accuracy_distance_scale: number("accuracy_distance_scale")?,
            accuracy_graph_range: number("accuracy_graph_range")?,
            attacker_count_decrease: number("attacker_count_decrease")?,
            attacker_count_max: count("attacker_count_max")?,
            attacker_recent_ms: ms("attacker_recent_ms")?,
            target_crouch_factor: number("target_crouch_factor")?,
            target_prone_factor: number("target_prone_factor")?,
            motion_speed_cap: number("motion_speed_cap")?,
            motion_factor_floor: number("motion_factor_floor")?,
            motion_speed_divisor: number("motion_speed_divisor")?,
            miss_clearance: number("miss_clearance")?,
            cover_search_interval_ms: ms("cover_search_interval_ms")?,
            cover_distance_max: number("cover_distance_max")?,
            cover_weight_distance: number("cover_weight_distance")?,
            cover_weight_engagement: number("cover_weight_engagement")?,
            cover_weight_target_direction: number("cover_weight_target_direction")?,
            cover_weight_visibility: number("cover_weight_visibility")?,
            cover_weight_type: number("cover_weight_type")?,
            cover_weight_angle: number("cover_weight_angle")?,
            cover_weight_relations: number("cover_weight_relations")?,
            cover_weight_player_los: number("cover_weight_player_los")?,
            cover_weight_priority: number("cover_weight_priority")?,
            cover_weight_danger: number("cover_weight_danger")?,
            cover_direction_min: number("cover_direction_min")?,
            cover_direction_dot: number("cover_direction_dot")?,
            cover_priority_flag: count("cover_priority_flag")?,
            cover_engage_min: number("cover_engage_min")?,
            cover_engage_max: number("cover_engage_max")?,
            cover_candidates: count("cover_candidates")?,
            cover_danger_ms: ms("cover_danger_ms")?,
            path_random_cost_percent: number("path_random_cost_percent")?,
            path_choke_cost: number("path_choke_cost")?,
            path_overlap_cost: number("path_overlap_cost")?,
            path_momentum: number("path_momentum")?,
            path_momentum_min_speed: number("path_momentum_min_speed")?,
            friendly_suppression: number("friendly_suppression")? != 0.0,
            friendly_suppression_distance: number("friendly_suppression_distance")?,
            friendly_fire_block_ms: ms("friendly_fire_block_ms")?,
            grenade_max_speed: number("grenade_max_speed")?,
            grenade_gravity: number("grenade_gravity")?,
            grenade_landing_error: number("grenade_landing_error")?,
            grenade_arc_segments: count("grenade_arc_segments")?,
            grenade_cooldown_ms: ms("grenade_cooldown_ms")?,
            grenade_release_height: number("grenade_release_height")?,
        })
    }
}

/// The match's profile, read from its table once. A missing or broken
/// table is an error each time it is asked for, never a default profile.
pub(crate) fn profile(runtime: &mut Runtime) -> Result<Arc<AiProfile>, String> {
    if let Some(profile) = &runtime.ai.profile {
        return Ok(profile.clone());
    }
    let tables = runtime.tables.clone();
    let rows =
        table(&tables, PROFILE_TABLE).ok_or_else(|| format!("{PROFILE_TABLE} is not loaded"))?;
    let profile = Arc::new(AiProfile::parse(|key| {
        (0..rows.rows)
            .find(|&row| {
                rows.cell(row, 0)
                    .is_some_and(|cell| cell.eq_ignore_ascii_case(key))
            })
            .and_then(|row| rows.cell(row, 1))
            .map(str::to_owned)
    })?);
    runtime.ai.profile = Some(profile.clone());
    Ok(profile)
}
