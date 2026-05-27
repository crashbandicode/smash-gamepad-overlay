# smash-gamepad-overlay

Rust Skyline plugin for Super Smash Bros. Ultimate that renders a simple P1 controller input overlay inside the game.

The first milestone is intentionally small:

- Poll P1 raw controller state.
- Draw text during matches using Smash UI panes.
- Show pressed buttons, left stick, right stick, and GameCube analog trigger values when available.

## Current Status

- Tested against Smash display version `13.0.4`.
- Current tested environment notes: ARCropolis `4.0.7`, Atmosphere `1.11.1`, and Eden `0.1.0`.
- Without Training Modpack, the known-good path still uses `nn::ui2d::Layout::Draw`, which resolves to `.text+0x4b620` in the tested setup.
- The normal draw path is gated to display version `13.0.4` because its ui2d helper offsets are version-specific.
- With Training Modpack present at its standard plugin path, this plugin skips the shared `Layout::Draw` hook and uses a non-draw HUD capture/update path.
- Training Modpack compatibility requires the patched `info_melee/layout.arc` to be installed as a normal Smash data replacement.
- The overlay targets the match HUD layout, `info_melee`.
- The overlay has two display modes: `Visual` and `DebugText`.
- `Visual` mode is configured by default and targets the `minimal_debug` square pane skin under `sgpo_root`.
- A second built-in skin definition, `switch_pro_alt_builtin`, mirrors the RetroSpy `switch-pro-alt` layout as data for future custom-skin conversion, but it is not active until matching panes/assets are generated.
- The visual panes come from a modified `info_melee` `layout.arc`. The default build expects that layout to be installed as a normal Smash data replacement.
- Pressed controls dim/brighten and scale through a `SkinElement` renderer loop; missing injected panes fall back to `DebugText`.
- Visual panes are resolved once per `info_melee` layout/root instance in the normal draw path, or once from captured P1 HUD parts layout data in the Training Modpack path, and then cached for per-frame updates.
- When Training Modpack is loaded, SGPO still renders in non-training matches and also renders in Training mode by default. Create `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD` to suppress SGPO only in Training mode.
- DebugText fallback is available on the normal draw path. With Training Modpack loaded, missing visual panes are logged and SGPO stays inactive rather than installing the conflicting draw hook.

## Requirements

- Rust toolchain configured for Skyline development.
- `cargo-skyline`.
- Skyline installed for Smash.
- Smash Ultimate title ID: `01006A800016E000`.

The current development setup is tailored around a Windows 11 host with Ubuntu running in WSL2. Emulator deployment paths, plugin copy paths, and log locations should stay local in `.env`.

## Agent Use

This repo includes a few handoff files for AI coding agents:

- `AGENTS.md`: general project instructions and constraints for Codex-style agents.
- `CLAUDE.md`: Claude Code entry point that references the shared handoff files.
- `AGENTSUMMARY.md`: canonical current-state summary; update it when architecture, hooks, asset workflow, offsets, or next steps change.
- `sgpo-handoff.mdc`: portable Cursor rule content for this project.

For Cursor, copy the tracked `sgpo-handoff.mdc` file into your local workspace rules path:

```text
.cursor/rules/sgpo-handoff.mdc
```

The local `.cursor/` directory is ignored. Keep the tracked root `sgpo-handoff.mdc` updated, then copy it into `.cursor/rules/` when you want Cursor to apply it.

## Build

Check the plugin:

```sh
cargo skyline check
```

Build a release NRO:

```sh
cargo skyline build --release
```

Runtime logs include a build ID such as `c12-b3-...`. `c12` is derived from `git rev-list --count HEAD`; `b3` is a local build counter that increments when Cargo rebuilds the plugin, which helps spot stale NRO installs during normal testing. The local build counter lives under `target/` and resets after `cargo clean`.

Build with the old NRO-embedded layout injection hook enabled:

```sh
SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT=1 cargo skyline build --release
```

Do not use embedded layout injection with Training Modpack. The default build keeps this hook out of the NRO so Training Modpack can scan and hook the same offset safely.

For the current visual mode, generate `local-assets/modified/info_melee/layout.arc` from a local Smash 13.0.4 `data.arc` dump before building. Local game dumps and extracted layout assets are ignored and should not be committed.

The patcher adds hidden SGPO panes to the root `info_melee` layout and to both player HUD parts layouts. The root panes are used by the normal draw path; the player-parts panes are used by the Training Modpack non-draw path.

Validate the patcher's BFLYT/pic1 assumptions against your unpacked source layout:

```sh
python tools/patch_info_melee_layout.py --self-test
```

Stage the generated layout into an ARCropolis mod folder:

```sh
python tools/stage_arcropolis_layout.py
```

This creates:

```text
target/arcropolis/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc
```

Copy the `target/arcropolis/smash-gamepad-overlay` folder into your ARCropolis mods folder, for example `sd:/ultimate/mods/smash-gamepad-overlay`.

For local emulator testing, copy `.env.example` to `.env` and set:

```text
SGPO_DEPLOY_EMU=1
SGPO_EMU_PLUGIN_DIR=/path/to/emulator/user/sdmc/atmosphere/contents/01006A800016E000/romfs/skyline/plugins
SGPO_EMU_MODS_DIR=/path/to/emulator/user/sdmc/ultimate/mods
SGPO_EMU_LOG_PATH=/path/to/emulator/user/sdmc/smash-gamepad-overlay.log
```

With `SGPO_DEPLOY_EMU=1`, `python tools/stage_arcropolis_layout.py` also copies:

```text
target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro
```

into `SGPO_EMU_PLUGIN_DIR`, and stages the generated layout as:

```text
SGPO_EMU_MODS_DIR/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc
```

Use `--no-emu` to stage only under `target/arcropolis`, or `--skip-nro` to deploy only the layout.

The patched `info_melee` layout is expected to contain:

```text
sgpo_root
  sgpo_pro_lt
  sgpo_pro_lb
  sgpo_pro_rt
  sgpo_pro_rb
  sgpo_pro_minus
  sgpo_pro_plus
  sgpo_pro_l3
  sgpo_pro_r3
  sgpo_pro_ls_gate
  sgpo_pro_ls_dot
  sgpo_pro_rs_gate
  sgpo_pro_rs_dot
  sgpo_pro_du
  sgpo_pro_dd
  sgpo_pro_dl
  sgpo_pro_dr
  sgpo_pro_dul
  sgpo_pro_dur
  sgpo_pro_ddl
  sgpo_pro_ddr
  sgpo_pro_btn_y
  sgpo_pro_btn_x
  sgpo_pro_btn_b
  sgpo_pro_a_marker
```

The active built-in skin for this layout is `minimal_debug`.

## Skin Model

The renderer is skin-driven. It does not create or decode arbitrary PNG assets at runtime. Each skin is a Rust data table of `SkinElement` entries that map a logical `ControlId` to an already-existing named Smash UI pane.

Each `SkinElement` describes:

- logical control ID
- pane name
- optional image/material names for generated assets
- base x/y position
- width/height
- released/pressed alpha and scale
- optional stick movement range

Current built-in skins:

- `minimal_debug`: active square-based alpha skin using the patched `sgpo_pro_*` panes.
- `switch_pro_alt_builtin`: inactive data-only skin based on RetroSpy's `switch-pro-alt` Switch section and PNG dimensions.

Future custom skin flow:

```text
RetroSpy skin.xml + PNG assets
  -> PC-side converter
  -> generated layout.arc picture panes/materials/textures
  -> generated skin manifest or Rust skin table
  -> plugin updates panes by ControlId
```

The plugin should stay focused on polling controller state and updating named panes. Heavy PNG conversion, texture packing, material creation, and pane injection should happen on the PC side before runtime.

See [Skin Converter Notes](docs/skin-converter-notes.md) for the planned converter boundary.

The release artifact is expected at:

```text
target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro
```

## Install

Copy the NRO into Smash's Skyline plugin folder:

```text
atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmash_gamepad_overlay.nro
```

For emulator testing, use the equivalent mod/plugin path for the emulator's Smash mod directory.

## Training Modpack

Training Modpack hooks both `Layout::Draw` and the same layout-arc handoff that SGPO can optionally use for NRO-embedded layouts. To coexist, the default SGPO build does not include the embedded layout hook and does not install the draw hook while Training Modpack is detected.

In this mode, SGPO renders in normal matches and in Smash Training mode by default. To hide SGPO in Training mode while leaving it enabled everywhere else, create this empty flag file:

```text
sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD
```

For Eden on Windows/WSL, that maps to:

```text
<eden>/user/sdmc/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD
```

Install the patched `layout.arc` through your normal Smash data replacement/mod loader path instead:

```text
ui/layout/info/info_melee/info_melee/layout.arc
```

Use the locally generated file:

```text
local-assets/modified/info_melee/layout.arc
```

Do not commit or publish that `layout.arc`; it is generated from your local game dump.

## Runtime Log

The plugin writes a small diagnostic log to:

```text
sd:/smash-gamepad-overlay.log
```

For Eden on Windows/WSL, that usually maps to:

```text
<eden>/user/sdmc/smash-gamepad-overlay.log
```

This is useful when `cargo skyline listen` does not show output. The log records startup, build ID, Smash display version, hook resolution, non-draw HUD capture/update status, whether `info_melee` was seen, and the active display path.

If Training Modpack is loaded and the log says `could not find ... sgpo_root`, Smash is loading an unpatched `info_melee` layout. Regenerate and restage the ARCropolis layout mod.

## Known Limitations

- The visual HUD is intentionally rough programmer art. It uses cloned picture panes, not custom textures or labels.
- Training Modpack compatibility depends on installing the patched `info_melee/layout.arc` as a normal data replacement. Keep `SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT` unset for Training Modpack builds.
- The Training Modpack path currently captures P1's HUD parts layout, so placement is local to the P1 HUD and cannot reach true bottom-right without clipping. This keeps the overlay tied to player-HUD pause visibility.
- Training mode uses a separate left-side P1 HUD-local placement to avoid the CPU overlay near P1.
- The visual HUD is pane-based. It does not use custom textures yet.

## Validation

Before using a build as a new baseline, run through [Alpha Validation Checklist](docs/alpha-validation-checklist.md).

## Project Constraints

This project is Rust-only. It should not use C/C++, a PC overlay, OBS/browser sources, network streaming, RetroSpy hardware, or any external receiver. The overlay must render inside Smash so it is visible to capture cards.
