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

## Since: native only, and Legion

Two things changed the choice after this study was first written.

- **The game ships as a native executable to a handful of players**, not in the browser. The
  browser's limits (WebGPU, memory, data streamed over `/data/*`) no longer weigh, and a later
  client's heavier art and storage become affordable. What reads that data is native only; the
  wasm build keeps compiling as it does.
- **Which clients can be found.** On archive.org:

  | client | item | size |
  |---|---|---|
  | Wrath 3.3.5a (12340) | `lrdpsychochains-wotlk-3.3.5a-12340-client` (one zip), `WoW_3.3.5-12340` (multi-language) | 17.7 GB, 41 GB |
  | Cataclysm 4.3.4 (15595) | `WoW_4.3.4-15595_Multi` | 31 GB |
  | **Legion 7.3.5 (26972)** | `lrdpsychochains-legion-7.3.5.26972-client`, a CASC install file by file | 74 GB |
  | Battle for Azeroth 8.3.7 | none found | — |

  BfA's formats are Legion's (CASC, file data ids, chunked M2, DB2 WDC), and BfA cannot be found;
  Legion can, its build 26972 is the one community tools cover (WoWDBDefs, CascLib, wow.export,
  TrinityCore's 7.3.5 extractors), and it brings nearly everything up to it: the Demon Hunter, Monk
  and Death Knight, the Pandaren, Worgen, Goblins and the first allied races, the Broken Isles,
  Pandaria, Draenor, Northrend and Outland. **Legion 7.3.5 is the target.**

## The Legion spike

Two native crates, read by nothing in the client yet:

- **`benilla-casc`**: a read-only CASC reader for a Legion install: `.build.info` and the build
  config, the local `.idx` indices, BLTE (plain and zlib; encrypted chunks refused), the encoding
  and root tables, a file by its file data id or by its path (the root's Jenkins hash, checked on
  lookup3's own test vectors). Its `remote` feature reads a copy by byte ranges through `curl`, with
  a cache, so the 74 GB install on archive.org is read a few megabytes at a time; its
  `legion_probe` example writes the files asked for.
- **`benilla-legion`**: Legion's chunked `.m2` (`MD21` around the MD20 model, `SFID`, `TXID`,
  `AFID`…), the MD20 model at versions 264 to 274 far enough to draw it in its bind pose (vertices,
  texture records and the paths type 0 ones name, materials, 88-byte bones, the texture lookup,
  bounds), and the external `.skin` (48-byte sections whose `level` carries the high bits, 24-byte
  batches). Its `legion_model` example reads a model, its first skin and its textures, writes an
  `.obj` with the textures, and draws a bind-pose preview with a small software rasterizer, no GPU
  needed.

What it showed, from archive.org's copy, with some 130 MB read of the 74 GB:

- the install opens: 732,305 files in root; a path finds its file through the Jenkins hash;
- `creature/illidan2/illidan2.m2` (Legion's Illidan, MD20 v272): 6,005 vertices, 127 bones, 54
  sequences (4 in `.anim` files), 4 skin profiles, 7,509 triangles in its first; its three
  textures, two named by the model's own records and one replaceable, drawn right;
- `creature/fellord/fellord.m2` and `creature/felbat2/felbat2.m2` read the same way.

What it leaves for later:

- **A creature's replaceable textures** (types 11 to 13) come from `CreatureDisplayInfo`, a DB2
  (WDC1) table: a DB2 reader, best driven by WoWDBDefs' definitions for build 26972. The spike
  guesses them from the community listfile, which is wrong whenever no texture bears the model's
  name (the Fel Lord's body draws grey).
- **Geosets**: which sections a creature shows is `CreatureDisplayInfo`'s too; the spike draws the
  body and each group's first variant.
- **Animation**: Legion's tracks (an array of keys per sequence) and the `.anim` files; 1.12.1's
  pipeline (`benilla-formats/src/models`) reads 1.12.1's tracks from the raw bytes.
- **Into the client**: turning a Legion model into the `benilla_m2::M2Model` the renderer draws,
  without editing upstream's files: `benilla-m2`'s header walk already places the fields for
  versions above 263; its records' strides and its version gate stop it.
- Then characters (HD bodies, the new races), item models, maps (split ADTs), WMO with file data
  ids, and the data download (a fetch script for the 74 GB, or each player's own install).

## The Legion route, step by step

One choice runs through it: **a Legion asset is transcoded into a 1.12.1 one as it loads** (a
Legion M2 rewritten as an MD20 version 256 file with its skin inside and 1.12.1's tracks), so the
client's renderer, animation, attachments and particles draw it unchanged and upstream's crates
are barely touched. Teaching the renderer Legion's formats instead would conflict with every
upstream merge. Each step is one pull request or a few.

1. **Legion's DB2 tables.** A reader for WDC1 (build 26972's format), driven by WoWDBDefs'
   definitions (`.dbd`) rather than hand-written schemas. The definitions are WoWDBDefs' data,
   licensed CC BY-SA 4.0: they are read from a local folder, never copied into this repository.
   First `CreatureDisplayInfo`, `CreatureModelData`, `CreatureDisplayInfoExtra`,
   `ItemDisplayInfo`, `ModelFileData`, `TextureFileData`, `AnimationData`. Done when the spike
   draws a creature with its own skins and geosets, nothing guessed from the listfile.
   **Done**: `benilla-legion`'s `dbd` (a definition's layout by the DB2's layout hash, else by
   build) and `db2` (WDC1: plain, bit-packed, common, pallet and pallet-array fields, the id list,
   the copy table, the relationship map; sparse tables refused) and `display` (a display's model,
   replaceable textures, `CreatureGeosetData` geosets and scale; 7.3.5 has no
   `CreatureDisplayInfoGeosetData`). All seven tables read from build 26972 (`legion_tables`):
   66,165 creature displays, 91,634 item displays with copies. With `LEGION_DBD`,
   `legion_model` draws the Fel Lord in its own skins (display 61830), the Fel Bat in its body and
   wings (65202), Illidan at his display's scale (21135).
2. **The M2 transcoder, still.** A Legion M2 to a valid MD20 v256: vertices, the skin inside,
   textures named by virtual paths, materials, bones in their bind pose, the geosets its display
   picks, Legion's shader combos brought to the nearest 1.12.1 blend. Done when `benilla-m2`
   reads the result, and a test holds Illidan's counts.
   **Done**: `benilla-legion`'s `transcode` (`to_vanilla`): the 1.12.1 header (`0x144` bytes) with
   the Legion arrays it shares copied as they are (vertices, lookups, collision), the skin as one
   embedded view (sections cut to 32 bytes, the batches of the geosets the display hides dropped,
   one texture a batch), textures named `Legion\<file data id>.blp` (`texture_files`: `TXID`,
   else the record's path, else the display's replaceable one), materials to 1.12.1's flags and
   blends (Legion's 7, a premultiplied blend, to an alpha blend), bones keyless in their bind pose, sequences laid end to end on one
   timeline, colours and weights at their first key, attachments; no texture transforms, events,
   lights, cameras, ribbons or particles yet. `benilla-m2` and the client's own loader
   (`parse_m2_render_submeshes`) read the result: Illidan (6,005 vertices, 127 bones, 3 sections,
   14 attachments) in two draws, his glow left out as its alpha is 0 in his stand, as 1.12.1's
   static cull wants until step 4 brings the tracks; the Fel Lord in ten (display 61830), the Fel
   Bat in two (65202). `cargo test -p benilla-legion --test legion_data -- --ignored` holds
   Illidan's counts; `legion_model` writes `vanilla.m2` beside its preview.
3. **The Legion source in the asset chain.** `WOW_LEGION=<folder>` (native only) opens the install
   through `benilla-casc`; virtual paths (`Legion\<file data id>.m2`, `.blp`) are answered by our
   layer, which transcodes models and passes textures through. One hook into upstream's asset
   chain, kept small or in a file of our own. Done when a Legion model stands in a test scene.
4. **Animation.** Legion's tracks (an array of keys per sequence) laid onto 1.12.1's timeline and
   its ranges; the sequences kept in `.anim` files; Legion's animation ids mapped to 1.12.1's
   (stand, walk, run, attacks, death, casts), the nearest one where 1.12.1 has none. Done when
   Illidan walks, strikes and dies in the test scene.
5. **The first Legion content in a run.** Legion creature displays in `data.rs`'s rosters and
   bosses, in an id range of their own (on the wire too); an Illidan bonus boss, the Legion's
   demons in a zone. In a lobby every player needs the Legion data: the lobby checks it, and
   refuses or falls back to a 1.12.1 display. **The first milestone players see.**
6. **Particles, ribbons, effects.** Legion's emitters and ribbons transcoded (their records
   differ): fel fire, glowing weapons.
7. **Items.** Weapons first (plain M2s, `ItemDisplayInfo` → `ModelFileData` with its race and sex
   variants), as loot in `data::ITEMS` and on the hero; armour after step 8, which composites it.
8. **Characters, races, classes.** The races' HD bodies (another texture layout, Legion's
   `CharSections` and geosets); then the new races (Pandaren, Worgen, Goblin, Draenei, Blood Elf,
   the first allied races) and classes (Demon Hunter, Monk, Death Knight) in `data.rs`, with
   Legion's spell icons and visuals. A choice to make then: every hero in HD, or 1.12.1's bodies
   kept and only the new races added.
9. **Maps.** Legion's split ADTs (`_tex0`, `_obj0`, `_obj1`), WDT and WMO by file data id, `MH2O`
   liquids, lighting and skies; Broken Isles zones in `data::MAPS`. The biggest step.
10. **Shipping.** A `fetch-legion` script (74 GB from archive.org, checked against the uploader's
    SHA256) or each player's own install; the menu shows Legion content only when the data is
    there; `make dist` for the native executable.

The order: 1 to 5 for the first milestone (a Legion boss, animated, in a run), then 6 and 7, then
8, and 9 last. Unit tests run on synthetic data; tests on real data read a local `LEGION_DATA`
and are `#[ignore]`d, as the balance simulator is, so CI needs no data.
