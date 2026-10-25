# Controls

**Azeroth Survivors** — a third-person 3D view behind your hero: the mouse looks around, WASD runs
where you look. **C** switches to the classic top-down views.

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
| Playing | **mouse** | look around; the hero turns with the view |
| Playing | **W / S / A / D** or **arrow keys** | run where the camera looks, backpedal (slower, as in WoW), strafe |
| Playing | **both mouse buttons** held | run forward, as in WoW (toward the cursor in the top-down view) |
| Playing | **Space** | jump; a running jump keeps its momentum, a standing one may be steered once |
| Playing | **F** (from level 5) | your class's movement spell: Blink, Charge… aimed at the crosshair |
| Playing | **mouse wheel** | zoom the camera in and out |
| Playing | hold **Alt** | free the mouse pointer (minimap buttons, tooltips) |
| Playing | **C** | cycle the view: third person → top-down → top-down unlocked |
| Any | **G** | toggle the picture polish (`?grade=0` starts it off) |
| Playing | **Esc** | pause (frees the pointer) |
| Playing | — | abilities cast themselves (no aim) |
| Level up | **1 / 2 / 3** (or click a card) | pick an upgrade |
| Level up | **R** | deal the cards again: three rerolls a run |
| Menu | **Run Options** (bottom left) | the difficulty: Heroic opens once a zone is won on Normal, Mythic once won on Heroic |
| Paused | **Esc** | resume |
| Game over | **Enter** (or **Play again**) | back to the menu (or the lobby, in multiplayer) |
| Any | minimap **+ / −** / minimize | minimap zoom / collapse |

## The views

- **Third person** (default): the camera hangs behind and above the hero, the pointer is hidden
  and the mouse turns the view. Movement is relative to where you look, and the hero faces that
  way, so **S** backpedals and **A / D** strafe. The boom pulls in when a hill comes between the
  camera and the hero. Mouse speed: `WOW_MOUSE_SENS=1.5` (desktop) or `?mouse_sens=1.5` (browser);
  1 is the default, from 0.1 to 10.
- **Top-down**: locked at 61°, laptop-friendly; the hero faces the cursor and WASD is
  world-relative.
- **Top-down unlocked**: a mouse drag swings and tilts the camera around the hero.

In the menu the camera frames the hero in its portrait and slowly turns around it; in the
Armory, a drag turns it. Your race, gender, class and zone are
remembered across reloads.
