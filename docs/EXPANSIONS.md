# Later expansions' data: a feasibility study

Could the mode draw on a later client's data, Wrath of the Lich King (3.3.5a, build 12340) for
a start, for more classes, races, creatures, items and zones? This is a study, not a plan of
record: nothing here is built yet.

**Verdict.** Feasible, as an **optional content source beside the 1.12.1 base**, not as a switch
of the whole client to 3.3.5a. The base install keeps running everything it runs today (UI, menus,
tables); a 3.3.5a install, when present, is read by version-aware readers and its creatures, item
looks, zones and races are imported into the catalogs the mode already uses. It is a large piece
of work, but it splits into steps that each bring something visible.

## Why not move the client to 3.3.5a

- The client is a faithful reimplementation of the 1.12.1 binary: the readers name the reference's
  addresses, and every format crate says it reads 1.12.1 (`benilla-mpq`, `benilla-dbc`,
  `benilla-m2`, `benilla-adt`, `benilla-wdt`, `benilla-wmo`, `benilla-blp`).
- `benilla-formats` holds some 240 hand-written DBC schemas for build 5875: `Spell.dbc` at 173
  columns (234 in 3.3.5a), localized strings over 8 locales and a flags column (16 and a flags
  column in 3.3.5a).
- Upstream (benilla) targets 1.12.1 alone. Moving the whole client would make every upstream merge
  a rewrite (docs/UPSTREAM.md). Whatever reads 3.x data goes in new files or behind a switch.

## What changes from 1.12.1 to 3.3.5a, format by format

| format | today | for 3.3.5a | effort |
|---|---|---|---|
| MPQ (`benilla-mpq`) | formats V1 and V2, no PTCH | the same container; another chain order (`common`, `common-2`, `expansion`, `lichking`, the patches, the locale folder's archives); PTCH only arrives with Cataclysm | small |
| BLP2 (`benilla-blp`) | Raw1, Raw3, DXTC | unchanged | none |
| WMO v17 (`benilla-wmo`) | root and groups | mostly unchanged; a few flags | small |
| DBC (`benilla-dbc`, the same WDBC container) | 1.12.1 schemas | 3.3.5a schemas for the twenty to thirty tables needed, 16 locales | medium |
| ADT (`benilla-adt`) | monolithic, liquids in `MCLQ` | still monolithic (split ADTs arrive with Cataclysm); liquids in `MH2O`, 64×64 alpha maps (the WDT's `MPHD` flag), `MCNK` flags | medium |
| **M2** (`benilla-m2`) | MD20 versions **256 to 263** (vanilla and TBC) | **v264**: skin profiles in external `.skin` files, animation tracks as one array per sequence, some sequences in external `.anim` files, no playable animation lookup, particle and ribbon changes | **large** |
| characters (`characters/`) | 1.12.1 races and sections | Draenei, Blood Elves, the Death Knight; `CharSections`, hair and facial hair geosets read differently | medium to large |
| lighting, spell visuals | 1.12.1 tables | `Light*` and `SpellVisual*` reorganised | medium |

M2 v264 is the keystone: nearly everything new in 3.x (creatures, item models, doodads, the new
races' bodies) is in it.

## What each goal needs in the mode

The mode's numbers are its own (`survivors/data.rs`); the client only brings the looks: models,
icons, spell visuals, terrain.

- **Item looks**: 3.3.5a `ItemDisplayInfo`, M2 v264 for the models, and the textures. To check:
  `outfit::entry` packs an item's display into 16 bits; a 3.3.5a display id above 65535 would need
  another encoding.
- **Creatures and bosses** for the zones' rosters: 3.3.5a `CreatureDisplayInfo` and
  `CreatureModelData`, and M2 v264. The cheapest visible win.
- **Classes**: a class is a `data.rs` entry with its abilities; it needs the 3.3.5a `Spell.dbc` for
  icons and visuals, and a starting outfit. The Death Knight follows once the models load.
- **Races** (Draenei, Blood Elves): the race's M2 v264 body, the skin compositing and the five
  appearance dials (`outfit.rs`), their racials in `data.rs`.
- **Zones** (Northrend): the heaviest. 3.x ADTs with `MH2O` liquids, WDT, WMO, M2 v264 doodads,
  lighting, the minimap; then each zone's `MAPS` entry, roster and bosses.

## Constraints

- **Data.** A 3.3.5a client is some 17 GB: a second download in `scripts/fetch-data.sh`, still
  never in git (the "No Blizzard content in git" invariant holds).
- **The browser.** More data served through `/data/*`, and a heavier world to draw; to be measured.
- **Ids.** 3.3.5a display, spell and map ids overlap 1.12.1's: each must say which source it comes
  from (an offset, or a source tag carried with the id), on the wire too.
- **Upstream.** 3.x readers live in new files or behind a switch, so upstream merges stay cheap.

## A route, step by step

1. **Groundwork**: a second chain (`WOW_DATA_WOTLK`), version detection, and keeping the two
   sources' ids apart.
2. **The M2 v264 reader** (`.skin`, the per-sequence tracks, `.anim`): the big piece, which unlocks
   the rest.
3. **3.3.5a DBC schemas** for the tables needed, imported into the existing catalogs.
4. **Lich King creatures and bosses** in the rosters, and 3.x item looks: the first win seen in a
   run.
5. **Races** (Draenei, Blood Elves) and the **Death Knight**.
6. **Northrend zones**: 3.x ADTs, `MH2O`, lighting, the minimap.

**A cheaper step first: TBC (2.4.3).** `benilla-m2` already takes MD20 versions 260 to 263, TBC
models keep their skins inside, and TBC ADTs still carry `MCLQ`. Draenei, Blood Elves, Outland and
TBC items would mostly need the DBC schemas: a halfway step toward Wrath.

**The first concrete move, when the time comes**: a spike that loads one 3.3.5a creature model in
the current renderer through a v264 reader, to measure what M2 v264 really costs before committing
to the rest.
