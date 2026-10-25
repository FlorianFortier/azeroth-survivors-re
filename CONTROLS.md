# Controls

**Azeroth Survivors** — a third-person 3D view behind your hero, with WoW's mouse and keys: the
pointer stays free, a drag looks around, A/D turn and Q/E strafe. **C** opens the character
sheet, and **V** switches to the classic top-down views.

| When | Key / Input | Action |
|---|---|---|
| Menu | Click a class and a zone | make your pick; **Random** rolls a name in the race's sound. A locked zone says which final boss opens it |
| Menu | Click the hero's portrait, **Armory** or an equipment slot | open the Armory: race, sex, look and the gear worn |
| Armory | drag the hero, or the arrows under it | turn the hero |
| Armory | the arrows beside an option, or **Randomize** | change the skin, face, hair, hair colour or facial hair |
| Armory | a slot on the left, then a piece on the right | wear it (**None** takes it off); **Done** goes back |
| Menu | **mouse wheel** over the zones | scroll the zone list |
| Menu | **Enter** (or **Enter <zone>**) | start the run; the world travels to the zone picked |
| Menu | **Co-op** (bottom right) | create or join a lobby — see [docs/MULTIPLAYER.md](docs/MULTIPLAYER.md) |
| Playing | **left button** held and dragged | swing the camera around the hero |
| Playing | **right button** held and dragged | turn the hero and the camera together (A/D then strafe) |
| Playing | **both mouse buttons** held | run forward, steering with the mouse (toward the cursor in the top-down view) |
| Playing | **click** an enemy (left or right) | target it: a ring under it, and your single-target spells strike it first while in reach |
| Playing | **left click** on nothing, or **Esc** | clear the target (Esc pauses once nothing is targeted) |
| Playing | **Tab** | target the nearest enemy ahead; again for the next |
| Playing | **W / S** or **up / down arrows** | run forward, backpedal (slower, as in WoW) |
| Playing | **A / D** or **left / right arrows** | turn the hero |
| Playing | **Q / E** | strafe |
| Playing | **Space** | jump; a running jump keeps its momentum, a standing one may be steered once |
| Playing | **F** (from level 5) | your class's movement spell: Blink, Charge… aimed at the pointer |
| Playing | **mouse wheel** | zoom the camera in and out |
| Playing | **C** | the character sheet: the paper doll around your hero, your numbers, what your run items do; the run waits under it. **C**, **Esc** or its **X** shut it |
| Sheet | drag the hero, or the arrows over it | turn the hero |
| Playing | **V** | cycle the view: third person → top-down → top-down unlocked |
| Any | **G** | toggle the picture polish (`?grade=0` starts it off) |
| Playing | **Esc** | clear the target, or pause |
| Playing | — | abilities cast themselves (no aim) |
| Playing | stand at a **chest** for a second | open it with the run's gold (each one costs more), or with a key if you hold one |
| Playing | at the **merchant**: **1 / 2 / 3** (or click a ware) | buy that ware with the run's gold |
| Playing | walk into a **jar** or a **crate** | break it: gold, or a heal |
| Playing | stand in a **shrine** | charge it (Power: choose a blessing with **1 / 2 / 3** or a click), or wake it (Challenge, Greed, Curses) |
| Level up | **1 / 2 / 3** (or click a card) | pick an upgrade |
| Level up | **R** (or **Reroll**) | deal the cards again: three rerolls to start a run |
| Level up | **B** (or **Banish**), then a card | take that card out of your deals for the rest of the run: one to start |
| Level up | **X** (or **Skip**) | let the level go by without a card: one to start |
| Menu | **Run Options** (bottom left) | the difficulty: Heroic opens once a zone is won on Normal, Mythic once won on Heroic |
| Paused | **Esc** | resume |
| Game over | **Enter** (or **Play again**) | back to the menu (or the lobby, in multiplayer) |
| Any | minimap **+ / −** / minimize | minimap zoom / collapse |

## The views

- **Third person** (default), as in WoW: the camera hangs behind and above the hero and the
  pointer stays free. Holding the left button and dragging swings the camera alone; the right
  button turns the hero with it, and while it is held **A / D** strafe instead of turning. Let go,
  and the pointer comes back where it was; a press let go without moving is a click. Running, the
  camera swings back behind the hero. The boom pulls in when a hill comes between the camera and
  the hero. Mouse speed: `WOW_MOUSE_SENS=1.5` (desktop) or `?mouse_sens=1.5` (browser);
  1 is the default, from 0.1 to 10.
- **Top-down**: locked at 61°, laptop-friendly; the hero faces the cursor and WASD is
  world-relative.
- **Top-down unlocked**: a mouse drag swings and tilts the camera around the hero.

In the menu the camera frames the hero in its portrait and slowly turns around it; in the
Armory, a drag turns it. Your race, gender, class and zone are
remembered across reloads.
