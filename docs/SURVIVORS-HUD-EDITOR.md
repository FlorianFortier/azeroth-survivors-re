# Live Survivors HUD layout editor

The native development build includes a manual editor for the Survivors action bar. It does not
use an AI agent and does not need Ollama, OpenCode or a game restart for layout iterations.

## Open the editor

1. Launch the native development build from the repository root so the default relative
   configuration path resolves.
2. Open the existing development overlay with the dev chord + `D`.
3. The **Survivors — Action Bar Layout** window appears alongside the existing debug panel.

The controls adjust the action bar width and height and move the bar horizontally or vertically.
The bar uses a compact ElvUI-inspired presentation: a dark translucent panel, a subtle border and
six individually framed square ability slots. It does not use Blizzard's MainMenuBar or its engraved
slot artwork. The six slots are ordinary learned abilities; no mobility spell occupies one of them.
A future class-active ability will be a separate action, without displacing these six learned slots.
Changes are applied to the already spawned HUD as soon as the editor values change.

## Save and external hot reload

The tracked defaults live in `ui-layouts/action_bar.toml`. **Save layout** writes the current
editor values to that file. You can also edit the TOML in an editor; the game checks the file every
250 ms and applies valid changes without restarting.

**Reload file** discards unsaved editor changes and reads the file again. **Reset** restores the
built-in defaults in memory; use **Save layout** afterwards to update the file too.

You can override the watched file with the native development environment variable
`WOW_SURVIVORS_UI_LAYOUT`.

The accepted ranges are width 270–1200, height 48–100, horizontal offset -400–400 and vertical
offset -150–150, all in the HUD's 768-reference-height coordinate space. Invalid TOML or values
are reported in the editor and leave the last valid layout active.

The editor and file watcher are compiled only for native builds with the `dev` feature. Player
builds and the wasm target do not include them. This first iteration edits the action-bar frame,
XP bar and action-row anchors; individual slot sizes, decorative texture offsets and other HUD
panels remain unchanged.
