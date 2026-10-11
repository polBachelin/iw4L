# Animation: what lives where

Three floors, deliberately different: the **curve facts** (pure mathematics),
the **runtime tree** (pose state) and **who picked which clip** (the game).

## `anim_iw4` — facts only, `#![no_std]`, no clock and no allocator

* `XAnimParts` — curve storage, unpacking of `half_quat` / `full_quat` and
  quantized translations (`XAnimCalcParts`); translations are **deltas**;
* span/sample/`time_to_frame` — a pure function of (clip, time);
* the blend rule `bind + scale * Δ`, rotation replaced (`DObjCompose`);
* `DObj` — how model, skeleton and attachments compose (0x9c, prefix mapped out);
* `XBoneInfo` — the OBBs for bone hits (`DObjGeom_RayBones`);
* `PartBits` — the six-word requested/ignore mask (`DObjCalcAnimInternal`);
* `PlayerAnimValue` — 10-bit `legsAnim` / `torsoAnim`, a 9-bit index and the
  `0x200` "restart" bit: a restart changes **only** that bit, not the index;
* the leaf time step and the goal-weight ramp are measured IW4 constants;
* script name tables: `ANIM_MT_NAMES`, `ANIM_BODY_PART_NAMES`, `ANIM_COND_NAMES`.

No `Vec`, no `AnimationClip`, no glam matrices and no playback clock here.

## `xmodel_runtime` — a runtime without Bevy and without ECS

A pure evaluator, used by **both** the authority **and** the client:

* `XAnimTreeDefinition` (the immutable hierarchy) + `XAnimTreeRuntime` (state;
  leaf time normalized into `[0,1]`, `update` is the per-leaf advance);
* `DObjAnimRuntime` — `update` / `pose_request` / `compose`; DObj reuse is
  equality of `(eType, model*)`, not "always a fresh one";
* `RetainedModelCapability` — pose, `XBoneInfo`, collSurf/collTris for hitscan;
* `build_body_head_weapon_dobj` — body + head on `j_spine4` + weapon on its tag.

Character models captured into `BodyMeshCatalog` (with bone names, parents and
bind pose), by name family: `mp_body_*`, `*_mp_body_*`, `*_mp_head_*`
(MP), `body_*`, `fullbody_*`, `head_*`, `hat_*` (SP), plus the dog model
`german_sheperd_dog`. `by_name` reaches any of them; `kits()` still builds
team kits from the MP bodies only. `torso_stabilizer` is optional: SP bodies
parent `j_spinelower` and the hips straight to `j_mainroot`.

A missing clip **never** turns into the bind pose; additive nodes are a typed
gap, not "ordinary blending".

## Who picks the clip

* `sim` — `player_anim_script.rs`, `mantle_xanim.rs`, `anim_script_gap.rs`:
  what animation the player is in, and it travels into the snapshot as
  semantics, not as a tree;
* `assets` — `XAnimParts` → owning clips;
* `render_frontend/adapters/anim/` + `render_anim` — the viewmodel (`fpv*.rs`,
  `viewmodel_controller.rs`), third person (`third_person.rs`, `remote_body.rs`),
  kick and sway (`view_kick.rs`, `view_sway.rs`), items and projectiles.
  FPV actions come from the presented `weapAnim`, **never** from the
  keyboard; the identity of the FPV mesh is
  `weapon_iw4::get_viewmodel_weapon_index`.
  The FPV consumer reads `PreparedFpv` only, never the weapon registry: views,
  rigs and material bindings are built before Ready; an equip picks one, and a
  mandatory material that failed is a refusal naming model, surface and
  material. Remote kits (`PreparedRemoteKits`: models, hide bits, shared DObj)
  and dropped items are composed at install, not on first sight.

Script entities with an anim tree (`sim::script::host::script_anim`) are the
exception to "semantics, not a tree": the blend state is authoritative on the script
entity and reaches the DObj as an `XAnimTreeSnapshot` (one blend root, one leaf per
animation, see [`GSC-RUNTIME.md`](GSC-RUNTIME.md)). `render_anim` resolves it like any
other tree: each leaf's tracks bind to the model's bones by name, tracks for bones the
model lacks are skipped, and the leaves blend by weight. A leaf whose clip is not
loaded on the client refuses the pose instead of falling back to the bind pose.

To watch it live: `IW4L_PERF=1` / [`PERF.md`](PERF.md).
