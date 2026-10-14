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

* `setgoalpos(point)`, `setgoalnode(node)`: walk to the goal over the map's
  path nodes (see [Actor navigation](#actor-navigation));
* `setgoalradius(radius)`: sets the `goalradius` field;
* `setweapon(name)`: the weapon `shoot` fires (`"none"` clears it), held in
  the hands: its world model attaches to the body's weapon tag (the first of
  `xmodel_runtime::TP_WEAPON_ATTACH_TAGS` the body has), replacing the gun
  held before; a re-dress puts it back on the new body;
* `shoot([point])`: one bullet from `tag_flash`, or standing eye height, at
  `point` or along the actor's angles, after the fire event a client's shot
  pushes (`EV_FIRE_WEAPON` on the actor's entity number). It fires nothing,
  without error, while actors are held (console `actor hold on`) or while the
  actor's yaw is more than its `aimtolerance` field (degrees, any when unset)
  off the point;
* `dropweapon()`: the weapon leaves the hands as an MP weapon item
  (`item::drop_weapon_at`, no client owner) falling from the gun's tag, else
  the origin, with a fresh weapon's clip and stock; returns the item entity
  (`itemweaponsetammo` works on it), undefined with no weapon;
* `isalive()`, `getteam()`.

Each frame a live actor turns toward the way it walked, else toward its
`goalyaw` field, by at most its `turnrate` field in degrees a second (at once
when unset). The console's `actor list|tp|hold` reach actors through
`SimWorld::actor_rows`, `actor_tp` and `set_actor_hold`, since they are no
clients.

Behavior lives in GSC. `setcharacter(body, [head], [hat])` dresses an actor:
the body becomes its `model`, the head and hat attach to the body's head tag,
and that DObj is what presence poses with the entity's anim tree (`useanimtree`
and the other anim-tree methods, see [`GSC-RUNTIME.md`](GSC-RUNTIME.md)) and
`render_anim` draws as a spawned script model. An actor with no character has
no model: nothing draws it and nothing hits it.

Bullets hit an actor's per-bone hit volumes, posed by its anim tree
(`AuthorityDObjState::hit_volumes`); a model without them falls back to its
bounds. Those volumes, the bounds fallback included, are flesh
(`EntityCollisionTraceGeom::flesh`, keyed on the entity being an actor, so a
script model's bounds stay non-flesh): a bullet plays the impact table's flesh
cell, the head variant when the bone's hit location is a head one and the
fatal variant when the hit took the actor's health to zero. Damage is applied
before the impact event is built, as for a player. Radius damage reaches it like any damageable script entity. A hit is
scaled by hit location as a player's is, and a head hit is `MOD_HEAD_SHOT`. The
actor is notified `damage` (amount, attacker, direction, point, means, model,
bone, hit location, flags, weapon) and, when its health reaches zero, `death`
(attacker, means, weapon); kill credit is the script's, from those. A dead
actor takes no more damage and stays until deleted.

## Actor navigation

Actors walk on the path nodes the map ships (`MapFacts::path_graph`, handed to
the sim as `SimPathGraph` by `SimContentBuilder::set_path_graph`), never on the
bots' baked graph. `sim/src/script/host/natives/actor_path.rs` holds it all.

`getnode(name, key)`, `getnodearray(name, key)` and `getallnodes()` return node
structs with `origin`, `angles`, `type` and, when the node has them,
`targetname` and `script_noteworthy`; `key` is one of those two names. A node
is the same struct on every call, so scripts can compare nodes and keep fields
on them. `type` is the node type's name (`"Path"`, `"Cover Stand"`, ...), and
`"BAD NODE"` for a type the table does not know.

`setgoalpos(point)` and `setgoalnode(node)` replace the actor's goal. The route
starts at the nearest of the eight nodes closest to the actor that a clip trace
at 32 units up reaches unobstructed (the nearest when none does) and ends at the
same choice around the goal, or at `node` itself. A* over the node links runs
under one budget of 512 node expansions per frame shared by every actor, in
actor order; a search that runs out resumes the next frame where it stopped.
The found route is walked node to node and then to the goal point as far as
the actor's playing anims carry it: each leaf's root-motion speed
(`AnimClip::move_speed`) by its blend weight, scaled by the actor's
`moveplaybackrate` field (1 when unset). An actor playing no moving anim does
not move. Each frame the feet are snapped to the ground with a clip trace from
18 units above to 64 below, and the actor turns toward its motion at its
`turnrate`. A dead actor keeps its
goal but does not move.

The actor is notified `goal` once it is within `goalradius` (field, 0 when
unset) of the goal horizontally and within its height vertically, or at the end
of the route, and `bad_path` when there are no nodes or no route links its
start and goal nodes. Either clears the goal.
