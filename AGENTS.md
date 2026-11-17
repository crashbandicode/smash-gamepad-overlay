# Project Instructions

This is a Rust Skyline plugin for Super Smash Bros. Ultimate.

Use Rust only for the plugin implementation. Do not create C or C++ source files unless explicitly asked.

Runtime constraints:
- The shipped plugin must remain an in-game Skyline plugin.
- The overlay must render inside Smash so it is captured by a capture card.
- Do not implement a PC overlay, OBS browser source, TCP/UDP streaming, RetroSpy hardware path, or external receiver.
- Online research, git clones, and local tooling scripts are allowed for development and reverse-engineering reference.
- Do not commit Nintendo assets: `data.arc`, extracted BFLYTs/BFLANs/BNTX, textures, or modified/generated `layout.arc` files.
- Do not reuse live HUD text panes or clone/append live panes at runtime. The renderer only updates SGPO's own panes, by skin pane name. Since 0.4.0 those panes are added to the `info_melee` archive's bytes at load, before the game builds any pane (`src/layout_merge.rs`, `crates/sgpo-layout-merge`): the operator approved this on 2026-11-17 ("yes release 0.4.0 after its proven in eden", the go for `docs/NRO-ONLY-DESIGN.md` option C).

Build target:
- `cargo-skyline`
- Rust
- Skyline plugin loaded by Smash

## Working rules

- Native Ubuntu commands run through `/usr/bin/pwsh -NoLogo -NoProfile -NonInteractive -Command`.
- Global routing authority: `/home/intpa/.codex/DELEGATION_ROUTING.md`.
- Forgejo `origin` (git.syntaxbyexample.com) is the source of truth; the GitHub remote is secondary. Do not push without the operator's explicit permission.
- Do not add a handoff file to this repository; keep it with the task's evidence outside the repository (the v0.3.1 work's is `/home/intpa/smash-training-lobby/artifacts/phase23/overlay-investigation/HANDOFF.md`).
- Before completing Cargo work: `cargo skyline clippy` with zero warnings, `cargo skyline build --release`, and the host tests in `crates/sgpo-ui-cache-metadata`, `crates/sgpo-final-input` and `tools/sgpo_installer`. Host `cargo clippy` on the plugin crate fails on stable Rust (see `docs/TECHNICAL.md`).
- Release NROs, SD zips and SHA-256 manifests are published on Forgejo releases and kept with the release's evidence, never committed.

## Current Milestone

Current baseline:
- Visual overlay works with no `layout.arc` on the card (0.4.0): an ARCropolis file callback merges SGPO's panes into whatever `info_melee` ARCropolis serves (vanilla or another mod's), in place. An older patched `layout.arc` still installed is left as it is.
- Buttons and analog sticks show the game's final input for P1 on the player's own layout (since v0.3.0), with the raw P1 HID state as the fallback.
- One render path for every setup (since 2026-11-05): the non-draw HUD path in P1's HUD parts, which needs SGPO's panes in that layout (merged at load since 0.4.0, or the installer's patched layout). Do not choose behaviour by other plugins' file names; that is how TrainingRooms broke the overlay.

Near-term goals:
- Keep the current A-marker/full square overlay as the stability baseline.
- Improve visual polish without breaking pane caching or fallback behavior.
- Two paths build SGPO's panes, from one pane table (`crates/sgpo-layout-merge`): the plugin's merge at load, and `tools/sgpo_installer` on the PC. The operator approved the second path on 2026-11-17 ("yes release 0.4.0 after its proven in eden"). Since 0.4.1 the merge also adds the textured RetroSpy skins (`switch_pro_alt_builtin`, `gamecube_tron_builtin`): their panes, materials and BNTX textures come from a blob that `tools/skin_blob.py` extracts from an installer-patched layout and the build embeds (`SGPO_SKIN_BLOB`; the blob is never committed). The operator asked for this on 2026-11-17 ("I want it all built into the nro without the arc file and it look like just like it does today"). Extend these two paths rather than adding a third; the installer's `in_place_merge_matches_the_installer_patch` test and `crates/sgpo-layout-merge/tests/full_skins.rs` keep them identical.

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
1. `README.md`, `docs/STATUS.md` and `docs/TECHNICAL.md`
2. `docs/info-melee-layout-notes.md`
3. `docs/alpha-validation-checklist.md`
4. `docs/skin-converter-notes.md` if working on skins
