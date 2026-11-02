# Phase 2 — Survivors spell adaptation catalogue

> Status: first exact-ID adapter slice. Based on the local-only Legion 7.3.5.26972 reports supplied for this task.
> No WoW DB2/DBC files or generated full inventory CSVs are copied into this repository.

## Scope

This phase establishes a strict boundary between a WoW source spell and a playable Survivors ability:

- A source `SpellID` identifies the WoW record.
- An `AbilityDef` identifies an implemented Survivors gameplay behavior.
- A name/icon match is not a gameplay implementation.
- An exact `SpellID` match may reuse an existing `AbilityDef`.
- An unmatched spell remains unsupported until a deliberate gameplay adaptation is implemented and reviewed.

The code adds `data::ability_for_spell_id`, an exact numeric lookup over the existing `ABILITIES` catalogue. It does not infer damage, cooldown, targeting, status effects, or visuals.

## Verified coverage from the supplied reports

The local inventory reports:

| Metric | Result |
|---|---:|
| Player classes | 12 |
| Specializations | 36 |
| Effective specialization talent seats | 756 |
| Unique talent SpellIDs | 603 |
| Exact talent-seat mappings to AbilityDef | 11 / 756 |
| Exact unique SpellID mappings to AbilityDef | 6 / 603 |
| Current Legion-class AbilityDefs | 29 |
| Those Legion AbilityDefs with valid local spell/name/icon references, gameplay Kind and nonzero Vanilla visual fallback | 29 / 29 |

The 11 seat matches represent repetitions of six unique source SpellIDs. They are not eleven different implementations.

## Exact mappings currently reusable

These are the six unique SpellIDs with exact matches in the current Survivors ability definitions:

| SpellID | 7.3.5 spell name | Survivors AbilityDef key | Gameplay kind | Existing visual() result |
|---:|---|---|---|---:|
| 6789 | Mortal Coil | `death_coil` | Bolt | 6789 (no explicit legacy fallback) |
| 12292 | Bloodbath | `sweeping_strikes` | Aura | 12292 (no explicit legacy fallback) |
| 17877 | Shadowburn | `shadowburn` | Strike | 17877 (no explicit legacy fallback) |
| 115098 | Chi Wave | `chi_wave` | Chain | 421 |
| 116847 | Rushing Jade Wind | `rushing_jade_wind` | Aura | 1680 |
| 211053 | Fel Barrage | `fel_barrage` | Bolt | 686 |

The first three have an exact gameplay key/SpellID mapping but no verified nonzero Vanilla visual fallback in the supplied report. The current `AbilityDef::visual()` therefore returns their source SpellID; this PR does not claim those visuals are valid in the 1.12.1 renderer.

## Candidates that are not automatic mappings

- Consecration (SpellID 202297) has a normalized exact-name candidate `consecration` in the supplied coverage CSV, but no exact SpellID match. It is not treated as implemented by the new lookup.
- A normalized name, matching icon, similar animation, or similar WoW tooltip is insufficient to reuse an AbilityDef without review.
- The other 597 unique talent SpellIDs have no exact current AbilityDef mapping in the supplied coverage report.

## What this PR changes

- Adds an exact `SpellID -> &'static AbilityDef` lookup.
- Adds regression tests for the six verified mappings.
- Adds tests that preserve the distinction between exact gameplay matches and unverified visual fallbacks.
- Documents current coverage and the rules for future adaptation.

## Explicitly out of scope

This slice does not:

- invent or rebalance damage, cooldown, range, target selection, duration, or status effects;
- generate AbilityDefs from spell names or numeric DB2 fields;
- add new class/spec pools or expose the 28 currently unexposed specializations;
- change the Vanilla talent model or the current Legion three-rank synthesis;
- change Profile serialization, host/guest messages, or simulation behavior;
- copy the supplied CSV reports or any local game data into Git.

## Next adaptation workflow

For each candidate spell, review its verified source data (Spell, SpellEffect, SpellMisc, SpellVisual and related visual-kit rows) and choose one of these outcomes:

1. **Reuse** an existing AbilityDef only when its behavior is genuinely equivalent.
2. **Adapt** by adding an explicit AbilityDef or a new reusable Kind with authored Survivors parameters.
3. **Defer** when the source effect, targeting, animation or asset chain cannot be represented accurately yet.

A new AbilityDef must declare its gameplay behavior and a valid visual strategy separately. Any visual fallback must be explicitly checked in the 1.12.1 renderer; nonzero alone is not a guarantee of visual equivalence.

## Validation

The new tests are part of `survivors::data`'s unit-test module. CI must run the repository's normal formatter, `benilla-app` library check and Survivors tests. This PR has not been compiled or visually tested on a local Windows game installation by the authoring environment.
