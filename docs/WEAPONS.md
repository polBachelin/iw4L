# Weapon configuration products

`asset_game::WeaponCatalog` captures source rows. `WeaponBuild` links dependencies and publishes immutable effective rows, semantic policies and consumer projections.
`weapon_catalog/{capture_merge,catalog_linking,model_linking,publication}` own
those operations; `configuration/{iw4,iw5,t5,t6}` own source selection rules.

`WeaponFamilies` normalizes selections and checks attachments and host limits.
Resolution and UI toggles return a private registry-issued `WeaponHandle` plus
read-only canonical selection. Console/UI use this resolver. Publication iteration
includes the last valid row; unarmed, unknown and unsupported remain distinct.

`WeaponRegistry::bind` rejects foreign revisions before row access. Exact clones
retain identity; republishing changes it. Raw projection getters are crate-private;
consumers use `BoundWeapon`. `PreparedWeapons` admits wire IDs only after checking
the installed snapshot epoch or original event generation. Local revisions never
enter wire IDs/content digests. Queued effects retain their producing generation.

Combat, equipment, penetration, FPV, HUD, world and event projections use one
effective row. Combat binds host rules and location damage before simulation.
The host burst cooldown remains 200 ms across sources. Presentation facts carry
camera, alternate, dual, shield, overlay and event policies without reinterpreting
capture classifications. Loadout labels/archive hints are prepared by asset_game.

A row's identity is its asset name less `_mp`, so within one zone an asset
and its `_mp` spelling merge (first capture wins). Its script name, what GSC
and the sim call it by, is the captured asset's own spelling: `iw5_ak47_mp`,
but `sentry_minigun_so` or `remote_missile_detonator`. A zone loaded later
replaces the whole row of a weapon it carries under the same script name; a
weapon it carries under another spelling of the same identity gets a row of
its own, so SP `claymore` or `flash_grenade` sits beside MP `claymore_mp` or
`flash_grenade_mp` and both resolve. Lookups by name try the exact script
name first (`resolve_script_name` does nothing else), then the identity. A row with no captured asset (a
configuration) falls back to the `_mp` spelling of its bare name. Every IW5
zone walked logs the weapon names it captured at debug level
(`IW5 weapons captured:`).

`BoundWeapon::preparation` retains source keys and per-component targets/refusals.
T6 models, clips, cues and materials retain T6 source and storage identity.
Missing native dependencies refuse their dependent capabilities; source weapon
rows never inherit another family's row. Publication retains captured family.

The map selects the soldier kit and hands family. Hands a mode names for its
players (Survival's `player_viewhands`) come before the kit's when the meshes
hold them. Weapon-authored hands are eligible only for that family. FPV composition validates typed native model
pairs; T6 guns additionally admit an explicit `T6WithIw4Hands` connection for
IW4 soldiers. Catalog owners must match. Unsupported family pairs refuse.

`FpvWeaponTable::bind` checks registry, mesh, clip, material, image and atlas owners
before accepting handles and alternate links. Retained compositions validate model
and hide identity; track mappings/rigs retain actual clips and meshes. Foreign clips
or rig poses refuse before writes. Optional bones/clips and preparation budget remain.
Material admission retains mesh/material owners; raw assembly construction is internal.
Items and remote kits share `ItemComposition` topology/hide and registry/world owners. Required model/pose failures are cached;
optional attachments and camo-to-base fallback preserve policy. Replacement clears
successes/refusals. Kits retain body/world catalogs; `models()` accepts no catalogs.
Prepared DObj identity controls reuse; heads use checked soldier capabilities.
`PlayerAnimationBinding` retains the character kit/rig, paired tree/script and clips.
Multiplayer body tracks validate leaves/tracks and soldier tree/script/clip family.
Missing native profiles refuse; persistent trees reuse only the same binding.
Body/tree/script/clip replacement resets animation and preserves corpse occupation; unchanged bindings preserve blend, phase and rate.

Publication compiles appearance plans once; `SelectedWeaponAppearance` binds models, overrides and UI metadata.
Raw hand/camouflage data stays on `WeaponBuild`; runtime consumers select appearances.
Native previews are optional; known IW4 model choices compile their own preview keys.
