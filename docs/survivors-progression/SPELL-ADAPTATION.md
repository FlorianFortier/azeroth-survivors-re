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


## Follow-up: effect and visual-chain audit for the eight exposed specializations

The local 7.3.5.26972 extraction supplied after the initial catalogue adds three reports:

- `talent-spell-details.csv`: 144 unique talent SpellIDs, one deduplicated row per `(spell_id, effect_index)`.
- `talent-visual-chain.csv`: 144 talent-spell visual lookups.
- `talent-unresolved-references.csv`: 260 unresolved references, representing 87 unique source values.

The validation summary reports all 144 SpellIDs processed and non-empty classification/evidence fields. Those classifications must not be interpreted as gameplay roles:

- 114 rows are labeled `ACTIVE` by the extractor because an actionable aura pattern was found.
- 30 rows are `UNKNOWN` and explicitly flagged for manual review.
- No rows were classified `PASSIVE`, `SUMMON` or `TRANSFORMATION`.
- The audit itself notes that the passive heuristic did not match known passive aura patterns, and that transformation aura 305 was deduplicated.

**Conclusion:** `ACTIVE` is a preliminary extraction heuristic, not proof that the talent SpellID is a player-cast active ability. Aura-based talents can be passive modifiers or proc handlers. Role assignment requires checking the complete talent context and linked/triggered spells.

### Visual-chain status

| Lookup result | Count |
|---|---:|
| No `SpellXSpellVisual` row for the talent SpellID | 86 |
| `SpellXSpellVisual` row exists but no valid kits were resolved | 58 |
| Talent-spell rows with a valid kit in this report | 0 |

The unresolved-reference report contains 86 missing talent-to-visual mappings and 174 references to `SpellVisualKitID = 4294967295`. This does not prove the actual cast has no visual: some talent spells are auras/procs whose triggered spell carries the cast visual.

### Triggered spells requiring a second extraction pass

The details report contains 17 nonzero `effect_trigger_spell` links. Their target SpellIDs were not included as roots in the supplied visual-chain report:

| Talent spell | Talent SpellID | Triggered SpellID | Specialization IDs |
|---|---:|---:|---|
| Rushing Jade Wind | 116847 | 148187 | 268, 269 |
| Power Strikes | 121817 | 129914 | 269 |
| Breath of Sindragosa | 152279 | 155166 | 251 |
| Frozen Pulse | 194909 | 195750 | 251 |
| Blighted Rune Weapon | 194918 | 195758 | 252 |
| Bloodworms | 195679 | 196361 | 250 |
| Eye of the Tiger | 196607 | 196608 | 268, 269 |
| Refreshing Jade Wind | 196725 | 162530 | 270 |
| Special Delivery | 196730 | 196734 | 268 |
| Prepared | 203551 | 203650 | 577 |
| Demon Blades | 203555 | 203796 | 577 |
| Master of the Glaive | 203556 | 213405 | 577 |
| Desperate Instincts | 205411 | 205478 | 577 |
| Runic Attenuation | 207104 | 221322 | 251 |
| Avalanche | 207142 | 207150 | 251 |
| Blade Turning | 247254 | 247253 | 581 |
| Chaos Blades | 247938 | 211797 | 577 |

These are source-data links, not confirmed cast animations. The target spells must be extracted and inspected before deciding whether they represent a cast, periodic proc, damage event, summon, or purely internal aura application.

### Required next local extraction

Extract the 17 triggered SpellIDs above as additional roots from `Spell`, `SpellEffect`, `SpellMisc`, `SpellCooldowns`, `SpellCastTimes`, `SpellRange`, `SpellDuration`, `SpellXSpellVisual` and the related visual-kit/effect/model attachment tables.

For each target, also follow nonzero `SpellEffect.TriggerSpell` references one level further, recording the parent SpellID and effect index. Produce:

- `trigger-spell-details.csv`
- `trigger-visual-chain.csv`
- `trigger-unresolved-references.csv`

Preserve raw source fields and unresolved references. Do not label these target spells as Survivors actives solely because they are referenced by a talent.

The three source reports remain local. This repository contains only this summarized, non-asset audit and the exact-ID code adapter.
