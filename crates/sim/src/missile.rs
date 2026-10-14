use crate::combat::{AcceptedShot, spread_direction_on_plane};
use crate::equipment::{
    GrenadeLaunchKind, ProjectileOwner, ProjectileState, launch_grenade, spawn_grenade_projectile,
};
use crate::frame::FrameWorld;
use crate::identities::MatchRng;
use entity_iw4::{
    TR_LINEAR, Trajectory, fire_grenade_no_draw_ms, fire_missile_apos, truncated_tr_delta,
};
use math_iw4::vec3_length;
use weapon_iw4::{FireWeaponKind, ROCKET_SPREAD_PLANE, fire_weapon_kind};

pub(crate) fn fire_accepted_shot(world: &mut FrameWorld, tick: crate::Tick, shot: &AcceptedShot) {
    let Some(combat) = world.combat_facts_for(shot.weapon) else {
        return;
    };
    match fire_weapon_kind(combat.weap_type, combat.weap_class) {
        Some(FireWeaponKind::Bullet) => {}
        Some(FireWeaponKind::GrenadeLauncher) => {
            spawn_grenade_projectile(
                world,
                shot.attacker,
                shot.weapon,
                tick,
                shot.origin,
                shot.angles,
                shot.owner_velocity,
                GrenadeLaunchKind::Launcher,
            );
        }
        Some(FireWeaponKind::Missile) => {
            fire_missile(world, tick, shot);
        }
        Some(FireWeaponKind::ThrownGrenade) => {
            spawn_grenade_projectile(
                world,
                shot.attacker,
                shot.weapon,
                tick,
                shot.origin,
                shot.angles,
                shot.owner_velocity,
                GrenadeLaunchKind::Thrown {
                    remaining_fuse_ms: None,
                },
            );
        }
        None => {}
    }
}

pub(crate) fn magic_bullet(
    world: &mut FrameWorld,
    tick: crate::Tick,
    owner: crate::ClientId,
    weapon: u32,
    start: [f32; 3],
    end: [f32; 3],
) -> Result<ProjectileState, String> {
    let combat = world
        .combat_facts_for(weapon)
        .ok_or_else(|| format!("weapon {weapon} has no combat facts"))?;
    let attacker_life = world
        .client_meta(owner)
        .ok_or("MagicBullet owner is not connected")?
        .life_sequence;
    let shot = AcceptedShot {
        shot_id: crate::ShotId(0),
        fire_cause: None,
        attacker: owner,
        attacker_life,
        hand: 0,
        weapon,
        ammo_used: 0,
        origin: start,
        angles: math_iw4::vect_to_angles(std::array::from_fn(|i| end[i] - start[i])),
        ads_frac: 1.0,
        view_height_current: 0.0,
        aim_spread_scale: 0.0,
        perks0: 0,
        combat_seed: 0,
        owner_velocity: [0.0; 3],
        spread_degrees: 0.0,
    };
    let launched = match fire_weapon_kind(combat.weap_type, combat.weap_class) {
        Some(FireWeaponKind::Missile) => fire_missile(world, tick, &shot),
        Some(kind @ (FireWeaponKind::GrenadeLauncher | FireWeaponKind::ThrownGrenade)) => {
            let launch = if kind == FireWeaponKind::GrenadeLauncher {
                GrenadeLaunchKind::Launcher
            } else {
                GrenadeLaunchKind::Thrown {
                    remaining_fuse_ms: None,
                }
            };
            spawn_grenade_projectile(
                world,
                owner,
                weapon,
                tick,
                start,
                shot.angles,
                [0.0; 3],
                launch,
            )
            .then(|| {
                let now = crate::level_time_ms(tick);
                crate::frame::collect_projectiles(world.ecs())
                    .into_iter()
                    .filter(|p| {
                        p.owner.client() == Some(owner)
                            && p.weapon == weapon
                            && p.spawn_time_ms == now
                    })
                    .max_by_key(|p| p.id.0)
            })
            .flatten()
        }
        Some(FireWeaponKind::Bullet) | None => {
            return Err("MagicBullet bullets are not simulated".into());
        }
    };
    launched.ok_or_else(|| format!("weapon {weapon} launched no projectile"))
}

fn fire_missile(
    world: &mut FrameWorld,
    tick: crate::Tick,
    shot: &AcceptedShot,
) -> Option<ProjectileState> {
    let Some(facts) = world.missile_launch_facts(shot.weapon) else {
        diag::warn!(
            Sim,
            "weapon {} fires no missile: no projectile speed",
            shot.weapon
        );
        return None;
    };
    let mut rng = MatchRng::new(shot.combat_seed as u64);
    let mut dir = spread_direction_on_plane(
        shot.angles,
        shot.spread_degrees,
        &mut rng,
        ROCKET_SPREAD_PLANE,
    );
    let mut origin = shot.origin;
    if facts.missile_guidance == 3 {
        dir = glam::Vec3::new(dir[0], dir[1], dir[2] + 0.3)
            .normalize()
            .to_array();
        let (_, right, _) = math_iw4::angle_vectors(shot.angles);
        origin = core::array::from_fn(|i| origin[i] + right[i] * 2.5);
    }
    let lock = world
        .client_meta(shot.attacker)
        .map(|m| m.weapon_lock)
        .filter(|lock| lock.can_fire(shot.weapon, shot.attacker_life.0));
    let guide = crate::MissileGuide {
        target: lock.and_then(|lock| lock.aim),
        top: lock.is_some_and(|lock| lock.flags & 4 != 0),
        ..Default::default()
    };
    let speed = facts.projectile_speed as f32;
    let gun_vel = shot.owner_velocity;
    push_missile(
        world,
        tick,
        ProjectileOwner::Client {
            id: shot.attacker,
            life: shot.attacker_life,
        },
        shot.weapon,
        origin,
        dir,
        core::array::from_fn(|i| dir[i] * speed + gun_vel[i]),
        guide,
        facts.missile_guidance,
    )
}

/// Spawns a projectile of `weapon` owned by the script entity `owner`, launched from `origin`
/// at the world `velocity`. A grenade's fuse comes from its weapon facts.
pub(crate) fn spawn_entity_projectile(
    world: &mut FrameWorld,
    tick: crate::Tick,
    owner: crate::ScriptModelId,
    weapon: u32,
    origin: [f32; 3],
    velocity: [f32; 3],
) -> Result<crate::ProjectileId, String> {
    let combat = world
        .combat_facts_for(weapon)
        .ok_or_else(|| format!("weapon {weapon} has no combat facts"))?;
    let speed = vec3_length(velocity);
    if speed < 1e-6 {
        return Err("projectile velocity is zero".into());
    }
    let dir = velocity.map(|v| v / speed);
    let owner = ProjectileOwner::Entity(owner);
    let launched = match fire_weapon_kind(combat.weap_type, combat.weap_class) {
        Some(FireWeaponKind::Missile) => {
            let facts = world
                .missile_launch_facts(weapon)
                .ok_or_else(|| format!("weapon {weapon} fires no missile"))?;
            push_missile(
                world,
                tick,
                owner,
                weapon,
                origin,
                dir,
                velocity,
                crate::MissileGuide::default(),
                facts.missile_guidance,
            )
            .map(|projectile| projectile.id)
        }
        Some(FireWeaponKind::GrenadeLauncher) => launch_grenade(
            world,
            owner,
            weapon,
            tick,
            origin,
            dir,
            velocity,
            GrenadeLaunchKind::Launcher,
        ),
        Some(FireWeaponKind::ThrownGrenade) => launch_grenade(
            world,
            owner,
            weapon,
            tick,
            origin,
            dir,
            velocity,
            GrenadeLaunchKind::Thrown {
                remaining_fuse_ms: None,
            },
        ),
        Some(FireWeaponKind::Bullet) | None => {
            return Err(format!("weapon {weapon} fires no projectile"));
        }
    };
    launched.ok_or_else(|| format!("weapon {weapon} launched no projectile"))
}

fn push_missile(
    world: &mut FrameWorld,
    tick: crate::Tick,
    owner: ProjectileOwner,
    weapon: u32,
    origin: [f32; 3],
    dir: [f32; 3],
    raw_velocity: [f32; 3],
    guide: crate::MissileGuide,
    missile_guidance: i32,
) -> Option<ProjectileState> {
    let id = world.allocate_projectile_id();
    let entnum = match world.allocate_dynamic_entity(crate::gentity::EntityRunKind::Missile) {
        Ok(entity) => entity.number(),
        Err(error) => {
            diag::warn!(Sim, "missile not spawned: {error:?}");
            return None;
        }
    };
    let time_ms = crate::level_time_ms(tick);
    let velocity = truncated_tr_delta(raw_velocity);
    let raw_speed = vec3_length(raw_velocity);
    let pos = Trajectory {
        tr_time: time_ms,
        tr_type: if missile_guidance == 3 {
            entity_iw4::TR_GRAVITY
        } else {
            TR_LINEAR
        },
        tr_duration: 0,
        tr_delta: velocity,
        tr_base: origin,
    };
    perf::projectile(weapon);
    let projectile = ProjectileState {
        id,
        owner,
        weapon,
        origin,
        velocity,
        pos,
        apos: fire_missile_apos(dir),
        entnum,
        launch_time: time_ms + fire_grenade_no_draw_ms(raw_speed),
        spawn_time_ms: time_ms,
        detonate_at_ms: None,
        cleanup_at_ms: time_ms.saturating_add(crate::equipment::ROCKET_CLEANUP_MS),
        travel_distance: 0.0,
        live: true,
        stuck_pane: None,
        grounded: false,
        guide,
        attached_to: None,
    };
    world.push_projectile(projectile);
    Some(projectile)
}
