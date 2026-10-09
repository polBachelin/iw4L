# `bots` — host AI

Authority tick: observation → memory → utility → task → route → motor → UserCmd.

`bot dummy [N]` creates bots with no controller or perception queries. They keep
their aim and weapon, accept `bot tp/give/fire`, and take damage and respawn normally.
They retain normal bot loadouts/perks; `bot hold off` leaves dummies idle.
Bots submit `TickInput.cmds` like players — [`SIM-STEP.md`](SIM-STEP.md).

A bot's client id is its entity number, so the roster hands out ids 1 to
`MAX_CLIENTS - 1` round-robin and a full set of client slots mints nothing more.
A bot the authority drops (a script `kick`, a connection fault) leaves the
roster with it: `bots: retired client N`. Bots a match script asks for (the
survival director, [`SURVIVAL.md`](SURVIVAL.md)) join bound to an archetype and
pick no class on join or on any later life; the script arms and spawns them.

| file | owns |
|---|---|
| `observation.rs` / `memory.rs` | what a bot may know, and for how long |
| `sensor.rs` | visibility probes and the rotation that spends them |
| `controller.rs` / `task.rs` / `intent.rs` | scores, task choice, stages |
| `nav.rs` | the baked graph, A\*, resumable route requests |
| `query.rs` | the shared collision budget every subsystem draws on |
| `weapon.rs` / `motor.rs` | the weapon channel; aim and movement |
| `tuning.rs` | `BotTuning`: turn rates, fire cone, aim error, sight range and field of view |

## The three rules that shape the rest

**A negative needs a probe that ran.** Visibility answers seen, evaluated and
not seen, or not evaluated. Only the second ends a contact; the third holds it,
briefly and without coordinates. Firing needs a positive observation of that
client, fresh on the tick the command goes out.

**A denied query is not a collision.** Perception, connectors, execution and
combat share one 96-query quota. A query that ran reports what it found even on
the last token; one that never ran comes back as denial, so no refusal is ever
fabricated from an empty budget.

**Work survives a tick that is granted nothing.** A route request owns its
phase — approach, attachments, search, validation — and every cursor inside it
is resumable, so a zero allocation repeats nothing and A\* keeps its frontier
under the shared 2048-expansion quota. Answers that follow from graph topology
alone are cached per request key until the graph changes; collision-dependent
refusals are rechecked.

## Movement and combat

The graph bakes world-aligned tiles at 48 units, refined to 24 near missing
support or a height change, with directed Walk, Drop and BreakGlass edges that
keep their entry and exit through execution. A fight moves to a *position*: one
baked support per sector around the threat, checked for line of fire and
exposure, then reached through the ordinary route executor — reachability is
never inferred from a component label.

A riot shield in hand (`weapType` shield, `WeaponClass::Shield`) has no
magazine: the bot holds at three quarters of the player melee range, fires
only once the target is within it, and fires with the melee button, pressed
and released on alternate commands since a bash starts on the press. The
weapon skill never reloads or switches away from a shield.

## Tuning and the survival hunt

Each controller carries a `BotTuning`; its defaults are the host bots' own
values and every multiplayer bot keeps them. `BotTuning::scaled(skill)`
multiplies the turn rates and accelerations by `skill` and divides the fire
cone and the per-bot aim error by it; sight range and field of view stay as
they are. Survival scales its bots by their archetype's skill
([`SURVIVAL.md`](SURVIVAL.md)).

In a survival match (`GameModeKind::Survival`), a bot on the enemy team is
told where every live client of the human team stands (`BotObservation::quarry`,
`PublicMode` contacts). Its Hunt walks the graph to the nearest of them,
keeps that target while it lives unless another is 256 units nearer, and
re-takes the destination when the target has moved 128 units. A destination
with no route falls back once to the graph support nearest the target. Only
when nobody of the human team is alive does Hunt roam. Fighting still needs
sight: perception, memory and firing are unchanged. Every other bot, and every
other mode, roams as before.

`HostController::decision()` exposes scores, stage and objective; the periodic
aggregate adds per-subsystem query counts and stuck bots. Both read live from a
running match, which is where bot behaviour is judged: `make map mp_boneyard`
([`RUN.md`](RUN.md)) and watch the aggregate.
