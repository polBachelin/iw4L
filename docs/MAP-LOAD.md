# Map load: what lives where

Three floors: **disk** (`asset_transport`), **format** (`fastfile_*` → the
`asset_iw4` IR), **products** (`asset_*` + `assets`). `assets` uses transport for disk access and its source lanes to compile
format captures into products; `fastfile_*` knows the format and opens no files. Installing the match is `session`, not here.

```text
console/ui `map` → session::lifecycle   SessionSwapRequest, tear the previous
                     occupancy down, then MapLoadApproved{request_id}
  → assets::match_load    start_match_load onto load_pool() (MatchLoadBusy),
                          poll_match_load → PreparedMatchReady
  → session::match_apply  apply_prepared_match → scene + SimWorld + MatchInstalled
```

Cancellation is `MatchLoadAbort(request_id)`: the walk returns
`MatchLoadOutcome::Canceled` at its next checkpoint, throwing nothing away.
Failure is `MapLoadFailed`, from discovery *and* install; a request ends once.
Startup and later load failures return to the main menu with a map error dialog;
OK or Escape dismisses it. The log keeps the full error.

## The walk: `assets::session_load::load_prepared_match`

The single entry point; the composition root sees no `fastfile_*` and no
`AssetSink`. Inside are parallel walks on `load_pool()`: the archive, startup
materials, `common_mp`, the IW5/T5/T6 weapon bundles, language zones. `common_mp`
goes **before** the map — its techset tables are needed before
`build_world_draw`. Output is `MatchLoadOutcome::Ready` with a `PreparedMatch`
(world, one material population, clip, weapons, catalogs and `PreparedMap`).

A walk enqueues its own image plan the moment it is ready, and the catalog keeps
one row per image name. Two plans resolving the *same* archive entry into the
same payload share one decode; a name three games spell alike but fill
differently is an override, not a duplicate, and `load_jobs.csv`
([`BENCH.md`](BENCH.md)) counts the two apart. The per-game adapter is
`assets::lane` (`ZoneGame` → iw4/iw5/t5/t6); a lane gap is a typed `LaneGap`, never
silence.

An IW5 zone's `PathData` is captured through `AssetLinkSink::capture_path_data`
into `asset_world::PathGraph`: one `PathNode` per zone node (type, origin, yaw
as angles, targetname and script_noteworthy when set) and a flat `links` list
(neighbor index, distance) that each node slices with `first_link`/`link_count`.
Node indices are the zone's, so link targets index `nodes` directly; a link
pointing past the node array is dropped and counted. It travels as
`MapFacts::path_graph` into `PreparedMap`, empty when the zone ships no
`PathData`. Capture only: nothing paths on it yet. An `AddonMapEnts` entity
string is appended after the zone's `MapEnts` string, and every IW5
entity-string reader (spawns, script structs, triggers, the map script
entities) parses the merged text.

`ZoneLane::load_common_mp` returns owned common builders and diagnostics. A lane whose preparation needs already assembled donors can retain a private, consumed-once `CommonFamilyCompiler` inside that result. The shared loader supplies owned weapon, material, FPV, world-model, projectile, animation and effect builders; compilation returns them together with dependency refusals. This stage finishes before the common set is published or cloned for a match. It is not invoked by frame execution.

T6 model/material/effect captures stay private to the T6 lane. Its compiler owns stand-in donor lookup, native technique linking, named texture adaptations, animation admission, weapon dressing and projectile binding. Missing model/effect donors return typed refusals, while accepted contributions and existing builders remain available; source conversion diagnostics remain in the load report. Other lanes finish these products within their existing common walk. All lanes publish through the same common-set and match installation path.

`ZoneLane::load_world` borrows the common `FilmVisionCatalog` immutably. The map's vision overrides a common vision, including a map parse error; a failed map parse does not silently fall back. Selection and the merged vision catalog belong to `LoadedWorld` output. Loading a map does not consume a caller's common vision entries.

These are CPU compilation contracts. They do not establish shader-port admission, decoded-media readiness, GPU residency or playable readiness. The existing common cache owns compiled builders under its path-based `CommonKey`; a retained key does not detect edits to source files in place. Such content needs a rebuilt common set. No per-family decoder payload escapes the deferred compiler, and changing source content must not reuse its captured payload with unrelated donor builders.

Installing it is `session::match_apply`. Preflight builds a `MatchInstallPlan` — mode, doors, objectives, scene conversion, drawable world —
and publishes nothing until it hands one over. Commit publishes it, boots the
sim and writes `MatchInstalled`. The authority then prepares bot navigation on
`load_pool()` from a world snapshot; it reports `BotNavigationReady` with the
world generation. Session owns readiness policy: dedicated advancement needs
navigation; listen advancement also needs rendering unless headless. Graphical
presentation and admission require generation-matched rendering and audio;
headless policy bypasses those presentation services.
Explicit silent audio satisfies that policy; missing or failed audio does not.
Old completion reports cannot release a newer world, and renderer systems do
not write the authority hold. Session also owns the local input predicate.
The walk graph is cached (`nav`), keyed by the content digest with the bake's
schema and hull, so the second start of a map reads it back instead of walking
the grid again; a match teardown drops the in-memory copy, not the file.

`MatchLoadOutcome::Ready` is the CPU package, not render-ready. The loading
screen also holds for GPU images, every queued pipeline (the HUD blood film is
queued as soon as its material exists) and the `first_person` stage: every
weapon's first-person materials admitted and compositions laid out
(`render_anim::PreparedFpv`), and every body, world-weapon, map-model and
projectile material plus the single-model DObjs (`PreparedModelMaterials`).
Remote kits and dropped-item compositions are built at install.
`load ledger:` in the log is one line per load of
what was handed over or compiled new against what an earlier load left behind.

## Cache, and poking it: `iw4l-artifacts/cache/<kind>/<prefix>/<key>`

Content-addressed leaf in `asset_transport::artifact_cache` (`cache_get` /
`cache_put`, `fnv1a64`). A miss is silent — the caller computes the value anyway
— and a hit must be the **same bytes** a miss would have written. The key names
every input; if the encoder changed, bump the format word. Live kinds: `mips`,
`wgsl`, `localize`, `nav` and `xwma_pcm`, whose miss is decoded by the native Rust T5 WMA2 decoder.
The launch's `RuntimeProfile` picks the tree whose first base zone anchors the
walk (IW4, or the map's own IW5 tree for `survival`); the startup zones come from
beside it. Every zone loaded beside it (sound bank base rows, localized strings,
the menu probe) goes through `RuntimeProfile::find_runtime_zone`, which refuses a
file of another game's envelope; the profile's own game is never also walked as a
donor. A launch mode names its base zones, loaded right after the map
(`ModeZones::base`; `survival` reads them from its `base_zones` settings row,
every other mode keeps `common_mp`). They are found through
`RuntimeProfile::find_base_zones` and are part of the `CommonKey`; the first one
anchors the census and the rest are laid over it in their order. The sound bank
rows, the menu catalog, the localized strings and the resident clip cache take
their base zones from the same set, with each base zone's `localized_` sibling
where the tree has one (a missing sibling is skipped without a gap); the menu
catalog and the menu strings read from the tree of the first base zone. A launch mode
adds its own zones (`survival` names MW3's `common_specialops`, `common_survival`,
`patch_specialops`, `patch_survival` in its `mode_zones` settings row). They are
found the same way, are part of the `CommonKey`, and are walked by the common
census after the base zones in that order, a later zone's asset replacing an
earlier one's: weapons in their row, catalogs by name, scripts and
tables, menus, localized strings and sound aliases. Their census claims their
sounds (`ZoneSoundCapture::claim_source`); their images decode with the merged
pool. A material another zone holds is captured as a reference row that links to
the real one by name.
`IW4L_GAMES` holds the game trees; no folder name is hardcoded. Live it is `make
map mp_boneyard` ([`RUN.md`](RUN.md)), with stages in `LoadProgress`.
