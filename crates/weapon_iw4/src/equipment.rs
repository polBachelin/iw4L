pub const WEAPCLASS_THROWINGKNIFE: i32 = 9;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EquipmentRuntimeFacts {
    pub offhand_class: i32,
    pub start_ammo: i32,
    pub clip_size: i32,
    pub impact_damage: i32,
    pub impact_payload_weapon: u32,
    pub fuse_time_ms: i32,

    pub hold_fire_time_ms: i32,

    pub cook_off_hold: bool,

    pub has_detonator: bool,
    pub detonate_delay_ms: i32,
    pub detonate_time_ms: i32,
    pub projectile_rotates: bool,
    pub stickiness: i32,
    pub timed_detonation: bool,

    pub proj_impact_explode: bool,

    pub stick_to_players: bool,
    pub ballistic_blade: bool,
    pub explosion_radius: i32,
    pub explosion_radius_min: i32,
    pub explosion_inner_damage: i32,
    pub explosion_outer_damage: i32,
    pub damage_cone_angle: f32,
    pub missile_guidance: i32,
    pub ignition_delay_ms: i32,
    pub require_lock_to_fire: bool,
    pub projectile_speed: i32,
    pub projectile_speed_up: i32,
    pub projectile_speed_forward: i32,
    pub projectile_speed_relative_up: i32,
    pub refuses_pickup: bool,
    pub projectile_activate_dist: i32,
    pub projectile_explosion_type: i32,
    pub weap_type: i32,
    pub weap_class: i32,
    pub parallel_bounce: Option<[f32; 31]>,
    pub perpendicular_bounce: Option<[f32; 31]>,
}

impl EquipmentRuntimeFacts {
    pub fn is_usable(self) -> bool {
        (self.projectile_speed > 0 || self.stickiness != 0)
            && (self.fuse_time_ms > 0 || self.impact_damage > 0 || self.explosion_inner_damage > 0)
    }

    /// A row the projectile runner can launch (`equipment_facts_for` or
    /// `missile_launch_facts` in sim).
    pub fn launches_projectile(self) -> bool {
        self.is_usable() || self.projectile_speed > 0
    }

    /// Whether a launched projectile can reach the bounce path, which reads
    /// the per-surface `parallelBounce` / `perpendicularBounce` tables.
    pub fn needs_bounce_coefficients(self) -> bool {
        self.launches_projectile() && !self.proj_impact_explode
    }

    pub fn has_bounce_coefficients(self) -> bool {
        self.parallel_bounce.is_some() && self.perpendicular_bounce.is_some()
    }

    pub fn is_throwing_knife(self) -> bool {
        self.weap_class == WEAPCLASS_THROWINGKNIFE
    }

    pub fn is_retrievable_knife(self) -> bool {
        self.is_throwing_knife() || self.ballistic_blade
    }

    pub fn is_offhand(self) -> bool {
        self.offhand_class != 0
    }

    pub fn spawn_clip_count(self) -> i32 {
        self.start_ammo.max(self.clip_size).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frag() -> EquipmentRuntimeFacts {
        EquipmentRuntimeFacts {
            offhand_class: 1,
            fuse_time_ms: 3500,
            projectile_speed: 600,
            explosion_inner_damage: 130,
            ..Default::default()
        }
    }

    #[test]
    fn bouncing_grenade_without_tables_is_flagged() {
        let facts = frag();
        assert!(facts.needs_bounce_coefficients());
        assert!(!facts.has_bounce_coefficients());
        let with_tables = EquipmentRuntimeFacts {
            parallel_bounce: Some([0.5; 31]),
            perpendicular_bounce: Some([0.3; 31]),
            ..facts
        };
        assert!(with_tables.has_bounce_coefficients());
    }

    #[test]
    fn impact_explode_and_non_projectile_rows_need_no_tables() {
        let rocket = EquipmentRuntimeFacts {
            proj_impact_explode: true,
            ..frag()
        };
        assert!(!rocket.needs_bounce_coefficients());
        assert!(!EquipmentRuntimeFacts::default().needs_bounce_coefficients());
    }
}
