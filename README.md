<img width="1920" height="1080" alt="brag" src="https://github.com/user-attachments/assets/fa574bb6-7bd4-4e78-8b72-14c58e3c716e" />

<div align="center">

# Azeroth Survivors

**A free fan-made WoW roguelike by [Aethus](https://wow.aethus.pl/), run locally on a
from-scratch WoW 1.12.1 client engine (Rust + [Bevy](https://bevy.org)).**

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue.svg)](LICENSE-MIT)

</div>

> **Not an original project.** *Azeroth Survivors* is a free, fan-made WoW roguelike by
> **Aethus** — play the original in the browser at **[wow.aethus.pl](https://wow.aethus.pl/)**
> ([r/wowservers announcement](https://www.reddit.com/r/wowservers/comments/1x0yidl/azeroth_survivors_roguelike_wow_singleplayer_game/)
> · [Discord](https://discord.gg/UzxaZuSAk)). This repository is a from-source local build of the
> game, running on the [benilla](#how-its-built) / [wenilla](#how-its-built) engine, for personal use.

Pick a class and a battleground, then survive: WASD to move, your abilities cast themselves,
enemies swarm in waves, you level up and pick upgrades, bosses come at 3, 6, 9 and 12 minutes,
and the zone's final boss at 16:00: bring it down to win. **No server, no login, no account** — fully offline and self-contained.

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/menu.jpg" alt="The pick screen: race, gender and class on the left, the zones on the right, the hero in between"></td>
    <td width="50%"><img src="docs/screenshots/combat.jpg" alt="A Mage holding off Elwynn Forest's wolves with frost, fire and arcane"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/level-up.jpg" alt="Level up: three spell cards, coloured by rarity, each showing exactly what its next rank changes"></td>
    <td><img src="docs/screenshots/results.jpg" alt="The results screen after a death in Redridge Mountains"></td>
  </tr>
</table>

### In this build

- **A Megabonk-style field**: kills drop the run's gold; six chests stand around the battleground
  and open for a hero who stands at one with the gold (20, then 40% dearer each time) for a run
  item, a spell's rank or a roll for gear. Four elites hold fixed posts, each over a reward of its
  own (gear, a spell's rank, a run item, a purse), and come back a while after they fall. Run
  items stack for the run; half the gold left unspent goes to your profile. A ring on the ground
  and a label over each chest give its price, each elite shows what it guards, and a fallen
  elite's post counts down to its return; the minimap marks them all, the far elites at its
  edge. Co-op guests see the same.
- **A main screen in tabs** (Character, Collection, Drop Tables, Codex, Talents, Achievements,
  Rewards). The Character tab shows the class on the left (its health, starting spell and spell
  pool), your hero in the middle (portrait, name, race, equipment, collection, racials) and the
  zones on the right, with Run Options, Enter and Co-op under them. The older pick screen built
  from the client's glue art is still there with `?new_menu=0` (`WOW_NEW_MENU=0` native).
- **The Armory**, opened from the hero's portrait, the Armory button or an equipment slot: eight
  races, both genders and the ten slots on the left, the hero on a stage you turn by dragging in
  the middle, and character creation's appearance options and the pieces you own for the slot
  picked on the right.
- **A twelve-zone campaign**, from Elwynn Forest's Defias Uprising to Naxxramas: Duskwood, Warsong
  Gulch, Un'Goro Crater, Arathi Basin, Feralas, Gurubashi Arena, Scarlet Monastery, Alterac
  Valley, Blackrock Mountain and the Gates of Ahn'Qiraj between. Each zone fights with its own
  creatures and five bosses, and opens once the final boss of the one before has fallen
  (`WOW_UNLOCK_ALL=1` or `?unlock_all=1` opens them all). Your pick, every run's record and the
  zones won are kept in a saved profile (`benilla-config/survivors-profile.toml`, or the
  browser's storage).
- **Your hero is a player character**: the race's own body with character creation's skin, face,
  hair, hair colour and facial hair (set in the Armory), dressed piece by piece in the class's
  starting outfit and its weapons, which the Equipment panel lists.
- **Loot that lasts**: each of the 60 bosses drops its own piece of gear (uncommon to legendary),
  and the horde now and then one of its zone's. Whatever you catch is kept, won or lost; wear it
  from the Armory (a click on a slot opens it there), and it shows on your hero and adds to your
  numbers.
- **A collection of all 60 pieces** (the **Collection** tab, and its bar under the equipment):
  every piece found adds a quarter of its first bonus to every run from then on, worn or not.
- **Gold and talents**: every run earns gold (kills, bosses reached, the win). Gold trains talent
  points, up to vanilla's 51, shared by every class; the **Talents** tab lays out the class's
  three real trees from the client's `Talent.dbc` (names, icons, tiers, prerequisites), each
  talent giving a stat of its tree's theme per rank.
- **Run Options**: Normal, Heroic and Mythic, each opened per zone by a win at the one below,
  tougher and better paid. **R** rerolls the level-up cards (three a run), and both mouse buttons
  run forward as in WoW.
- **Drop Tables and a Codex**: every zone's five pieces with who drops them and the horde's odds,
  and an encyclopedia of the horde, the bosses (with their times) and the spells your runs have
  met.
- **Achievements and Rewards**: thirty achievements written for the mode (runs, wins, kills,
  levels, the collection, bosses met, talents, every zone, Heroic and Mythic), each paying gold
  once claimed from the **Rewards (N)** tab.
- **Options and French**: the **Options** page (bottom left) sets the master, music and effects
  volumes, the mouse sensitivity, the picture polish and the language, English or French, and
  lists the controls. The run's headings follow the language from the next start.
- **Three racials per race**, after vanilla's: Troll Berserking every 35 s, Undead Will of the
  Forsaken, Tauren Endurance, Gnome Expansive Mind… Each is a stat bonus held all run or an
  effect that fires on its own, shown on the main screen and carried by co-op guests too.
- **Level-up cards** that deal in, tilt toward the cursor and lift on hover. Their WoW item-quality
  colour says how strong a pick is, and each card lists exactly what changes, like
  `Damage 20 » 26` or `Targets 2 » 3`.
- **A class movement spell on F** from level 5, aimed at the pointer: Blink, Charge, Sprint, Dash,
  Fade, Ghost Wolf, Aspect of the Cheetah, Blessing of Freedom, Howl of Terror.
- **A third-person 3D view with WoW's controls:** the pointer stays free; a left-drag swings the
  camera, a right-drag turns the hero, both buttons run; A/D turn and Q/E strafe; a click on an
  enemy (or **Tab**) targets it, and your single-target spells strike it first. **C** switches to
  the classic top-down views, locked or free.
- **WoW's movement:** no inertia on the ground, backpedalling at the stock 4.5 against 7 yards a
  second, strafing at full speed, and a running jump that keeps its momentum to the landing (a
  standing one may be steered once, at walk speed).
- A light sharpen-and-colour pass over the picture (**G** toggles it).

## Play it

```bash
git clone https://github.com/FlorianFortier/azeroth-survivors-re
cd azeroth-survivors-re
make build     # fetch game data + install toolchain + build the wasm (~5.5 GB download)
make start     # → open http://127.0.0.1:8090/
```

`make build` chains three re-runnable scripts, each skipping what's already done:

| Step | What it does |
| --- | --- |
| `scripts/fetch-data.sh` | downloads the WoW 1.12.1 client and extracts its MPQs |
| `scripts/web-setup.sh` | installs the wasm toolchain (wasi-sdk, wasm-bindgen, binaryen) |
| `scripts/web-build.sh` | builds the client into `web/dist/` |

**Prerequisites:** [Rust](https://rustup.rs/), `make`, `curl`, `unzip`, and a bash shell. On Windows, run everything from Git Bash; PowerShell and cmd can't run the scripts.

Run `make` (or `make help`) to list every target.

## Multiplayer

Up to four players can run a battleground together: one hosts a lobby, the others join with its
five-character code. The host sets the zone, the seats, the difficulty (presets, or enemy health,
damage and crowd size one by one), the run's length, how often bosses come, shared experience,
revives, and whether the lobby is listed publicly.

Each hero brings its own profile into the run: the gear it wears, its collection, its class's
talents and its race's racials.

`make start` already serves a lobby beside the game: players on your network open your page,
click **Co-op**, and create or join. To host lobbies on the internet without exposing game
files, run the standalone server with `make lobby`. See [docs/MULTIPLAYER.md](docs/MULTIPLAYER.md).

## Controls

See [CONTROLS.md](CONTROLS.md). The short version, as in WoW: drag with the **left button** to
look, the **right button** to turn, both to run; **W/S** to run, **A/D** to turn, **Q/E** to
strafe; **click** or **Tab** to target; **Space** to jump, **F** for your class's movement spell,
the **mouse wheel** to zoom, **C** to change the view, **Esc** to clear the target or pause,
**1/2/3** to pick a level-up card.

## Options

Add these to the page's address (`http://127.0.0.1:8090/?unlock_all=1`), or set the `WOW_…`
variable for the native build:

| browser | native | what |
| --- | --- | --- |
| `?unlock_all=1` | `WOW_UNLOCK_ALL=1` | open every zone |
| `?new_menu=0` | `WOW_NEW_MENU=0` | the older glue-art pick screen |
| `?grade=0` | `WOW_GRADE=0` | start with the picture polish off |
| `?mouse_sens=1.5` | `WOW_MOUSE_SENS=1.5` | mouse speed in the third-person view, 0.1 to 10 |
| `?name=…` | `WOW_NAME=…` | your name in a lobby |
| `?lobby=…` | `WOW_LOBBY=…` | the lobby server to use |

## The game data (not in this repo)

This repo ships **no Blizzard content** — no art, models, sounds, maps, MPQ contents, or
FrameXML. The game reads a WoW 1.12.1 client's `Data` folder at runtime, which you supply.

`make build` (via `scripts/fetch-data.sh`) downloads the **English WoW 1.12.1 (build 5875)**
client from [archive.org](https://archive.org/details/WoW_1.12.1-5875) (~5.5 GB) and extracts the
MPQs into `wow-data/Data/` (git-ignored, resumable, skipped when already present). Or point at a
client you already have with `--data` / `WOW_DATA`.

### Changing the download source

If the archive.org item moves or you have your own mirror, override the URL without editing
anything:

```bash
make build WOW_DATA_URL=https://your-mirror.example/client.zip
```

Or set it permanently by editing the single clearly-marked `DEFAULT_DATA_URL` line at the top of
`scripts/fetch-data.sh`. Either way the URL must be a direct link to a zip holding the client's
`Data/*.MPQ` files.

## How it's built

The survivors mode is `crates/benilla-app/src/survivors/`, running on two upstream projects:

- [benilla](https://github.com/samwhosung/benilla) — a from-scratch WoW 1.12.1 client.
- [wenilla](https://github.com/Arnesen/wenilla) — benilla compiled to WebAssembly, with a local
  host and realm service.

To work on it, start with [AGENTS.md](AGENTS.md): where everything lives, file by file, the
commands and tests (most need no game data), the rules every change keeps, and the known
pitfalls. Multiplayer is [docs/MULTIPLAYER.md](docs/MULTIPLAYER.md), syncing upstream
[docs/UPSTREAM.md](docs/UPSTREAM.md).

## Legal

*Azeroth Survivors* is a free fan game by **Aethus**, not affiliated with or endorsed by
Blizzard Entertainment, and it ships no Blizzard game files. The screenshots in `docs/screenshots/`
show World of Warcraft art rendered from a user-supplied client; that art is © Blizzard
Entertainment. World of Warcraft is a trademark of Blizzard Entertainment, Inc. The engine code in
this repository is licensed under
[MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option; the vendored components under
`third_party/` keep their own upstream licenses.
