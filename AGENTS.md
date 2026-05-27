# Project Instructions

This is a Rust Skyline plugin for Super Smash Bros. Ultimate.

Use Rust only for the plugin implementation. Do not create C or C++ source files unless explicitly asked.

Runtime constraints:
- The shipped plugin must remain an in-game Skyline plugin.
- The overlay must render inside Smash so it is captured by a capture card.
- Do not implement a PC overlay, OBS browser source, TCP/UDP streaming, RetroSpy hardware path, or external receiver.
- Online research, git clones, and local tooling scripts are allowed for development and reverse-engineering reference.
- Do not commit Nintendo assets: `data.arc`, extracted BFLYTs/BFLANs/BNTX, textures, or modified/generated `layout.arc` files.

Build target:
- `cargo-skyline`
- Rust
- Skyline plugin loaded by Smash

## Current Milestone

Current baseline:
- Visual overlay works with patched `info_melee/layout.arc`.
- Buttons and analog sticks update from raw P1 HID state.
- `DebugText` remains the fallback on the normal draw path.
- Training Modpack compatibility uses a non-draw HUD path and requires the patched layout to be installed through ARCropolis as a data replacement.

Near-term goals:
- Keep the current A-marker/full square overlay as the stability baseline.
- Improve visual polish without breaking pane caching or fallback behavior.
- Continue toward a skin-capable system where RetroSpy-style skin packs can be converted into patched Smash layout assets and matching `SkinElement` data.

Prefer simple, testable steps over a large polished implementation.

## External Reference Map

Agents may use online research and clone/read external open-source projects for reference. Do not copy large blocks blindly; translate concepts into this Rust/cargo-skyline project.

Useful references:
- UltimateTrainingModpack: native Smash UI hooks, `Layout::Draw` signature scan, `info_training`/`info_melee` behavior, and Training Modpack compatibility concerns.
- skyline-smash / skyline-rs: Rust Skyline bindings, ui2d helper offsets, hook/from_offset patterns.
- HDR / HewDraw Remix: match-mode hooks, utility patterns, byte scanning, game-mode/HUD-related references.
- local-latency-slider-de: non-draw scene/update hook patterns and latency-sensitive hook style.
- RetroSpy / Open Joystick Display / m-overlay: controller overlay layout, skin concepts, control naming, and stick/button visualization only.
- ArcExplorer / smash-arc: extracting `ui/layout/info/info_melee/info_melee/layout.arc` from `data.arc`.
- Python `sarc`: unpacking/repacking `layout.arc`.
- Switch Toolbox / Switch Layout Editor: manual BFLYT/BFLAN inspection/editing references.

Before major changes, read:
1. `README.md`
2. `AGENTSUMMARY.md`
3. `docs/info-melee-layout-notes.md`
4. `docs/alpha-validation-checklist.md`
5. `docs/skin-converter-notes.md` if working on skins
