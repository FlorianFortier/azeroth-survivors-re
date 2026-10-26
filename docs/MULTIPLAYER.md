# Multiplayer

Up to four players run a battleground together. One player **hosts a lobby**; the others **join
it with its five-character code** (or find it in the server's list, if the host made it public).
The host tunes the run's rules, everyone readies up, and the host starts it.

There is no Blizzard server involved and no account: the lobby runs on a small server you host
yourself, which needs no game files.

## Play

### Everyone on one machine's network (LAN, or a VPN like Tailscale)

The host serves the game as usual; `wenilla-host` mounts the lobby at `/lobby` beside it.

```bash
make build                                  # once
cargo run --release -p wenilla-host -- --www web/dist --data wow-data/Data --bind 0.0.0.0:8090
```

Players open `http://<host-ip>:8090/`, pick a hero, click **Co-op**, and either
**Create a lobby** or type the host's code and **Join**. The page's own origin is the lobby
server, so nothing else needs configuring.

> `--bind 0.0.0.0` exposes `/data` (your game files) to that network. Keep it to a network you
> trust. To host lobbies on the open internet, use `survivors-server` below.

### Players who each run the game themselves

Each player runs the game locally (`make start`, or `make native`), and everyone connects to one
**lobby server**:

```bash
cargo run --release -p survivors-server -- --bind 0.0.0.0:7777
# or: make lobby
```

`survivors-server` serves no game files, so it is safe to expose. Point the game at it:

- in the lobby screen's **Server** field: `ws://your.server:7777/lobby`;
- in the browser: `http://127.0.0.1:8090/?lobby=ws://your.server:7777/lobby`;
- on the desktop: `WOW_LOBBY=ws://your.server:7777/lobby make native`.

A page served over `https` can only open `wss://` sockets: put the lobby server behind a TLS proxy
(Caddy, nginx, a `tailscale serve`) and use `wss://…/lobby`. The desktop build speaks plain
`ws://` only.

The player name defaults to "Adventurer"; set it in the lobby screen, or with `?name=` / `WOW_NAME`.

`GET /lobby/health` answers `ok protocol=… lobbies=… connections=…` for monitoring.

## The lobby

| | |
|---|---|
| **Code** | Five characters from an alphabet without look-alikes (no `0/O`, `1/I/L`). Typing is forgiving: case, spaces and dashes are ignored. |
| **Seats** | 1 to 4. The host is always seat 1 (★). The host can remove a player (×). |
| **Ready** | Every player but the host readies up; the host's **Start the run** unlocks when all are ready. Changing a rule clears everyone's ready. |
| **Change hero** | Back to the hero picker while keeping your seat; Enter, the big Enter button or Co-op return to the lobby with the new hero. |
| **Leave** | A guest leaving frees the seat. The host leaving closes the lobby for everyone. |

### Rules the host sets

| Rule | Range | Default |
|---|---|---|
| Zone | any the host's campaign has opened | the host's pick |
| Seats | 1 – 4 | 4 |
| Difficulty preset | Easy, Normal, Hard, Nightmare | Normal |
| Enemy health, enemy damage, crowd size | 25 % – 400 %, by 10 | 100 % |
| Grows with players | on / off: +50 % enemy health and +35 % crowd per extra hero | on |
| Final boss at | 5:00 – 30:00, by the minute; the other four bosses' times scale with it; a stage runs 60 % of it, its boss portal opening at that time | 16:00 |
| An ambush every | 1:00 – 5:00, by 15 s: the horde closes a ring around every hero | 2:15 |
| Shared experience | every hero gains what any hero picks up | on |
| Revive allies | stand over a fallen ally for 4 s to bring them back at half health | on |
| Public | listed in **Browse public lobbies**, or joined by code only | code only |

The server clamps every value it receives, and the game clamps again before it plays. A run is
three stages, a zone each; when a stage's boss falls a portal onward opens, and a hero standing in
it takes everyone over: the host waits for every guest to load the new zone, as at the start. The
run is won when the final swarm after the third stage is held, and every player's profile counts
the win on the zone the run began in. The crowd never exceeds
400 enemies, whatever the density.

## In a run

- The run never pauses: a player choosing a level-up card keeps moving, and the pause menu only
  covers the screen of the player who opened it.
- Each player's own hero moves on their own machine, so it responds instantly. Everything else —
  the horde, spells, damage, gems — is the host's, streamed to the others.
- Allies show as blue dots on the minimap and as party frames at the left edge.
- A fallen hero waits for a revive, if the rules allow it. The run is lost when every hero is
  down; it is won when the clock runs out.
- The results show the whole party's levels, kills and damage. **Play again** takes everyone back
  to the lobby, where the host can start the next run.

## How it works

```
 guest ──WebSocket──┐                      ┌──WebSocket── guest
                    ├── lobby server ──────┤
 guest ──WebSocket──┘  (codes, seats,      └──WebSocket── host (plays the run)
                        ready, relay)
```

- **`crates/survivors-net`** — the wire protocol, the lobby settings and the lobby server's state
  machine ([`hub.rs`](../crates/survivors-net/src/hub.rs)). No I/O and no dependencies: a small
  binary codec of its own, shared by the server, the native game and the browser game.
- **`crates/survivors-server`** — the hub behind an axum WebSocket at `/lobby`. Bounded per-connection
  queues, a frame-rate limit, idle timeouts. Its router is what `wenilla-host` mounts.
- **`crates/benilla-app/src/survivors/net/`** — the game side:
  - `socket.rs`: the WebSocket (a `tungstenite` thread on the desktop, the page's `WebSocket` in
    the browser);
  - `host.rs`: the host streams the run the way a WoW server streams a world — each unit with its
    update fields, field changes, motion, despawns, and every cast, impact, swing and combat
    number — about twenty times a second, batched into one frame per tick;
  - `client.rs`: a guest builds those units exactly as the client builds a server's (a `Guid`, a
    `NetEntity` and an `ObjectStore` are all the engine's attach, animation and spell-visual
    systems need), draws them a tenth of a second behind the host between two positions it really
    sent, and sends back its own hero's motion (stamped with its clock), moves at the level-up
    cards (pick, reroll, banish, skip) and mobility casts;
  - `smooth.rs`: that interpolation, the same on both sides: each motion carries its sender's
    clock, a `Track` keeps the last positions and a `Clock` maps the sender's time to ours by the
    quickest arrival, so a late packet does not stutter the picture; the host draws each guest's
    hero the same way, rather than snapping it to every message;
  - `ui.rs`: the lobby screen, the party frames and the scoreboard.
- **The simulation** (`survivors/`) plays any number of heroes: each carries its own numbers
  (`HeroState`), enemies chase the nearest hero they can see, every blow is credited to the hero
  who dealt it, and gems fly to the nearest magnet.

The host checks what guests send: positions are held to the battleground; the host deals every
hero's level-up cards and their rarities, so a pick must be a card it dealt that hero at the
rarity it dealt it, and a reroll, banish or skip one the hero holds (the banished cards stay on
the host); a shrine's blessing must be one of the three the host offered that hero; a purchase must be of a
ware still at the merchant, by a hero standing at it with the gold; a mobility
cast must be the hero's own and learned. It is a co-op game
among friends, not an anti-cheat: a guest's hero position is trusted within the battleground.

## Limits

- Four players. No joining a run already in progress, and no reconnecting to one.
- If the host leaves, the run ends for everyone.
- Bandwidth: the host uploads roughly 100–200 KB/s per guest late in a run (a full horde moving
  twenty times a second). Fine on a LAN or decent broadband.
- The desktop build cannot reach a `wss://` lobby.

### What each hero brings

A player's loadout carries what their own profile brings into a run beyond their race: the gear
they wear (by name), the bonuses of that gear, of their collection, of their class's talents and
of their perks, and the ranks of their other perks (rerolls, banishes, skips, the first hand,
gold at the start). The host, who plays every hero, seats each guest's hero with its race's
racials, those bonuses and perks, dressed in that gear. Each value is held to a cap on the way in; it is a game
among friends, not an anti-cheat.
