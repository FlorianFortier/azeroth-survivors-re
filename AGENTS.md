# Working in this repository

*Azeroth Survivors* is a local, LAN build of Aethus' fan-made WoW roguelike, running on
[wenilla](https://github.com/Arnesen/wenilla) — the WebAssembly build of
[samwhosung/benilla](https://github.com/samwhosung/benilla), a from-scratch WoW 1.12.1 client in
Rust + Bevy. There is no server, no login and no realm: the game boots straight into the
survivors mode, natively or in the browser. This file is the map for a person or an agent who
has to build, fix or change something here. Upstream's own map and rules for the client crates
are `docs/METHOD.md`, `docs/MAP.md` and `docs/CONTRIBUTING.md`; read them before changing client
code. Syncing upstream is [docs/UPSTREAM.md](docs/UPSTREAM.md). What reading a later client's data
(Wrath 3.3.5a, TBC 2.4.3) would take is studied in [docs/EXPANSIONS.md](docs/EXPANSIONS.md).

## What lives where

| path | what | whose |
|---|---|---|
| `crates/benilla-app/src/survivors/` | **the game**; file by file in [The survivors mode](#the-survivors-mode-file-by-file) | ours |
| `crates/benilla-*` | the client: formats, world, UI, protocol, app | upstream's, plus our `#[cfg]`-gated carries |
| `crates/benilla-app/src/webbridge/` | the JavaScript bridge, wasm side | wenilla's |
| `crates/benilla-app/src/survivors/character.rs` | the main screen (title, tabs, Character tab: class column, hero column with the hero's portrait, zone column, bottom bar; the Armory: race, look and gear around the hero on its stage), built from the kit on the same `Selection` and profile as the glue menu (`menu.rs`), which it replaces; `WOW_NEW_MENU=0` (`?new_menu=0`) brings the glue menu back | ours |
| `crates/benilla-app/src/survivors/outfit.rs` | the hero as a player character: the race's body, the five appearance dials, and the pieces it wears, dressed from item entries the mode answers itself (`outfit::template`); `WOW_HERO_BODY=trainer` brings back the class trainer's look | ours |
| `crates/benilla-app/src/survivors/talents.rs` | talents: the class's three trees from `Talent.dbc`/`TalentTab.dbc` under vanilla's rules, each talent a stat of its tree's theme per rank; points trained with gold (`point_cost`), 51 at most | ours |
| `crates/benilla-app/src/survivors/lang.rs` | the mode's languages: text is written in English and looked up for French (`lang::tr`, `trf` for `{}` templates); the profile's `options.language` picks it, and a change rebuilds the main screen | ours |
| `crates/benilla-app/src/survivors/treasure.rs` | the run's field: chests opened with the run's gold, elites at fixed posts that come back, their rewards (gear roll, spell rank, run item, gold), and the shrines (Power, Challenge, Greed, Curses) | ours |
| `crates/benilla-app/src/survivors/names.rs` | hero names: a random one in each race's sound, and what a typed one may hold | ours |
| `crates/benilla-app/src/survivors/kit.rs` | the menu's UI kit: panels, headings, tabs, icon and call-to-action buttons, stat tiles, progress bars, chips, tooltips; clicks read through `KitClicks`. `WOW_UI_KIT=1` (`?ui_kit=1`) shows a gallery over the menu | ours |
| `crates/benilla-app/src/survivors/profile.rs` | the saved profile (`benilla-config/survivors-profile.toml`, `localStorage` on the web): pick, run records, and the meta-progression as it lands | ours |
| `crates/benilla-app/src/survivors/net/` | multiplayer, game side: socket, host stream, guest copy, lobby screen | ours |
| `crates/survivors-net` | multiplayer protocol, lobby settings, the lobby hub's state machine. No I/O, no deps | ours |
| `crates/survivors-server` | the lobby server (`make lobby`); its router is mounted by `wenilla-host` at `/lobby` | ours |
| `crates/warcraft-survivors` | the native launcher (`make native`) | ours |
| `crates/benilla-casc`, `crates/benilla-legion` | the route toward Legion 7.3.5's data, native only, read by the client when `WOW_LEGION` names an install (`benilla-app/src/entities/legion.rs`: the `mpq://legion/…` paths through `benilla_assets::set_extra_source`, the creature displays into the catalog from `library::DISPLAY_BASE`): a CASC reader (with a `remote` feature reading a copy by byte ranges) Legion's chunked M2 and `.skin`, and its DB2 tables (WDC1, typed by WoWDBDefs' `.dbd` read from a local folder, never vendored) with creature displays, a transcoder writing a Legion model as a 1.12.1 MD20 (`transcode`), and the `library` answering the client's `legion/` paths and the Broken Isles' tiles
(`terrain`: a Legion tile's split files written as one 1.12.1 ADT, the map added to the client's
`Map.dbc` table; `wmo`: its buildings as 1.12.1's; `lights`: its lights, added to the client's light
catalog); their examples draw a model to a PNG without a GPU. See [docs/EXPANSIONS.md](docs/EXPANSIONS.md) | ours |
| `crates/wenilla` | the wasm entry crate (`#[wasm_bindgen(start)]`), boots the survivors mode | ours |
| `crates/wenilla-host` | local dev server: static files, `/data/*`, and the lobby at `/lobby`. Loopback by default. | wenilla's |
| `web/` | the page side: `index.html`, `boot.js`, `platform.js`, `bridge.js`, `examples/`, `wasi_stubs.js` | wenilla's |
| `scripts/fetch-data.sh` | downloads the 1.12.1 client and extracts its MPQs into `wow-data/` | ours |
| `scripts/fetch-legion.sh`, `scripts/dist.sh` | Legion 7.3.5's game files (or a streamed copy's list) and WoWDBDefs' definitions, pinned, into `legion-data/`; the player build into `dist/`. The launcher (`crates/warcraft-survivors`) finds `wow-data/` and `legion-data/` beside itself or in the folder it runs from, a set variable winning | ours |
| `scripts/web-setup.sh`, `scripts/web-build.sh` | toolchain and build for the wasm | wenilla's |
| `third_party/` | patched `lua-src`, `mlua-sys` and `kira`; each has a `BENILLA.md` | vendored |
| `.github/workflows/check.yml` | the PR gate: wasm check, host and UI tests, client-carry and survivors-table tests | ours |
| `.github/workflows/ci.yml` | the gates of `scripts/gates.sh` that need neither game data nor a display | — |

`crates/wenilla-host` also carries a `/ws/*` relay to a mangos realmd/worldd. The survivors mode
never opens it; it is inherited from wenilla.

## The survivors mode, file by file

All under `crates/benilla-app/src/survivors/`. Each file opens with a `//!` header that says what
it holds; read it before changing the file.

| file | what |
|---|---|
| `mod.rs` | the plugin: `Phase` (Menu, Playing, LevelUp, Paused, GameOver, Lobby), `Run`, `Selection`, `Vitals`, `HeroState`, and the system sets and their order (`WorldStage::Input`, `SimSet`) |
| `data.rs` | every table and number: classes, abilities, passives, racials, zones (`MAPS`), rosters and bosses, waves, items (`ITEMS`), difficulties, achievements |
| `stats.rs` | a hero's numbers from every `data::Bonus` source; the one place they combine |
| `hero.rs` | the local hero: WoW's mouse and keys in the chase view (`MouseLook`: a left-drag swings the camera, a right-drag turns the hero, both run, a press let go in place is a click; `CamRig::facing` is the hero's heading, A/D turn it, Q/E strafe), movement under WoW's rules (backpedal at `RUN_BACK_RATIO`, a running jump locked to its momentum, one walk-speed nudge for a standing jump, no ground inertia), the jump, the chase/top-down cameras, health, and the menu camera that frames the hero into the screen's stage (`MenuStage`, `stage_eye`) |
| `abilities.rs` | auto-cast: each owned ability fires on its cooldown at the targets its kind picks; the local hero's target first for those that strike the nearest (`aimed_order`) |
| `rewards.rs` | the Rewards tab: reward points (`AchievementDef::points`, a tenth of an achievement's gold, earned once it is done) spent on perks (`data::PERKS`), races opened to every class (`data::RACE_POINTS`), tabards (`data::TABARDS`), head and back appearances (`data::APPEARANCES`) and companions (`data::COMPANIONS`, with their instinct and their training, `data::PET_TRAINING`), each bought or earned by an achievement (`data::Unlock`), on a draft kept with Confirm (`profile::Spend` sums it), and the perk reset (`data::MAX_PERK_RESETS`), which leaves the rest; `apply_perks` puts the non-bonus ranks on a hero at its run's start |
| `companion.rs` | the hero's companion: a vanity pet at its side in every run that runs for the pickups within its leash (its instinct's first) and sends each it comes near flying to its hero; spawned by `sync::begin_run` for every hero that brings one, the host's like everything the run does |
| `sheet.rs` | the character sheet on `C`, over a paused run: the paper doll (gear worn, else the starting outfit; the neck, rings and trinkets are the run items), the live hero framed in its stage, the hero's numbers and its items' effects |
| `targeting.rs` | the hero's target: a click on an enemy, Tab, Esc; mirrored into the client's `target::Selection`, whose ring draws under it |
| `mobility.rs` | the class movement spell on `F` from level 5 |
| `enemies.rs` | the horde: spawning (the stage's crowd, its waves: a surge of packs every `data::WAVE_EVERY`, a lull between, all under `HARD_CROWD_CAP`), the chase, contact damage, death, experience gems, loot |
| `cards.rs` | the level-up cards as they show (the key and NEW or RANK badge, icon, rarity, what a new spell does, the perks in green, the foot; the deal-in, tilt and burst), the Banish button under each, and the row under them (the spell slots, Skip and Reroll) |
| `offer.rs` | the level-up offer as the simulation deals it: four spell cards per banked level at drawn rarities (`data::CardRarity`), each with the perks it grows (`draw_perks`), rerolls, banishes and skips, and `apply_action`, which takes a move only if it fits the hero's offer |
| `units.rs` | local units built as streamed ones (`Guid`, `NetEntity`, `ObjectStore`), and `Fx`, every visual |
| `sync.rs` | seating the run's heroes (and each guest's bonuses, `seat_bonuses`), the local player's moves at the cards onto its hero, the local hero mirrored into `Run` |
| `outfit.rs` | the hero as a player character: body, the five appearance dials, worn pieces |
| `talents.rs` | the class's three trees from `Talent.dbc`, vanilla's rules, the mode's numbers |
| `profile.rs` | the saved profile: pick, records, gold, inventory, equipped, talents, codex, achievements |
| `names.rs` | random names in each race's sound, and what a typed one may hold |
| `kit.rs` | the menu's UI kit (panels, headings, tabs, buttons, icons, tiles, bars, chips, tooltips, text fields, scroll areas: the wheel scrolls the one under the pointer, and a bar on its right shows and drags it); `?ui_kit=1` shows a gallery |
| `character.rs` | the main screen: title, tabs (Character, Collection, Drop Tables, Codex, Talents, Achievements, Rewards), the Character tab's class, hero and zone columns and bottom bar, and the full-screen Armory (race, sex and slots; the hero on a stage a drag turns; appearance dials and the pieces owned for the slot picked) |
| `menu.rs` | the older pick screen built from the glue screens' own art, behind `?new_menu=0`; its pick helpers (`pick_class`, `all_looks`, `save_pick`…) serve `character.rs` too |
| `hud.rs` | the in-run HUD, pause, results, tooltips, and `start_run` |
| `frames.rs` | the HUD's WoW frames: the player and target unit frames at the top left, the damage meter (`Vitals::damage_by`, each hit credited to its `Hit::spell`), the action bar with its gryphons under the spells and the movement spell, and the line under the clock (`next_boss_line`, `wave_line`) |
| `feel.rs` | how a hit feels on every machine: the struck model's brighten (the client's highlight bit, in `Last`) and the camera's shake (`Options::shake`), both read off `Struck`, which `units::Fx::number` writes; the shove itself is the simulation's (`data::knock_impulse`, `Enemy::knock`) |
| `legs.rs` | the run in stages: the boss portal that calls a stage's final boss once its time has come, the stage's rewards when it falls (and every gem on the field pulled in, `enemies::Vacuum`), the portal onward to the next zone (everyone taken over and the zone loaded again), and the final swarm to hold out |
| `market.rs` | the shady merchant (three run items for the run's gold, restocked, dearer with each bought; the shop panel at it) and the jars and crates a hero breaks by walking into them (gold, a heal) |
| `marks.rs` | the field's marks over the world: rings on the ground under chests and elites, a fallen elite's post filling as it comes back, a chest's price and an elite's reward over them; all read off `treasure::FieldMark` and `Run::posts`, so guests see them too |
| `minimap.rs` | the minimap cluster and the day clock; the field's chests, elites and empty posts on it |
| `grade.rs` | the picture polish pass (`G`, `?grade=0`) |
| `auto.rs` | `WOW_SURVIVORS_AUTO`: a hands-off run for testing, with screenshots |
| `balance.rs` | the balance simulator (tests only): whole runs played headless through `SimSet`'s own systems on an empty world, a pilot playing the hero (kiting by look-ahead, chests, portals, the boss targeted, the rarest card), each stage logged (time, level, kills, damage taken, the portal, the boss); `--ignored` runs every class and prints the table, `trace_a_run` (`SIM_CLASS`, `SIM_ZONE`, `SIM_SEED`, `SIM_LIMIT`) what one run faced every ten seconds |
| `net/` | multiplayer, game side: `socket.rs` (WebSocket, native or browser), `host.rs` (plays and streams the run), `client.rs` (a guest's copy), `smooth.rs` (snapshot interpolation: what another machine moves is drawn `smooth::DELAY` behind its clock, between two positions it sent), `ui.rs` (lobby screens, party frames, scoreboard) |

## Switches

Native reads `WOW_<NAME>` from the environment; the browser reads `?<name>=` from the page's query
string (`crate::webenv::var`: the prefix dropped, lowercased).

| native | browser | what |
|---|---|---|
| `WOW_UNLOCK_ALL=1` | `?unlock_all=1` | every zone open |
| `WOW_NEW_MENU=0` | `?new_menu=0` | the glue-art pick screen (`menu.rs`) instead of the main screen |
| `WOW_UI_KIT=1` | `?ui_kit=1` | the kit's gallery over the menu |
| `WOW_GRADE=0` | `?grade=0` | start with the picture polish off |
| `WOW_MOUSE_SENS=1.5` | `?mouse_sens=1.5` | chase-view mouse speed, 0.1 to 10 |
| `WOW_HERO_BODY=trainer` | `?hero_body=trainer` | the class trainer's look instead of a player body |
| `WOW_NAME=…` | `?name=…` | the player's name in a lobby |
| `WOW_LOBBY=…` | `?lobby=…` | the lobby server's address |
| `WOW_SURVIVORS_AUTO=<class>[:idle\|:all\|:late]` | — | a hands-off test run; `WOW_SURVIVORS_MAP=<index or name>` picks the zone, `WOW_SURVIVORS_SHOTS=<dir>` saves screenshots |
| `WOW_DATA=…` | — | the 1.12.1 client's `Data` folder |
| `WOW_LEGION=…` | — | a Legion 7.3.5 install's folder (or a remote copy's URL, cached in `WOW_LEGION_CACHE`): its models and textures as `mpq://legion/…`; the launcher sets it (and the cache, and `WOW_LEGION_DBD`) from `legion-data/` when unset |
| `WOW_LEGION_DBD=…` | — | a folder of WoWDBDefs' `.dbd` definitions (never copied here): Legion's creature displays join the catalog as 1,000,000 plus their id |

## Commands

```bash
make build                  # fetch-data + web-setup + web-build (~5.5 GB download the first time)
make start                  # http://127.0.0.1:8090/ — the browser build
make native                 # the native desktop build (finds wow-data/ and legion-data/)
make fetch-legion           # Legion 7.3.5's game files into legion-data/ (71 GB, SHA-1 checked);
                            # STREAM=1: read from archive.org as needed instead
make dist                   # dist/azeroth-survivors/: the player build, the fetch scripts, a README
scripts/web-build.sh        # → web/dist (WebGPU build). WEB_DEBUG=1 keeps symbols for readable stack traces.
cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
                            # the wasm build without linking; what check.yml runs
cargo check --workspace --all-targets      # native (Debian/Ubuntu: libasound2-dev libudev-dev)
cargo test -p benilla-app --lib survivors  # tables, balance, cards, host→guest replication; no game data
cargo test -p survivors-net -p survivors-server  # multiplayer protocol, lobby rules, server end to end
cargo test -p benilla-app --lib --no-default-features survivors::balance -- --ignored --nocapture
                            # the balance simulator: every class through whole runs, the table printed
make lobby                  # a standalone lobby server on :7777
cargo test --workspace                     # ~320 tests need a 1.12.1 patch chain and fail without one
```

### Without game data (a cloud container, CI)

Most work here is done without the 1.12.1 client, as `check.yml` does. What works and what does not:

- `cargo test -p benilla-app --lib survivors` (add `--no-default-features` as CI does) runs the
  mode's tables, balance, cards, stats, profile, camera framing and host→guest replication with
  no data. A native build needs `libasound2-dev libudev-dev` (CI also installs `libwayland-dev
  libxkbcommon-dev`); the first build takes about ten minutes.
- The wasm `cargo check` needs `scripts/web-setup.sh` first: the vendored `mlua-sys` builds the
  Lua VM against wasi-sdk, and fails at its build script without it.
- Nothing shows on screen without the data and a WebGPU adapter. A UI or camera change cannot be
  seen there: say so in the PR, and leave the in-game check to someone with the client.
- `cargo fmt -p benilla-app` also reformats upstream files that are not formatted; run `rustfmt
  --edition 2021` on the files you changed, and leave upstream files as they are.

A browser needs WebGPU; the world does not run without it. Linux Chrome needs
`--enable-unsafe-webgpu`. Without an adapter the client aborts in
`wgpu::create_bind_group_layout` at world entry: that is the missing adapter, not your change.
A throwaway profile with a devtools port, for driving it from a script:

```bash
google-chrome --user-data-dir=/tmp/wenilla-chrome --enable-unsafe-webgpu --remote-debugging-port=9333 http://127.0.0.1:8090/
```

## Invariants

- **No Blizzard content in git.** `wow-data/`, `WoW/`, extracted assets and real-client test
  fixtures are git-ignored; keep them that way.
- **The game's numbers live in `survivors/data.rs`.** Spell ids there name `Spell.dbc` rows only
  for icon, name and visual; every number is the mode's own. Every key a class lists in `start`
  or `pool` must exist in `ABILITIES` (`data::ability` panics otherwise), and every zone needs at
  least three creatures on its roster; `data.rs`'s tests check both.
- **Carries into upstream files are `#[cfg]`-gated or in our own files.** A change inside an
  upstream file goes under `#[cfg(target_arch = "wasm32")]` or `cfg(not(target_os = "macos"))`,
  or into a new file. The next upstream merge conflicts on every upstream line a carry touches,
  so touch few.
- **Upstream is merged, never copied.** A sync is a real merge commit with the upstream branch as
  its second parent. See docs/UPSTREAM.md.
- **What ships is the `cp` line in `scripts/web-build.sh`.** A page-side file that is not on it
  never reaches `web/dist`.
- **`wasm-bindgen` crate and CLI versions must match exactly**; `web-setup.sh` enforces it.
- **No threads, no std time on wasm.** `std::time::Instant`, `SystemTime` and `env::vars_os`
  panic there; use `bevy::platform::time::Instant` or `web-time`. The wasm target is caught only
  by `check.yml` or your own `cargo check --target wasm32…`.
- **`ring_reaction` returns 0..7**; every consumer adds `+ 1` for the Lua 1..8 scale.

- **Multiplayer: the host plays, guests copy.** Only solo and the host run `SimSet`; a guest
  builds the host's units in `net/client.rs` and moves only its own hero. Anything that changes
  the run goes through the simulation (`HeroState`), never straight into `Run`, which is only the
  local hero's view. Visuals go through `units::Fx`, which the host also logs for its guests.
- **A dropped guest is away, not gone.** In a run, a guest's connection dropping keeps its seat
  (`hub::Seat::conn` none, `Member::away`) for `Up::Rejoin` with the token its `Down::Started`
  gave; the host keeps the hero, `Vitals::away` (harmed by nothing, no one's quarry, casting
  nothing, down for the loss, not waited for at a load). Back in, the guest clears its copies
  (`client::rejoined`) and sends `Game::Resync`; the host's next tick sends that seat alone every
  unit (`host::stream`). The host's own drop still ends the run.
- **Every stat goes through `survivors/stats.rs`.** A hero's numbers are `Vitals::stats()`:
  the class head start, the run's passives, `Vitals::meta` (what the profile brings in:
  collection, talents, racials, gear, through `Profile::run_bonuses`; a guest's lobby `Loadout` carries its
  `Profile::meta_bonuses` and gear names, and the host adds `data::racial_bonuses` to them in
  `sync::seat_bonuses`) and the running effects,
  all as `data::Bonus`, combined `(base + Σflat) × (1 + Σpercent)`. A new source adds bonuses;
  it never edits a stat by hand. `stats.rs`'s tests hold the old numbers fixed.
- **A text field owns the keyboard while focused.** `kit::KitFocus::typing()` is set while a
  name is typed; a system that reads keys in the menu (Enter, G, C…) checks it first.
- **The profile only grows.** A new profile field takes `#[serde(default)]`, so every older
  profile keeps loading; renaming or changing a field's meaning bumps `PROFILE_VERSION`. Classes,
  zones and items are stored by name, never by table index.
- **The campaign is `data::MAPS`, in order.** Each zone has exactly five bosses, one per
  `data::BOSS_TIMES` slot, the last its final boss. A run is `data::STAGES` stages, each a zone
  from the one picked on (`data::stage_zone`, `Selection::stage` the one stood in), its clock
  `STAGE_LENGTH` of the run's and its foes tougher each stage (all in `Run::threat`). The
  final boss is never the clock's: its time opens the boss portal, a hero standing in it calls
  the boss, and its fall (`Run::final_slain`) clears the stage (`legs.rs`): rewards, then the
  portal onward, or after the last stage the final swarm; the run is won when that is held
  (`Run::cleared`). The run goes on the record of the zone it began in (`Selection::map`). A
  zone opens when the one before it has a win on the profile (`Profile::zone_open`). A zone renamed keeps its records through
  `profile::RENAMED_ZONES`. The first `data::CLASSIC_ZONES` are 1.12.1's; after them come
  Legion's zones (`MapDef::legion`: Val'sharah, on the Broken Isles), offered only where
  `entities::legion` loads Legion's maps and draws its creatures (`data::zone_shown`), never in a
  lobby (`net::ui`'s zone rule, and a run's zone held to the classic ones in `net::mod`), and
  never stepped into from a classic run (`data::stage_zone`: a Legion zone's run stays in it).
  The legendary finals are the classic campaign's last two and every Legion zone's. A Legion
  creature may be drawn at its own size over the crowd's scale (`Foe::legion_sized`,
  `EnemyDef::legion_scale`, `EnemyDef::drawn_scale`): its body's room and reach stay the
  scale's.
- **A hero is a player, dressed from its fields.** Its worn pieces are `PLAYER_VISIBLE_ITEM_*`
  entries with the top bit set (`outfit::entry`); every machine answers their templates from the
  entry alone (`outfit::queue_templates`), so a guest needs nothing but the host's fields.
- **Gear is `data::ITEMS`: five per zone, one per boss, stored by name.** A boss always drops
  its own; the horde one of its zone's in `HORDE_ITEM_ODDS`. What a hero catches goes on
  `Vitals::loot` and onto the profile's `inventory` when the run ends, won or lost. What is
  `equipped` adds its bonuses through `Profile::run_bonuses` and its look through `outfit::body`,
  a display picked from `ItemDisplayInfo` by the icon's slot (`outfit::gear_worn`).
  Every item ever found is in the collection (`Profile::collected`), which adds
  `data::collection_bonus` (a `COLLECTION_SHARE` of its first bonus) to every run.
- **The run's field is `survivors/treasure.rs`, in `SimSet`.** Gold is the run's (`Vitals::gold`,
  coins from kills, a pile from bosses, a purse from elites); a chest opens at once for a hero who comes
  to it with `data::chest_cost` gold and offers it `data::CHEST_CHOICES` run items
  (`treasure::deal_chest`, on `Vitals::chest_offer`, shown first on the level-up cards); a guest
  answers with `Game::ChestChoice`, which `treasure::take_chest_item` takes only for an item of
  that offer. Elites hold fixed posts (`data::ELITES`, `Enemy::home`),
  drop their kind's `data::Reward` and come back on its timer; run items (`data::RUN_ITEMS`) stack
  on `Vitals::run_items` and count in `Vitals::stats`, five kinds at most
  (`data::MAX_RUN_ITEM_KINDS`, the sheet's neck, rings and trinkets): once full, `treasure::grant`
  draws among the kinds held, and gold once each is at `MAX_RUN_ITEM_STACKS`. Half the unspent gold goes to the profile.
  A run item has a rarity (`data::CardRarity`, drawn by `item_odds` in `data::draw_run_item`)
  and may have a `data::ItemEffect` beyond its stat (lifesteal, an arc, thorns, a burst, a
  heal on kills, a burn, a chill, an execute, a dodge, a shield on kills, a nova, a bounty),
  each number per stack (`ItemEffect::at`, the execute and the dodge capped). Only the
  simulation fires them, in `abilities::apply_hits` (`afflict`, `on_hit`, `on_kill`),
  `abilities::item_novas` and `enemies::move_enemies`, their visuals through `units::Fx`; an
  item's own hits carry `Hit::proc` and set nothing more off. Guests see items through
  `Vitals::run_items` on `Game::Me` alone.
  Chests and elites carry a `treasure::FieldMark` on every machine (the host sets it; a guest
  reads it off `Role::Chest`/`Role::Elite`), and the posts reach guests as `Run::posts` on
  `Game::Run`: what shows the field (`marks.rs`, the minimap) reads only those, never `Field`.
  The shrines (`data::SHRINES`) are the host's too: a hero standing in one charges it, and the
  state reaches guests as `Run::shrines` (with `Run::greed` and `Run::curse`) on `Game::Run`. A
  Shrine of Power offers the hero in it three blessings (`Vitals::shrine_offer`, shown on the
  level-up cards); a guest answers with `Game::ShrineChoice`, which `treasure::take_boon` takes
  only for a blessing of that offer. What shrines give stays on `Vitals::boons`, counted in
  `Vitals::stats`; Greed and Curses toughen the foes through `Run::threat`, which every spawn
  reads instead of `Session::rules` alone.
  Elites drop keys (`data::KEY_ODDS`, on `Vitals::keys`): a key opens a chest for nothing and
  leaves its price. The merchant and the jars and crates are `market.rs`'s, in `SimSet`: the
  merchant is a unit (`FieldMark::Merchant`, `Role::Merchant`), its wares `Run::shop` on
  `Game::Run`. A purchase goes through `market::BuyQueue` (the local player's own, or a guest's
  `Game::Buy`), and `market::buy` takes it only for a hero at the merchant with the gold, for a
  ware still there, within the five kinds.
- **Reward points are derived, perks are stored.** Points earned are counted from the
  achievements done (`Profile::points_earned`), never stored; the profile keeps only the perk
  ranks bought (`Profile::perks`) and the resets used. A perk's bonus rides
  `Profile::meta_bonuses`; its other ranks ride `Brings::perks` and the lobby `Loadout`, and land
  on the hero through `rewards::apply_perks` (the host's for a guest, through `seat_perks`,
  each rank held to its perk's most). A race opened is stored by name (`Profile::races`) and
  only adds looks: `menu::all_looks` gives every class a look of it in both sexes, and
  `outfit::body` dresses a pair vanilla never had in the class's outfit from another race. The
  lobby `Loadout` already carries class and race, so it needs nothing on the wire. A tabard
  bought is stored by key (`Profile::tabards`), one earned by an achievement is derived
  (`Profile::tabard_owned`); the one worn (`Profile::tabard`) is only a look: `outfit::body`
  puts its real 1.12.1 display in the tabard slot, and it reaches the host as `Brings::tabard`
  and the lobby `Loadout::tabard`. Appearances go the same way: bought by key
  (`Profile::appearances`), worn one a slot (`Profile::looks`), only a look laid over the gear's
  (`profile::cosmetics`, a display of 0 baring the slot) and carried as `Brings::looks` and
  `Loadout::looks`; the gear's numbers stay. A companion bought is stored by key
  (`Profile::pets`), the one at the hero's side as `Profile::pet` and its instinct as
  `Profile::instinct`, both carried as `Brings::pet`/`instinct` and `Loadout::pet`/`instinct`;
  its training is perks (`data::PET_TRAINING`, found by `data::perk`), so it is stored, reset
  and carried with them, and `data::pet_stats` reads it. A companion is a plain unit
  (`Role::Totem` on the wire) that only marks pickups with `Gem::send_to`; the hero's own
  magnet brings them in.
- **Legion's looks are a machine's own.** A creature with a Legion look (`EnemyDef::legion`, a
  Legion display; `Foe::remade` for Legion's remake under the same id) takes it only where
  `data::legion_looks()` (set by `entities::legion` once Legion's displays are in the catalog):
  `EnemyDef::look` at the spawn. A guest maps a Legion display it cannot draw back to the
  creature's 1.12.1 one (`data::shown_display`, in `net/client.rs`), and anything that finds a
  creature by its display uses `EnemyDef::wears`. A zone's bonus boss (`data::bonus_boss`,
  Illidan in Warsong Gulch) stands only where the host draws Legion's creatures, at a post of
  the elite kind `data::BONUS` after `data::ELITES` (`data::elite_kind` reads either), falls
  once and drops its piece (`data::BONUS_ITEMS`: gear beyond the sixty, found by `data::item`
  and listed by `data::gear_items`, never in the collection); the balance simulator, without
  the data, never meets it. The main screen names him and his piece (the zone's row, Drop
  Tables, the Codex) only where this machine draws him (`data::shown_bonus_boss`), and the Codex
  records him from `Run::bonus_slain`. What the main screen counts (the collection's pieces, the
  Codex's creatures and spells) is what this machine shows: `data::shown_items`,
  `data::all_creatures`, `data::ability_shown`. Legion's weapon looks are item displays from
  `data::LEGION_ITEM_FIRST` (a worn entry carries sixteen bits), the glaive's first, then
  `data::LEGION_WEAPON_FAMILIES`' in order, each with a 1.12.1 icon of its family so gear's
  pools draw it (`entities::legion`); a piece with `ItemDef::look` wears that one where the
  catalog has it, and an entry whose look the catalog lacks takes its family's first 1.12.1
  one (`outfit::legion_fallback`). Legion's races (`data::LEGION_RACES`: the Blood Elf, the
  Draenei, the Goblin, the Worgen and the Pandaren, whose 1024×512 skins `benilla_legion::layout`
  puts on 1.12.1's atlas) join `data::playable_races` only where `entities::legion` put them into the client's
  character tables (`data::legion_race`); they are stored by name (`data::race_name`), never
  opened in the Rewards tab, wear no helm, and a guest without them draws their stand-in
  (`data::stand_in`, `data::stand_in_body`, in `net/client.rs`). Legion's classes
  (`data::LEGION_CLASSES`: the Death Knight, the Monk and the Demon
  Hunter) are in `data::CLASSES` everywhere, but offered only
  where `entities::legion` read their spells (`data::class_shown`). Their abilities' `spell` is
  Legion's (its icon and name, `data::legion_spell`, which `hud::fill_icons` and
  `hud::spell_name` read first) and their `visual` the 1.12.1 spell they play
  (`AbilityDef::visual`, `Mobility::visual`): a guest needs nothing of Legion's to see them.
  Their talent trees are their specializations, added to the client's talent catalog
  (`entities::legion`, tab and talent ids from `TALENT_BASE`); they wear a 1.12.1 class's
  starting outfit (`data::outfit_class`) and borrow its trainers' looks, held to their races
  (`data::race_classes`, in `menu::all_looks`).
- **The level-up offer is the simulation's.** Solo and the host deal each hero's cards
  (`offer::deal_offers`, onto `Vitals::offer`); a guest sees them on `Game::Me` and answers with
  `Game::Card`, which `offer::apply_action` takes only for a card of that offer at the rarity it
  was dealt, or a reroll, banish or skip the hero holds. The HUD (`hud::Offer`) only follows
  `Run::offer`; it never deals. The cards are spells only, four of them; a spell's ranks have no
  cap. A card grows stats of its spell (`data::Perk`, drawn by `offer::draw_perks` for the
  stats the spell has: one, two for an Epic, three for a Legendary, the rarer the more), summed
  on `Owned::perks`, which a spell's damage, cooldown, targets, area and duration read; a new
  spell's card brings one perk from Uncommon up. A card is taken only with the perks it was
  dealt. A skip is always there, for `data::SKIP_GOLD`.
- **The damage meter counts what the simulation credits.** Every `abilities::Hit` names the
  spell it is credited to (`Hit::spell`, 0 for none, shown as Other), and `abilities::credit`
  adds it to the hero's `Vitals::damage_by`, which reaches guests on `Game::Me`. A target's
  health reaches guests as a percent in its health field (`units::set_health`, 1..100 while it
  lives; 0 is `kill_unit`'s).
- **A balance change is measured.** A number that changes a run's pace (the horde, its waves,
  the bosses, the cards' perks, the hero's stats) is checked with the balance simulator
  (`balance.rs`) before and after, and the table goes in the PR. A stage's showdown (from the
  boss portal's opening to the boss's fall: `enemies::spawn_enemies`, `data::SHOWDOWN_CROWD`)
  thins the crowd and brings no ring and no surge; an ambush ring always leaves its openings
  (`data::ring_angles`); a surge comes from one side (`data::SURGE_ARC`).
- **Shown text goes through `lang::tr`.** A new label on the main screen or the HUD is written
  in English inside `tr(…)`, and its French added to `lang.rs`'s table; a name the profile stores
  (zones, items, classes) stays English in data and is translated only where it is shown.
- **The wire is versioned.** Any change to `survivors-net`'s `protocol` or `game` messages bumps
  `PROTOCOL_VERSION`. `ZONE_COUNT` must equal `data::MAPS.len()` (a test holds them).

## Pitfalls

- **Clicks need a `ButtonLook`.** `kit::KitClicks` only reads buttons that carry
  `kit::ButtonLook`; a bare `Button` gets an `Interaction` (hover, drag, tooltip) but never a click.
- **The menu's order is fixed.** `character.rs`'s chain runs after `hud::show_panels`, which runs
  after `WorldStage::Input`, where `hero::place_camera` sits. Ordering a menu system before the
  camera makes a cycle, and Bevy panics at startup, not at compile time.
- **The menu's hero is the live one.** It shows through the portrait and the Armory's stage,
  which have no fill; a background on them, or on a node around them, hides it.
- **A view rebuilds on a key.** Each `refresh_*` system keeps what it last drew in a `Local` and
  rebuilds only when that changes: a new input to a view goes into that key, or it goes stale.

## Working on a change

- One change per pull request, after `.github/pull_request_template.md`: the issue, what changes,
  what 1.12.1 does and how you know, how you checked it.
- Before pushing: `cargo test -p benilla-app --lib survivors` and, for multiplayer,
  `cargo test -p survivors-net -p survivors-server`; `rustfmt --check` on the files you changed.
- Update this file and the README when a file, a switch, a rule or something a player sees
  changes; `CONTROLS.md` when a key does.

## Where to look when

| symptom | look at |
|---|---|
| the hero sits outside its portrait, the Armory's stage or the sheet's | `hero::stage_eye` frames it into `hero::MenuStage`, which `hero::frame_stage` fills from the `StageFrame` on screen (at the menu, or over a paused run); a fill on that frame hides it |
| the wasm panics at boot with `RuntimeError: unreachable` | rebuild with `WEB_DEBUG=1`; check for a WebGPU adapter first; `?bridge=0` rules the bridge out |
| `survivors: no ability "…"` at boot | a class's `start`/`pool` names a key missing from `ABILITIES`; `cargo test -p benilla-app --lib survivors::data` |
| wasm-bindgen glue errors at load | version drift: rerun `scripts/web-setup.sh` |
| an upstream merge broke only the wasm build | `cargo check --target wasm32…`; the `cvars.rs` struct-vs-tuple case is the pattern |
| a guest sees nothing move, or units pile up | `net/host.rs`'s `stream` and `net/client.rs`'s `receive`; `GET /lobby/health` for the server |
| a guest's copy drifts from the host's (a unit where it came in, one missing) | `net/mod.rs`'s `the_guest_converges_through_a_bad_link` plays the case; a message for a copy spawned the same frame goes through the command queue (`client::receive`), never a query that cannot see it yet |
| other players move in jerks | `net/smooth.rs`: copies are drawn `DELAY` behind the sender's clock (`Game::Motions::t` from the host, `Game::Input::t` from a guest); a `DELAY` under the sender's tick, or a position written straight to a `Transform`, brings the jerks back |
| `make build` fails at the download | the archive.org item moved: `make build WOW_DATA_URL=…`, see the README |
| no sound in the browser | the page resumes the AudioContext on the first gesture; the per-target mixer backend is the carry in `sound/mixer.rs` |
| short stalls in the browser on the first play of a sound | the SFX carry (`sound/kit/deferred.rs`, the glue and cinematic arms of `sound/glue.rs`/`sound/cinematic.rs`), wasm-only |
