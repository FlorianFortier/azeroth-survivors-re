# Working in this repository

*Azeroth Survivors* is a local, LAN build of Aethus' fan-made WoW roguelike, running on
[wenilla](https://github.com/Arnesen/wenilla) — the WebAssembly build of
[samwhosung/benilla](https://github.com/samwhosung/benilla), a from-scratch WoW 1.12.1 client in
Rust + Bevy. There is no server, no login and no realm: the game boots straight into the
survivors mode, natively or in the browser. This file is the map for a person or an agent who
has to build, fix or change something here. Upstream's own map and rules for the client crates
are `docs/METHOD.md`, `docs/MAP.md` and `docs/CONTRIBUTING.md`; read them before changing client
code. Syncing upstream is [docs/UPSTREAM.md](docs/UPSTREAM.md).

## What lives where

| path | what | whose |
|---|---|---|
| `crates/benilla-app/src/survivors/` | **the game**: menu, hero, enemies, abilities, level-up cards, HUD | ours |
| `crates/benilla-app/src/survivors/data.rs` | the game's tables and balance curves (classes, abilities, passives, zones, waves) | ours |
| `crates/benilla-*` | the client: formats, world, UI, protocol, app | upstream's, plus our `#[cfg]`-gated carries |
| `crates/benilla-app/src/webbridge/` | the JavaScript bridge, wasm side | wenilla's |
| `crates/benilla-app/src/survivors/character.rs` | the main screen (title, tabs, Character tab: class column, hero column with the hero's portrait, zone column, bottom bar; the Armory: race, look and gear around the hero on its stage), built from the kit on the same `Selection` and profile as the glue menu (`menu.rs`), which it replaces; `WOW_NEW_MENU=0` (`?new_menu=0`) brings the glue menu back | ours |
| `crates/benilla-app/src/survivors/outfit.rs` | the hero as a player character: the race's body, the five appearance dials, and the pieces it wears, dressed from item entries the mode answers itself (`outfit::template`); `WOW_HERO_BODY=trainer` brings back the class trainer's look | ours |
| `crates/benilla-app/src/survivors/talents.rs` | talents: the class's three trees from `Talent.dbc`/`TalentTab.dbc` under vanilla's rules, each talent a stat of its tree's theme per rank; points trained with gold (`point_cost`), 51 at most | ours |
| `crates/benilla-app/src/survivors/lang.rs` | the mode's languages: text is written in English and looked up for French (`lang::tr`, `trf` for `{}` templates); the profile's `options.language` picks it, and a change rebuilds the main screen | ours |
| `crates/benilla-app/src/survivors/names.rs` | hero names: a random one in each race's sound, and what a typed one may hold | ours |
| `crates/benilla-app/src/survivors/kit.rs` | the menu's UI kit: panels, headings, tabs, icon and call-to-action buttons, stat tiles, progress bars, chips, tooltips; clicks read through `KitClicks`. `WOW_UI_KIT=1` (`?ui_kit=1`) shows a gallery over the menu | ours |
| `crates/benilla-app/src/survivors/profile.rs` | the saved profile (`benilla-config/survivors-profile.toml`, `localStorage` on the web): pick, run records, and the meta-progression as it lands | ours |
| `crates/benilla-app/src/survivors/net/` | multiplayer, game side: socket, host stream, guest copy, lobby screen | ours |
| `crates/survivors-net` | multiplayer protocol, lobby settings, the lobby hub's state machine. No I/O, no deps | ours |
| `crates/survivors-server` | the lobby server (`make lobby`); its router is mounted by `wenilla-host` at `/lobby` | ours |
| `crates/warcraft-survivors` | the native launcher (`make native`) | ours |
| `crates/wenilla` | the wasm entry crate (`#[wasm_bindgen(start)]`), boots the survivors mode | ours |
| `crates/wenilla-host` | local dev server: static files, `/data/*`, and the lobby at `/lobby`. Loopback by default. | wenilla's |
| `web/` | the page side: `index.html`, `boot.js`, `platform.js`, `bridge.js`, `examples/`, `wasi_stubs.js` | wenilla's |
| `scripts/fetch-data.sh` | downloads the 1.12.1 client and extracts its MPQs into `wow-data/` | ours |
| `scripts/web-setup.sh`, `scripts/web-build.sh` | toolchain and build for the wasm | wenilla's |
| `third_party/` | patched `lua-src`, `mlua-sys` and `kira`; each has a `BENILLA.md` | vendored |
| `.github/workflows/check.yml` | the PR gate: wasm check, host and UI tests, client-carry and survivors-table tests | ours |
| `.github/workflows/ci.yml` | the gates of `scripts/gates.sh` that need neither game data nor a display | — |

`crates/wenilla-host` also carries a `/ws/*` relay to a mangos realmd/worldd. The survivors mode
never opens it; it is inherited from wenilla.

## Commands

```bash
make build                  # fetch-data + web-setup + web-build (~5.5 GB download the first time)
make start                  # http://127.0.0.1:8090/ — the browser build
make native                 # the native desktop build
scripts/web-build.sh        # → web/dist (WebGPU build). WEB_DEBUG=1 keeps symbols for readable stack traces.
cargo check --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
                            # the wasm build without linking; what check.yml runs
cargo check --workspace --all-targets      # native (Debian/Ubuntu: libasound2-dev libudev-dev)
cargo test -p benilla-app --lib survivors  # tables, balance, cards, host→guest replication; no game data
cargo test -p survivors-net -p survivors-server  # multiplayer protocol, lobby rules, server end to end
make lobby                  # a standalone lobby server on :7777
cargo test --workspace                     # ~320 tests need a 1.12.1 patch chain and fail without one
```

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
  `data::BOSS_TIMES` slot, the last its final boss: the run is won when that one falls
  (`Run::final_slain`), never on the clock. A zone opens when the one before it has a win on the
  profile (`Profile::zone_open`). A zone renamed keeps its records through
  `profile::RENAMED_ZONES`.
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
- **Shown text goes through `lang::tr`.** A new label on the main screen or the HUD is written
  in English inside `tr(…)`, and its French added to `lang.rs`'s table; a name the profile stores
  (zones, items, classes) stays English in data and is translated only where it is shown.
- **The wire is versioned.** Any change to `survivors-net`'s `protocol` or `game` messages bumps
  `PROTOCOL_VERSION`. `ZONE_COUNT` must equal `data::MAPS.len()` (a test holds them).

## Where to look when

| symptom | look at |
|---|---|
| the hero sits outside its portrait or the Armory's stage | `hero::stage_eye` frames it into `hero::MenuStage`, which `character.rs`'s `frame_stage` fills from the frame on screen; a fill on that frame hides it |
| the wasm panics at boot with `RuntimeError: unreachable` | rebuild with `WEB_DEBUG=1`; check for a WebGPU adapter first; `?bridge=0` rules the bridge out |
| `survivors: no ability "…"` at boot | a class's `start`/`pool` names a key missing from `ABILITIES`; `cargo test -p benilla-app --lib survivors::data` |
| wasm-bindgen glue errors at load | version drift: rerun `scripts/web-setup.sh` |
| an upstream merge broke only the wasm build | `cargo check --target wasm32…`; the `cvars.rs` struct-vs-tuple case is the pattern |
| a guest sees nothing move, or units pile up | `net/host.rs`'s `stream` and `net/client.rs`'s `receive`; `GET /lobby/health` for the server |
| `make build` fails at the download | the archive.org item moved: `make build WOW_DATA_URL=…`, see the README |
| no sound in the browser | the page resumes the AudioContext on the first gesture; the per-target mixer backend is the carry in `sound/mixer.rs` |
| short stalls in the browser on the first play of a sound | the SFX carry (`sound/kit/deferred.rs`, the glue and cinematic arms of `sound/glue.rs`/`sound/cinematic.rs`), wasm-only |
