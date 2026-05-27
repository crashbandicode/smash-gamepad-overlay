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

- Without Training Modpack, test once with the patched layout missing or stale and confirm Visual mode logs one fallback message.
- Confirm DebugText fallback appears and remains usable.
- Confirm missing visual panes are not searched/logged repeatedly every frame.

## Training Modpack Compatibility

- Install `local-assets/modified/info_melee/layout.arc` as a normal Smash data replacement at `ui/layout/info/info_melee/info_melee/layout.arc`.
- Prefer staging the ARCropolis layout mod with `python tools/stage_arcropolis_layout.py`, then copy `target/arcropolis/smash-gamepad-overlay` to `sd:/ultimate/mods/`.
- Build SGPO with default settings.
- Confirm the runtime log build ID has the expected git change-count prefix and local build number.
- Enable Training Modpack with a normal SGPO build and confirm this plugin logs `using non-draw HUD path and skipping shared layout/draw hooks`.
- Confirm Training Modpack does not show `Failed to find offset for LAYOUT_ARC_MALLOC` or `Could not find pane TrModInputLog`.
- Start a non-training match and confirm the visual overlay appears dim by default, then updates with controller input.
- Enter Training mode without `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD` and confirm SGPO renders alongside Training Modpack.
- Create an empty `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD`, relaunch, and confirm SGPO stays inactive only in Training mode.
- Remove `HIDE_TRAINING_GAMEPAD`, relaunch, and confirm SGPO renders in Training mode again.
- Leave Training mode, start another non-training match, and confirm SGPO renders again.
- If the overlay does not appear, check whether the log shows `non-draw HUD path captured skin` or a missing `sgpo_root` message.
- If the log says `sgpo_root` is missing and `original_set_rep_01=true`, the loaded P1 HUD parts layout is unpatched; check the ARCropolis layout mod path.
- A missing Training Modpack data-replacement layout should log once and stay inactive; DebugText fallback is only expected on the non-Training-Modpack draw path.
