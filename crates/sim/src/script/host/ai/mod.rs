//! Actor decisions the engine owns: what an actor knows of its enemies,
//! which one it fights, whether a shot lands, which cover it takes and
//! whether a throw can reach. Scripts sequence the actions; these services
//! answer them. Every number comes from the profile.

pub(crate) mod accuracy;
pub(crate) mod cover;
pub(crate) mod grenade;
pub(crate) mod profile;
pub(crate) mod shot;
pub(crate) mod suppression;
pub(crate) mod threat;

use crate::script::{Arc, NativeRegistry, Runtime};
use bevy_ecs::prelude::World;

/// Match-wide decision state. It lives in `Runtime`, so a map reset wipes
/// it with every actor it refers to.
#[derive(Clone, Debug, Default)]
pub(crate) struct AiState {
    pub profile: Option<Arc<profile::AiProfile>>,
    /// Draws for hit rolls and decisions only, apart from the script's
    /// stream, so cosmetic script draws never shift an outcome.
    pub rng: u32,
    pub claims: cover::NodeClaims,
    pub attackers: threat::AttackerSnapshot,
}

/// One actor's decision state, keyed by its object id: ids are never reused
/// within a runtime, so a stale reference can only miss, never alias.
#[derive(Clone, Debug, Default)]
pub(crate) struct ActorAi {
    pub threat: threat::ActorThreat,
    pub shot: accuracy::ShotState,
    pub cover: cover::ActorCover,
    pub grenade: grenade::ActorGrenade,
    pub fire_block: suppression::FireBlock,
}

pub(crate) fn seed(runtime: &mut Runtime, fingerprint: &[u8; 32]) {
    runtime.ai.rng = u32::from_le_bytes(fingerprint[4..8].try_into().unwrap()) | 1;
}

pub(crate) fn random(runtime: &mut Runtime) -> u32 {
    let mut x = runtime.ai.rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    runtime.ai.rng = x;
    x
}

/// A draw in [0, 1).
pub(crate) fn random_unit(runtime: &mut Runtime) -> f32 {
    (random(runtime) >> 8) as f32 / (1u32 << 24) as f32
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    threat::register(registry);
    shot::register(registry);
    cover::register(registry);
    grenade::register(registry);
}

/// The per-tick services, after this tick's deletes and before routes
/// advance: perception and threat, then the attacker snapshot accuracy
/// reads.
pub(crate) fn perceive(world: &mut World) {
    threat::update(world);
    suppression::update(world);
}

/// Everything an entity owned or was remembered by, released as it is
/// deleted.
pub(crate) fn forget(runtime: &mut Runtime, id: u64) {
    cover::release_all(runtime, id);
    threat::forget(runtime, id);
}
