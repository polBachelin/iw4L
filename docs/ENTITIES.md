# World entities and the funnel

## There is one funnel

```text
human ─┐
bot    ├─→ TickInput ─→ sim::step ─→ Snapshot ─→ client / theater / demo
replay ┘

pub fn step(world: &mut SimWorld, tick: Tick, input: &TickInput,
            msec: i32, reason: StepReason) -> Snapshot;
```

Headless, prediction and replay call exactly this one.
`msec` is the step length; time enters **only** as an argument. The shape of
the funnel is [`SIM-STEP.md`](SIM-STEP.md), the code `crates/sim/src/lib.rs`.

## What can be an entity at all — fixed by the data (`entity_iw4`)

The wiring is ours, the taxonomy comes with the data. The crate neither
serializes nor orders fields "for the network": it pins the layout it reads,
and leaves transport to `net`.

| | |
|---|---|
| `EntityState` | replicated snapshot slot, stride `0x100` |
| `Centity` | client-side slot, stride `0x204` (pose plus snapshot anchors) |
| `ClientState` | per-client slot, stride `0x7c` (command, `name[16]`) |
| `CorpseInfo` | server-side corpse slot, stride `0x53c`, 8 of them |
| `ET_PLAYER` / `ET_PLAYER_CORPSE` / `ET_ITEM` / `ET_MISSILE` | |
| `Trajectory` | `evaluate_trajectory`: gravity 400, delta 800 |
| events | the entity-event ring, an 11-bit wrapper |
| `ENTITYNUM_NONE` | `0x7FF` |
| `playerstate_iw4` | player and input: the two ends of the funnel |

## Where everything else lives

* `sim/` — `gentity spawn world snapshot damage bullet* missile item corpse
  match_state score rules world_objects identities` (`ScriptModelId` is the one
  identity of a script model; a raw `u32` is not a second one);
* `frame` — `AppScreen`, `HasWorld`, `RuntimeRole`, `LaunchIdentity`; no
  plugins, no systems;
* `session` — standing the match up and tearing it down (`MatchInstalled` /
  `MatchTornDown`); `net` — wire, deltas, prediction; `bots` — host-only
  controllers that observe through a sensor adapter, walk a ClipMap-baked graph
  and enter the same `TickInput`.

`net/transport/meta_wire.rs` encodes and parses metadata messages. The pure
`world_object_wire.rs` codec validates complete world-object updates;
`world_sync.rs` owns mutable baselines and periodic full synchronization.
Frame decoding commits its prepared world update after reliable and service
sections succeed, so a refused frame leaves the world baseline intact.

Snapshots publish the **semantics** of DObj composition, never runtime trees or
posed vertices ([`ANIM.md`](ANIM.md)).

## Actors

An actor is a character the server simulates without a client: it takes no
client slot, no bot slot and no scoreboard row. `spawnactor(origin, angles,
team)` (`sim/src/script/host/natives/actor.rs`) makes a script entity of kind
`EntityKind::Actor` on a script mover gentity, with `team`, `health` and
`maxhealth` (100) fields; `delete()` removes it like any spawned entity. The
methods are in `script/profile/actor_catalog.rs` under `Owner::Actor`:

* `setgoalpos(point)`, `setgoalnode(node)`: accepted, no movement yet;
* `setweapon(name)`: the weapon `shoot` fires (`"none"` clears it);
* `shoot([point])`: one bullet from `tag_flash`, or standing eye height, at
  `point` or along the actor's angles;
* `isalive()`, `getteam()`.

Behavior lives in GSC. `setcharacter(body, [head], [hat])` dresses an actor:
the body becomes its `model`, the head and hat attach to the body's head tag,
and that DObj is what presence poses with the entity's anim tree (`useanimtree`
and the other anim-tree methods, see [`GSC-RUNTIME.md`](GSC-RUNTIME.md)) and
`render_anim` draws as a spawned script model. An actor with no character has
no model: nothing draws it and nothing hits it.

Bullets hit an actor's per-bone hit volumes, posed by its anim tree
(`AuthorityDObjState::hit_volumes`); a model without them falls back to its
bounds. Radius damage reaches it like any damageable script entity. A hit is
scaled by hit location as a player's is, and a head hit is `MOD_HEAD_SHOT`. The
actor is notified `damage` (amount, attacker, direction, point, means, model,
bone, hit location, flags, weapon) and, when its health reaches zero, `death`
(attacker, means, weapon); kill credit is the script's, from those. A dead
actor takes no more damage and stays until deleted.
