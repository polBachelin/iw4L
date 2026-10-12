use std::sync::Arc;

use anim_iw4::{
    ANIM_COND_AKIMBO, ANIM_COND_FASTMANTLE, ANIM_COND_FIRING, ANIM_COND_MOUNTED,
    ANIM_COND_MOVETYPE, ANIM_COND_PERK, ANIM_COND_PLAYERANIMTYPE, ANIM_COND_PLAYERANIMTYPEPRIMARY,
    ANIM_COND_RIOTSHIELDNEXT, ANIM_COND_STRAFING, ANIM_COND_WEAPON_POSITION, ANIM_COND_WEAPONCLASS,
    ANIM_ET_DEATH, PLAYER_ANIM_INDEX_MASK, PlayerAnimValue, anim_weapon_position_from_pm_flags,
    random,
};
use playerstate_iw4::{PlayerState, get_viewmodel_weapon_index};
use weapon_iw4::WeaponCombatFacts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimScriptCommand {
    pub body_part: u8,
    pub anim_index: u16,

    pub duration_ms: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimScriptCondition {
    pub index: u8,
    pub bitflags: bool,
    pub bits: u64,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimScriptItem {
    pub skip: bool,
    pub conditions: Vec<AnimScriptCondition>,
    pub commands: Vec<AnimScriptCommand>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimConditions {
    pub written: u32,
    pub value: [u64; 18],
}

impl AnimConditions {
    pub fn set_value(&mut self, cond: u8, v: u32) {
        if (cond as usize) >= 18 {
            return;
        }
        self.written |= 1 << cond;
        self.value[cond as usize] = u64::from(v);
    }

    pub fn set_bit(&mut self, cond: u8, bit: u8) {
        if (cond as usize) >= 18 || bit >= 64 {
            return;
        }
        self.written |= 1 << cond;
        self.value[cond as usize] |= 1u64 << bit;
    }
}

pub fn anim_conditions_from_pmove(
    ps: &PlayerState,
    view_facts: Option<WeaponCombatFacts>,
    primary_facts: Option<WeaponCombatFacts>,
    movetype: Option<u8>,
    strafing: u8,
    buttons: u32,
) -> AnimConditions {
    let mut conds = AnimConditions::default();
    conds.set_value(ANIM_COND_MOUNTED, u32::from(ps.e_flags & 0xc00 != 0));
    conds.set_value(
        ANIM_COND_FIRING,
        u32::from(buttons & playerstate_iw4::buttons::ATTACK != 0),
    );
    conds.set_value(
        ANIM_COND_FASTMANTLE,
        u32::from(ps.mantle_flags & playerstate_iw4::mantle_flags::FAST_MANTLE != 0),
    );
    if let Some(facts) = view_facts {
        conds.set_bit(ANIM_COND_PLAYERANIMTYPE, bit_index(facts.player_anim_type));
        conds.set_bit(ANIM_COND_WEAPONCLASS, bit_index(facts.weap_class));
    }
    if let Some(facts) = primary_facts {
        conds.set_bit(
            ANIM_COND_PLAYERANIMTYPEPRIMARY,
            bit_index(facts.player_anim_type),
        );
    }
    conds.set_value(ANIM_COND_AKIMBO, u32::from(ps.last_weapon_hand == 1));
    if let Some(movetype) = movetype {
        conds.set_bit(ANIM_COND_MOVETYPE, movetype);
    }
    conds.set_value(ANIM_COND_STRAFING, u32::from(strafing));
    conds.set_value(ANIM_COND_PERK, 0);
    conds.set_value(ANIM_COND_RIOTSHIELDNEXT, 0);
    conds.set_value(
        ANIM_COND_WEAPON_POSITION,
        anim_weapon_position_from_pm_flags(ps.pm_flags),
    );
    conds
}

pub(crate) fn anim_strafing(ps: &PlayerState, forward: i8, right: i8) -> u8 {
    let forward = f32::from(forward);
    let right = f32::from(right);
    let command_length = (forward * forward + right * right).sqrt();
    let speed_sq = ps.velocity[0] * ps.velocity[0] + ps.velocity[1] * ps.velocity[1];
    if command_length == 0.0 || speed_sq <= 100.0 || forward.abs() > command_length * 0.5 {
        0
    } else if right > 0.0 {
        2
    } else {
        1
    }
}

pub(crate) fn event_anim_movetype(ps: &PlayerState, previous: Option<u8>) -> u8 {
    let last_stand = ps.pm_flags & playerstate_iw4::pm_flags::LAST_STAND != 0;
    previous
        .filter(|movetype| last_stand == matches!(*movetype, 51..=53))
        .unwrap_or_else(|| {
            movement_iw4::footsteps_anim_move_type(ps, 0, 0, true).unwrap_or(anim_iw4::ANIM_MT_IDLE)
        })
}

fn bit_index(v: i32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

pub fn pmove_anim_weapon_ids(ps: &PlayerState) -> (u32, u32) {
    (get_viewmodel_weapon_index(ps), ps.weapon)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerAnimScript {
    slots: Vec<(u8, u8, Vec<AnimScriptItem>)>,
    events: Vec<(u8, Vec<AnimScriptItem>)>,
}

impl PlayerAnimScript {
    pub fn from_tables(
        slots: Vec<(u8, u8, Vec<AnimScriptItem>)>,
        events: Vec<(u8, Vec<AnimScriptItem>)>,
    ) -> Arc<Self> {
        Arc::new(Self { slots, events })
    }

    pub fn event_item_count(&self) -> usize {
        self.events.iter().map(|(_, items)| items.len()).sum()
    }

    fn items(&self, state: u8, movetype: u8) -> &[AnimScriptItem] {
        self.slots
            .iter()
            .find(|(s, m, _)| *s == state && *m == movetype)
            .map(|(_, _, items)| items.as_slice())
            .unwrap_or(&[])
    }

    fn event_items(&self, event: u8) -> &[AnimScriptItem] {
        self.events
            .iter()
            .find(|(e, _)| *e == event)
            .map(|(_, items)| items.as_slice())
            .unwrap_or(&[])
    }

    pub fn apply(
        &self,
        ps: &mut PlayerState,
        movetype: u8,
        client_num: u32,
        conditions: &AnimConditions,
        force: bool,
    ) -> Option<u8> {
        if ps.pm_type >= 8 {
            return None;
        }
        let selected = self
            .matching_item(self.items(0, movetype), conditions)
            .map(|item| (movetype, item))
            .or_else(|| {
                (movetype != 1)
                    .then(|| self.matching_item(self.items(0, 1), conditions))
                    .flatten()
                    .map(|item| (1, item))
            });
        let (movetype, item) = selected?;
        if item.commands.is_empty() {
            return None;
        }
        let cmd = item.commands[client_num as usize % item.commands.len()];
        execute_command(ps, cmd, false, true, force);
        Some(movetype)
    }

    pub fn apply_event(
        &self,
        ps: &mut PlayerState,
        event: u8,
        conditions: &AnimConditions,
        seed: &mut u32,
        force: bool,
    ) -> bool {
        if event != ANIM_ET_DEATH && ps.pm_type >= 8 {
            return false;
        }
        let Some(item) = self.matching_item(self.event_items(event), conditions) else {
            return false;
        };
        if item.commands.is_empty() {
            return false;
        }
        let pick = (random(seed) as usize) % item.commands.len();
        execute_command(ps, item.commands[pick], true, false, force);
        true
    }

    fn matching_item<'a>(
        &self,
        items: &'a [AnimScriptItem],
        conditions: &AnimConditions,
    ) -> Option<&'a AnimScriptItem> {
        items
            .iter()
            .find(|item| !item.skip && item_matches(item, conditions))
    }
}

fn item_matches(item: &AnimScriptItem, conditions: &AnimConditions) -> bool {
    for cond in &item.conditions {
        let idx = cond.index as usize;
        if idx >= 18 {
            return false;
        }
        if cond.bitflags {
            if (cond.bits & conditions.value[idx]) == 0 {
                return false;
            }
        } else {
            if conditions.written & (1 << cond.index) == 0 {
                return false;
            }
            if conditions.value[idx] as i32 != cond.value {
                return false;
            }
        }
    }
    true
}

fn execute_command(
    ps: &mut PlayerState,
    cmd: AnimScriptCommand,
    set_timer: bool,
    is_continue: bool,
    force: bool,
) {
    let duration = cmd.duration_ms.saturating_add(50);
    if cmd.body_part == 1 || cmd.body_part == 3 {
        play_anim(
            ps,
            cmd.anim_index,
            true,
            duration,
            set_timer,
            is_continue,
            force,
        );
    }
    if cmd.body_part == 2 {
        play_anim(
            ps,
            cmd.anim_index,
            false,
            duration,
            set_timer,
            is_continue,
            force,
        );
    } else if cmd.body_part == 3 {
        play_anim(ps, 0, false, duration, set_timer, is_continue, force);
    }
}

fn play_anim(
    ps: &mut PlayerState,
    anim_num: u16,
    legs: bool,
    duration: i32,
    set_timer: bool,
    is_continue: bool,
    force: bool,
) {
    let (timer, raw) = if legs {
        (ps.legs_timer, ps.legs_anim as u16)
    } else {
        (ps.torso_timer, ps.torso_anim as u16)
    };
    if timer >= 50 && !force {
        return;
    }
    let same = (raw & PLAYER_ANIM_INDEX_MASK) == (anim_num & PLAYER_ANIM_INDEX_MASK);
    if is_continue && same {
        if set_timer {
            if legs {
                ps.legs_timer = duration;
            } else {
                ps.torso_timer = duration;
            }
        }
        return;
    }
    let written = PlayerAnimValue::restarting(anim_num, raw).raw() as i32;
    if legs {
        ps.legs_anim = written;
        if set_timer {
            ps.legs_timer = duration;
        }
    } else {
        ps.torso_anim = written;
        if set_timer {
            ps.torso_timer = duration;
        }
    }
}

pub(crate) fn reset_stance_torso(ps: &mut PlayerState) {
    play_anim(ps, 0, false, 0, false, true, true);
    ps.torso_timer = 0;
}

/// A mode's own table from player state to the anims a player's third-person
/// body plays, posed by an anim blend instead of the playeranim script.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerBodyAnims {
    rows: Vec<PlayerBodyAnimRow>,
    blend_time: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct PlayerBodyAnimRow {
    state: Arc<str>,
    stance: Arc<str>,
    direction: Arc<str>,
    anims: Vec<Arc<str>>,
}

/// What a player is doing, as `PlayerBodyAnims` picks by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlayerBodyPose {
    states: &'static [&'static str],
    stance: &'static str,
    direction: &'static str,
}

impl PlayerBodyAnims {
    /// `rows`: state, stance (empty for any), direction (empty for any) and
    /// the anims, one drawn per play.
    pub fn new(rows: Vec<(String, String, String, Vec<String>)>, blend_ms: u32) -> Arc<Self> {
        Arc::new(Self {
            rows: rows
                .into_iter()
                .map(|(state, stance, direction, anims)| PlayerBodyAnimRow {
                    state: state.into(),
                    stance: stance.into(),
                    direction: direction.into(),
                    anims: anims.into_iter().map(Arc::from).collect(),
                })
                .collect(),
            blend_time: blend_ms as f32 / 1000.0,
        })
    }

    pub(crate) fn blend_time(&self) -> f32 {
        self.blend_time
    }

    /// The row for `pose`: its states in order, each by stance and direction,
    /// then any stance, then standing; a standing idle when none has anims.
    pub(crate) fn select(&self, pose: PlayerBodyPose) -> Option<usize> {
        let find = |state: &str, stance: &str, direction: &str| {
            self.rows.iter().position(|row| {
                &*row.state == state
                    && &*row.stance == stance
                    && &*row.direction == direction
                    && !row.anims.is_empty()
            })
        };
        let lookups = |state: &'static str| {
            [
                (state, pose.stance, pose.direction),
                (state, pose.stance, ""),
                (state, "", pose.direction),
                (state, "", ""),
            ]
        };
        pose.states
            .iter()
            .flat_map(|state| lookups(state))
            .chain([("idle", "stand", ""), ("idle", "", "")])
            .find_map(|(state, stance, direction)| find(state, stance, direction))
    }

    pub(crate) fn anim(&self, row: usize, draw: usize) -> Option<&Arc<str>> {
        let anims = &self.rows.get(row)?.anims;
        anims.get(draw % anims.len().max(1))
    }
}

/// The pose of a live player from its stance, its anim move type and strafe,
/// and whether it holds fire or reloads.
pub(crate) fn player_body_pose(
    ps: &PlayerState,
    movetype: u8,
    strafing: u8,
    buttons: u32,
) -> PlayerBodyPose {
    let stance = player_body_stance(ps);
    let name = anim_iw4::ANIM_MT_NAMES
        .get(usize::from(movetype))
        .copied()
        .unwrap_or("");
    let moving: &'static [&'static str] = if name == "SPRINT" {
        &["sprint", "run", "walk", "idle"]
    } else if name.starts_with("RUN") {
        &["run", "walk", "idle"]
    } else if name.starts_with("WALK") {
        &["walk", "idle"]
    } else {
        &["idle"]
    };
    let direction = match (moving[0], strafing) {
        ("idle", _) => "",
        (_, 1) => "l",
        (_, 2) => "r",
        _ if name.ends_with("BK") => "b",
        _ => "f",
    };
    let reloading = weapon_iw4::WeaponState::from_i32(ps.weaponstate_primary)
        .is_ok_and(weapon_iw4::WeaponState::is_reload_family);
    let firing = buttons & playerstate_iw4::buttons::ATTACK != 0;
    // One leaf poses the whole body, so a moving player keeps its legs'
    // anim while it fires or reloads.
    let states: &'static [&'static str] = match moving[0] {
        "idle" if reloading => &["reload", "idle"],
        "idle" if firing => &["fire", "idle"],
        _ => moving,
    };
    PlayerBodyPose {
        states,
        stance,
        direction,
    }
}

pub(crate) fn player_body_death_pose(ps: &PlayerState) -> PlayerBodyPose {
    PlayerBodyPose {
        states: &["death"],
        stance: player_body_stance(ps),
        direction: "",
    }
}

fn player_body_stance(ps: &PlayerState) -> &'static str {
    if ps.pm_flags & playerstate_iw4::pm_flags::PRONE != 0 {
        "prone"
    } else if ps.pm_flags & playerstate_iw4::pm_flags::CROUCH != 0 {
        "crouch"
    } else {
        "stand"
    }
}
