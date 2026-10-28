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
enemies swarm in waves, you level up and pick upgrades. A run is three stages, each a zone of the
campaign from the one you picked on: bosses come on the stage's clock, then a boss portal opens
for its final boss; bring it down for gold, a key and a blessing, step through the portal onward
to the next, tougher zone, and after the third hold out against the final swarm to win. **No server, no login, no account** — fully offline and self-contained.

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
  and open at once for a hero who walks up to one with the gold (20, then 40% dearer each time),
  offering three run items to choose from. Four elites hold fixed posts, each over a reward of its
  own (gear, a spell's rank, a run item, a purse), and come back a while after they fall. Run
  items stack for the run, five kinds at most (the neck, rings and trinkets of the character
  sheet: once five are held, a chest offers more of them), Common to Legendary (the rarer,
  the less often drawn), shown with
  their stacks under your talents; besides a stat, some do more: a vampiric fang heals off your
  damage, a storm charm arcs lightning, thornmail strikes back, a volatile ember bursts on kills,
  a soul shard heals on them; a brand sets foes burning, a locket chills them, a headsman's axe
  finishes the weakened, a shadowstep charm dodges blows, a sigil shields you on kills, a
  reliquary and a frost wyrm's scale pulse novas around you, and a goblin's purse pays out. Half the gold left unspent goes to your profile. A ring on the ground
  and a label over each chest give its price, each elite shows what it guards, and a fallen
  elite's post counts down to its return; the minimap marks them all, the far elites at its
  edge. Co-op guests see the same.
- **A WoW HUD**: your unit frame (portrait, name, health, power, level) and your target's at the
  top left, the clock with the time to the next boss (then to the boss portal) under it, a
  Damage Done meter by spell under the minimap, and your spells on the action bar with its
  gryphons at the bottom, the movement spell on its last slot and the experience bar on top.
  Gold, kills, the chest's price and keys stay spelled out under the clock. Co-op guests see
  their own damage and their target's health too.
- **Quicker levels, snappier pickups**: the first levels come faster, so a run opens on choices;
  gems fly to you faster once your pickup range catches them, and when a stage's boss falls every
  gem left on the field comes in at once.
- **The horde comes in waves**: every 45 seconds of a stage it surges for 12, a third again as
  thick and in packs from one side, then thins out for a breather; the line under the clock
  counts down to the next wave. The crowd is thicker than before, and never past what the frame
  can carry.
- **Balanced by simulation**: whole runs are played headless by a simple pilot for every class,
  and the numbers are tuned against what it meets: an ambush ring always leaves two openings, a
  wave comes from one side, the horde thins while you face a stage's boss, and the bosses are
  set to a stage's strength.
- **Hits you feel**: every blow shoves its foe back (the heavier the hit, the further; elites
  and bosses hardly budge), a struck model brightens for a moment, and the camera shakes at a
  heavy blow to you, a run of critical strikes and a boss's coming (Options turns the shake off).
  Co-op guests see and feel the same.
- **A shady merchant** stands somewhere on the field with three run items for the run's gold
  (Common to Legendary, dearer with each bought, restocked every two minutes): walk up to it and
  click a ware or press 1, 2 or 3. Elites sometimes drop a **key**, which opens a chest for
  nothing, and **jars and crates** about the field break as you walk into them, for a few coins
  or a bite to eat. Co-op guests buy, break and open too, the host checking each.
- **Shrines** across the field, small stones with a ring around them: stand within the wide ring
  of a **Shrine of Power** until it charges and choose one
  blessing of three (a stat for the rest of the run, drawn Common to Legendary); wake the
  **Shrine of Challenge** and beat the elites it calls for a free chest; the **Shrine of Greed**
  gives every hero more gold and the foes more bite, the **Shrine of Curses** more experience
  and more foes; those three have a narrow ring, to be woken on purpose. Each shows its ring, its charge and what it does over the world and on the
  minimap, and co-op guests see and use them too.
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
  browser's storage). With Legion's data (below), a thirteenth zone opens after Naxxramas:
  **Val'sharah**, the Emerald Nightmare on the Broken Isles, Legion's own terrain, forest,
  buildings, water and light, its horde the Nightmare's (satyrs, dryads, ents, ancients,
  wisps…) and its bosses Archdruid Glaidalis, Oakheart, Dresaron, the Shade of Xavius and Xavius.
  It is played solo for now, and a run there stays there.
- **Legion's demons**, natively, with a Legion 7.3.5 install (`WOW_LEGION=<folder>` and
  `WOW_LEGION_DBD=<WoWDBDefs definitions>`): Warsong Gulch's felhounds, felguards and doomguards
  wear Legion's models, and **Illidan Stormrage** waits on the field as a bonus boss, an
  optional fight once a run for his **Warglaive of Azzinoth** (a legendary main hand, kept and
  worn like the rest, in his Legion look), a spell rank, a run item and a boss's gold. Legion's
  weapons join the looks your gear's weapons draw from, and the **Blood Elf**, the **Draenei**,
  the **Goblin** and the **Worgen** are playable, in their pre-HD bodies, for the classes they
  were in The Burning Crusade and Cataclysm. A machine without the data sees the 1.12.1 looks (a
  Human for a Blood Elf or a Worgen, a Night Elf for a Draenei, a Gnome for a Goblin). Legion's
  classes are playable too: the **Death Knight** for every race (Obliterate, Howling Blast, Death
  Coil…, Death's Advance on `F`), the **Monk** for all but the Goblin and the Worgen (Tiger Palm,
  Rising Sun Kick, Fists of Fury…, Roll on `F`) and the **Demon Hunter** for Night Elves and
  Blood Elves, a Warglaive of Azzinoth in each hand (Chaos Strike, Blade Dance, Eye Beam…, Fel
  Rush on `F`). Their spells show Legion's icons and names and play 1.12.1's visuals, and their
  specializations are talent trees of Legion's talents; in a lobby the bonus boss is there when the host has the data.
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
  tougher and better paid. Both mouse buttons run forward as in WoW.
- **Drop Tables and a Codex**: every zone's five pieces with who drops them and the horde's odds,
  and an encyclopedia of the horde, the bosses (with their times) and the spells your runs have
  met.
- **Achievements and Rewards**: thirty achievements written for the mode (runs, wins, kills,
  levels, the collection, bosses met, talents, every zone, Heroic and Mythic), each paying gold
  once claimed from the **Achievements (N)** tab, and reward points once done. The **Rewards
  (N)** tab spends the points on perks every hero of every class brings into a run: more
  rerolls and banishes, more gold for a skipped hand, a fuller first hand, more experience and gold, gold in hand at the
  start; and on races, each opened to every class for 100 points (an Orc paladin, a Tauren
  mage), kept through resets; and on 1.12.1's own tabards (Stormwind, the Argent Dawn, the
  Scarlet Crusade, the battlegrounds', Flame and Frost), some bought, some earned by an
  achievement, one worn over the gear; and on appearances for the head and back, 1.12.1's own
  (the Lucky Fishing Hat, the Goblin Rocket Helmet, the Helm of Wrath, the Onyxia Scale Cloak,
  Sapphiron's drape…), worn over the gear's look with its numbers kept, or the helm and cloak
  hidden; and on a companion, one of 1.12.1's vanity pets (a Prairie Dog, a Worg Pup, a
  Zergling, Murky…) that runs at the hero's side and fetches the wisps, coins and keys within its
  leash, with an instinct to pick (everything, gold first, wisps first) and training to buy (a
  longer leash, quicker paws, a keener nose). All are bought on a draft kept with **Confirm**;
  **Reset** takes every perk point back, twice a save.
- **Options and French**: the **Options** page (bottom left) sets the master, music and effects
  volumes, the mouse sensitivity, the picture polish, the screen shake and the language, English or French, and
  lists the controls. The run's headings follow the language from the next start.
- **Three racials per race**, after vanilla's: Troll Berserking every 35 s, Undead Will of the
  Forsaken, Tauren Endurance, Gnome Expansive Mind… Each is a stat bonus held all run or an
  effect that fires on its own, shown on the main screen and carried by co-op guests too.
- **Level-up cards, Megabonk-style**: four spell cards that deal in, tilt toward the cursor and
  lift on hover, each drawn at a rarity, Common to Legendary in WoW's item-quality colours. A new
  spell's card shows what the spell does and, from Uncommon up, a bonus; a known spell's card
  grows one of its stats (`+25% damage`, `-10% cooldown`, `+1 targets`, area, duration), an Epic
  two and a Legendary three, the rarer the more. Ranks have no cap, so no two builds scale alike.
  Under each card its **Banish** (never dealt again that run), and under them **Skip** (+10 gold,
  always there) and **Reroll** (R): two rerolls and three banishes to start, and one more of them,
  in turn, every five levels. In co-op the host deals every hero's cards and takes only a pick of
  what it dealt.
- **A class movement spell on F** from level 5, aimed at the pointer: Blink, Charge, Sprint, Dash,
  Fade, Ghost Wolf, Aspect of the Cheetah, Blessing of Freedom, Howl of Terror.
- **A third-person 3D view with WoW's controls:** the pointer stays free; a left-drag swings the
  camera, a right-drag turns the hero, both buttons run; A/D turn and Q/E strafe; a click on an
  enemy (or **Tab**) targets it, and your single-target spells strike it first. **V** switches to
  the classic top-down views, locked or free.
- **A character sheet on C**, as WoW's: your hero in the middle of its paper doll, the gear worn
  on its slots, your run items in the neck, ring and trinket slots, your numbers under it and
  what your items do beside it.
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
the **mouse wheel** to zoom, **C** for the character sheet, **V** to change the view, **Esc** to clear the target or pause,
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

### Legion's data (native only, optional)

Legion's creatures, races, classes and Val'sharah need a **Legion 7.3.5 (build 26972)** client
too, and WoWDBDefs' definitions of its tables. `make fetch-legion` (`scripts/fetch-legion.sh`)
puts both in `legion-data/` (git-ignored):

```bash
make fetch-legion            # the install's game files, 71 GB from archive.org, each file checked
                             # against archive.org's SHA-1; resumable, re-runnable
make fetch-legion STREAM=1   # no big download: the game reads the copy on archive.org as it
                             # needs it (a few hundred MB, cached), with curl, online at play time
make native                  # finds wow-data/ and legion-data/ on its own
```

Either way the definitions come pinned to one WoWDBDefs commit. To use a Legion install of your
own, set `WOW_LEGION` (its folder) and `WOW_LEGION_DBD` (`legion-data/dbd`, or any folder of
WoWDBDefs' `.dbd` files); a variable set wins over what the launcher finds. Without Legion's data
the game is 1.12.1's alone: Legion's races, classes and zone are not offered.

### A build for players

`make dist` builds the native game (a release player build) into `dist/azeroth-survivors/`: the
executable, `scripts/fetch-data.sh` and `scripts/fetch-legion.sh`, and a README. No game data goes
in; a player runs the scripts there once, and the executable finds `wow-data/` and `legion-data/`
beside itself.

## How it's built

The survivors mode is `crates/benilla-app/src/survivors/`, running on two upstream projects:

- [benilla](https://github.com/samwhosung/benilla) — a from-scratch WoW 1.12.1 client.
- [wenilla](https://github.com/Arnesen/wenilla) — benilla compiled to WebAssembly, with a local
  host and realm service.

To work on it, start with [AGENTS.md](AGENTS.md): where everything lives, file by file, the
commands and tests (most need no game data), the rules every change keeps, and the known
pitfalls. Multiplayer is [docs/MULTIPLAYER.md](docs/MULTIPLAYER.md), syncing upstream
[docs/UPSTREAM.md](docs/UPSTREAM.md), and what later expansions' data (Wrath 3.3.5a) would take
[docs/EXPANSIONS.md](docs/EXPANSIONS.md).

## Legal

*Azeroth Survivors* is a free fan game by **Aethus**, not affiliated with or endorsed by
Blizzard Entertainment, and it ships no Blizzard game files. The screenshots in `docs/screenshots/`
show World of Warcraft art rendered from a user-supplied client; that art is © Blizzard
Entertainment. World of Warcraft is a trademark of Blizzard Entertainment, Inc. The engine code in
this repository is licensed under
[MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option; the vendored components under
`third_party/` keep their own upstream licenses.
