# Alpha Validation Checklist

Use this checklist before treating the current square-pane visual overlay as a stable baseline for visual polish.

## Match Flow

- Start a match and confirm the visual overlay appears before or by `GO`.
- Pause and unpause, then confirm button highlights and stick dots still update.
- Lose a stock and confirm the overlay remains visible and responsive.
- Finish the match and confirm there is no crash or hang on results/loadout transitions.
- Return to character select, start another match, and confirm the overlay reappears.

## Controllers

- Pro Controller: confirm A/B/X/Y, LB/RB/LT/RT, L3/R3, Plus/Minus, d-pad, and both stick dots update.
- GameCube controller: confirm face buttons, L/R trigger values where mapped, and both stick dots update.
- Handheld/Joy-Con, if available: confirm the plugin does not report `controller not ready` and the overlay updates.

## Fallback

- Test once with the patched layout missing or stale and confirm Visual mode logs one fallback message.
- Confirm DebugText fallback appears and remains usable.
- Confirm missing visual panes are not searched/logged repeatedly every frame.
