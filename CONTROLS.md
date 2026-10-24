# Controls

**Azeroth Survivors** — a third-person 3D view behind your hero: the mouse looks around, WASD runs
where you look. **C** switches to the classic top-down views.

| When | Key / Input | Action |
|---|---|---|
| Menu | Click a race, gender, class and zone | make your pick (**Randomize** rolls one) |
| Menu | **Enter** (or **Enter World**) | start the run; the world travels to the zone picked |
| Menu | **Multiplayer** (bottom left) | create or join a lobby — see [docs/MULTIPLAYER.md](docs/MULTIPLAYER.md) |
| Playing | **mouse** | look around; the hero turns with the view |
| Playing | **W / S / A / D** or **arrow keys** | run where the camera looks, backpedal, strafe |
| Playing | **Space** | jump |
| Playing | **F** (from level 5) | your class's movement spell: Blink, Charge… aimed at the crosshair |
| Playing | **mouse wheel** | zoom the camera in and out |
| Playing | hold **Alt** | free the mouse pointer (minimap buttons, tooltips) |
| Playing | **C** | cycle the view: third person → top-down → top-down unlocked |
| Any | **G** | toggle the picture polish (`?grade=0` starts it off) |
| Playing | **Esc** | pause (frees the pointer) |
| Playing | — | abilities cast themselves (no aim) |
| Level up | **1 / 2 / 3** (or click a card) | pick an upgrade |
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

In the menu the camera slowly orbits the preview hero. Your race, gender, class and zone are
remembered across reloads.
