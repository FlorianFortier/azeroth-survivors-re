# Breath of Sindragosa — first Legion effect integration

> Build data: WoW Legion 7.3.5.26972.  
> Scope: one Survivors ability and its existing SpellFx/M2 rendering path.  
> The proprietary game files remain local; only FileDataIDs and verified mapping metadata are recorded here.

## Gameplay adaptation

- Source talent spell: `152279` (Breath of Sindragosa).
- Triggered spell: `155166`.
- Survivors key: `breath_of_sindragosa`.
- Behavior: existing `Kind::Cone` targeting, with authored Survivors direct damage and damage-over-time.
- Cooldown: 14 seconds; cone radius: 16; damage-over-time duration: 3.5 seconds.
- The ability is added to the Death Knight pool. The current Survivors class model does not yet select one active Legion specialization at run loadout time, so Frost-only pool gating remains a separate follow-up.

These numbers are Survivors balance values, not copied from WoW.

## Legion visual mapping

The local 7.3.5 extraction confirms:

| Effect | SpellVisualEffectName ID | Model FileDataID | AttachmentID |
|---|---:|---:|---:|
| Main frost breath layer | 19257 | 948165 | 34 |
| Secondary smoke/frost layer | 19258 | 948167 | 34 |

The corresponding `SpellVisualKitModelAttach` rows are 27480 and 301331. Both FileDataIDs exist in the local CASC. The MD21 containers parsed and exposed textures; their skins are 948166 and 948168 respectively.

The implementation sends two one-shot `SpellKitFx::Begin` slots through the existing `SpellFx` pipeline, loading `Legion\\948165.m2` and `Legion\\948167.m2`. The Legion file reader already supports `legion/<FileDataID>.m2` and transcodes the model to the format consumed by Benilla's existing M2 loader.

## Timing and limitations

- The source visual uses multiple SpellVisualEvent start/end events and different SpellVisualKits (40635 and 40636). This first integration plays the two verified model layers together as one one-shot effect. It does not yet reproduce Blizzard's exact event ordering, target-type routing, caster animation, or original channel lifecycle.
- Attachment ID 34 is passed through the current attach-point fallback cascade. If a given unit model lacks that point, the existing SpellFx code falls back to attachment 15, attachment 19, then the unit root.
- The visual is emitted inside the regular `Fx::cast` path for SpellID 152279. This preserves the existing cast-event/FxLog path used by Survivors' host/guest visual replication; no network message or protocol schema is added.
- The two models and their dependencies are read from the local Legion CASC at runtime. No model/texture/sound bytes are copied into the repository.
- The current extraction could not reverse-resolve FileData.db2 paths (FileData.db2 is absent), but the M2 models' own named BLP texture paths are resolved and CASC-present.

## Validation status

Static mapping and regression tests are added. Compilation, runtime asset loading, and visual inspection still need to be run on the maintainer's Windows installation with `WOW_LEGION` and `WOW_LEGION_DBD` configured.
