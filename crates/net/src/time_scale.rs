use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use frame::RuntimeRole;

use crate::authority::runtime::{AuthorityLoadHold, authority_live};
use crate::client::runtime::LastAdoptedSnapshot;
use crate::{AuthorityClock, AuthorityWorld, FrameClock, GameActive, UdpAuthorityHub};

/// The menu on top of the local player's screen that asks for the game to
/// stop, written by the HUD every frame (`None` when there is none). It is a
/// request: whether the game stops is the host's call, in `apply_time_scale`.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct HostPauseRequest(pub Option<String>);

/// The pause in force: the authority tick it froze on and the wall clock
/// (seconds of `Time<Real>`) when it began.
#[derive(Resource, Debug, Default)]
pub(crate) struct HostPause(Option<(u32, f64)>);

#[derive(SystemParam)]
pub(crate) struct TimeScaleInputs<'w> {
    authority: Option<Res<'w, AuthorityWorld>>,
    authority_clock: Option<Res<'w, AuthorityClock>>,
    adopted: Option<Res<'w, LastAdoptedSnapshot>>,
    clock: Option<Res<'w, FrameClock>>,
    active: Option<Res<'w, GameActive>>,
}

#[derive(SystemParam)]
pub(crate) struct HostPauseInputs<'w, 's> {
    request: Res<'w, HostPauseRequest>,
    state: ResMut<'w, HostPause>,
    hold: Option<Res<'w, AuthorityLoadHold>>,
    hub: Option<Res<'w, UdpAuthorityHub>>,
    real: Res<'w, Time<Real>>,
    refused: Local<'s, bool>,
}

/// The one writer of the game clock (`Time<Virtual>`): the slow-motion plan
/// sets its rate, and a host pause stops it. Stopping the virtual clock stops
/// the fixed schedule (authority ticks, script waits, bots) and the listen
/// client's frame clock together, so nothing runs ahead of anything else and
/// nothing catches up on resume.
pub(crate) fn apply_time_scale(
    role: Res<RuntimeRole>,
    inputs: TimeScaleInputs,
    pause: HostPauseInputs,
    fixed: Res<Time<Fixed>>,
    mut time: ResMut<Time<Virtual>>,
) {
    let TimeScaleInputs {
        authority,
        authority_clock,
        adopted,
        clock,
        active,
    } = inputs;
    apply_host_pause(
        *role,
        authority.as_deref(),
        authority_clock.as_deref(),
        pause,
        &mut time,
    );
    let (plan, game_ms) = if role.runs_authority() {
        (
            authority.as_ref().and_then(|world| world.0.slow_motion()),
            authority_clock
                .as_ref()
                .map_or(0.0, |clock| f64::from(clock.time_ms))
                + fixed.overstep().as_secs_f64() * 1000.0,
        )
    } else {
        let snapshot = adopted
            .as_ref()
            .and_then(|a| a.next().or(a.snap.as_deref()))
            .filter(|_| active.as_ref().is_none_or(|active| active.0));
        let game_ms = clock.as_ref().filter(|clock| clock.started()).map_or_else(
            || snapshot.map_or(0.0, |s| f64::from(s.tick.0) * f64::from(sim::MATCH_TICK_MS)),
            |clock| f64::from(clock.time()),
        );
        (
            snapshot.and_then(|s| s.meta.objectives.slow_motion),
            game_ms,
        )
    };
    time.set_relative_speed(
        plan.filter(|p| p.valid())
            .map_or(1.0, |p| p.sample(game_ms)),
    );
}

/// A pause request stops the game only on a listen host no remote client is
/// connected to (bots are the host's own), while its authority runs a loaded
/// match. Anything else leaves the game running, and a remote client that
/// connects during a pause ends it.
fn apply_host_pause(
    role: RuntimeRole,
    authority: Option<&AuthorityWorld>,
    clock: Option<&AuthorityClock>,
    inputs: HostPauseInputs,
    time: &mut Time<Virtual>,
) {
    let HostPauseInputs {
        request,
        mut state,
        hold,
        hub,
        real,
        mut refused,
    } = inputs;
    let remote = hub.as_ref().is_some_and(|hub| hub.has_peers());
    let solo_host = role == RuntimeRole::Listen && !remote;
    let live = authority_live(role, authority, hold.as_deref().copied());
    let menu = request.0.as_deref();
    let tick = clock.map_or(0, |clock| clock.tick);
    match (menu.filter(|_| solo_host && live), state.0) {
        (Some(menu), None) => {
            state.0 = Some((tick, real.elapsed_secs_f64()));
            time.pause();
            diag::info!(Net, "host pause: paused tick={tick} menu={menu}");
        }
        (None, Some((paused_tick, since))) => {
            state.0 = None;
            time.unpause();
            diag::info!(
                Net,
                "host pause: resumed tick={tick} paused_tick={paused_tick} after={:.1}s",
                real.elapsed_secs_f64() - since
            );
        }
        _ => {}
    }
    let refusal = menu.filter(|_| live && !solo_host);
    if let Some(menu) = refusal
        && !*refused
    {
        diag::info!(
            Net,
            "host pause: `{menu}` open, game keeps running (role={role:?} remote_clients={remote})"
        );
    }
    *refused = refusal.is_some();
}
