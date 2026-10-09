/// How well one bot aims and how far it sees. The defaults are the host bots'
/// own; a match that rates its bots (survival archetypes) scales them with
/// [`BotTuning::scaled`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotTuning {
    pub max_yaw_deg_per_ms: f32,
    pub max_pitch_deg_per_ms: f32,
    pub max_yaw_accel_deg_per_ms2: f32,
    pub max_pitch_accel_deg_per_ms2: f32,
    /// How far off the target the view may be when the trigger is pulled.
    pub fire_cone_deg: f32,
    /// Full width of the per-bot aim offset drawn from its seed.
    pub aim_error_yaw_deg: f32,
    pub aim_error_pitch_deg: f32,
    pub sight_range_in: f32,
    pub sight_fov_deg: f32,
}

impl Default for BotTuning {
    fn default() -> Self {
        Self {
            max_yaw_deg_per_ms: 0.36,
            max_pitch_deg_per_ms: 0.24,
            max_yaw_accel_deg_per_ms2: 0.015,
            max_pitch_accel_deg_per_ms2: 0.01,
            fire_cone_deg: 8.0,
            aim_error_yaw_deg: 4.0,
            aim_error_pitch_deg: 2.0,
            sight_range_in: 5906.0,
            sight_fov_deg: 90.0,
        }
    }
}

impl BotTuning {
    /// An aim `skill` of 1 is the default bot. Above 1 the bot turns onto a
    /// target faster and holds its shot for a tighter line with a smaller
    /// standing error; below 1 the reverse. Perception is not aim, so sight
    /// range and field of view stay as they are.
    pub fn scaled(self, skill: f32) -> Self {
        Self {
            max_yaw_deg_per_ms: self.max_yaw_deg_per_ms * skill,
            max_pitch_deg_per_ms: self.max_pitch_deg_per_ms * skill,
            max_yaw_accel_deg_per_ms2: self.max_yaw_accel_deg_per_ms2 * skill,
            max_pitch_accel_deg_per_ms2: self.max_pitch_accel_deg_per_ms2 * skill,
            fire_cone_deg: self.fire_cone_deg / skill,
            aim_error_yaw_deg: self.aim_error_yaw_deg / skill,
            aim_error_pitch_deg: self.aim_error_pitch_deg / skill,
            ..self
        }
    }
}
