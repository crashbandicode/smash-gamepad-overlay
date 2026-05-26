# smash-gamepad-overlay

Rust Skyline plugin for Super Smash Bros. Ultimate that renders a simple P1 controller input overlay inside the game.

The first milestone is intentionally small:

- Poll P1 raw controller state.
- Draw text during matches using Smash UI panes.
- Show pressed buttons, left stick, right stick, and GameCube analog trigger values when available.

## Current Status

- Tested against Smash display version `13.0.4`.
- The `nn::ui2d::Layout::Draw` signature resolves to `.text+0x4b620` in the tested setup.
- The overlay draws only while the match HUD layout, `info_melee`, is being rendered.
- The overlay has two display modes: `Visual` and `DebugText`.
- `Visual` mode is configured by default and targets a minimal Switch Pro Controller pane skin under `sgpo_root`.
- The visual panes come from a modified `info_melee` `layout.arc` embedded at build time when `local-assets/modified/info_melee/layout.arc` exists.
- Pressed controls dim/brighten and scale through a `SkinElement` renderer loop; missing injected panes fall back to `DebugText`.
- If Training Modpack is installed at its standard Skyline plugin path, this plugin skips installing the draw hook to avoid a known hook/signature conflict.

## Requirements

- Rust toolchain configured for Skyline development.
- `cargo-skyline`.
- Skyline installed for Smash.
- Smash Ultimate title ID: `01006A800016E000`.

## Build

Check the plugin:

```sh
cargo skyline check
```

Build a release NRO:

```sh
cargo skyline build --release
```

For the current visual mode, generate `local-assets/modified/info_melee/layout.arc` from a local Smash 13.0.4 `data.arc` dump before building. Local game dumps and extracted layout assets are ignored and should not be committed.

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

## Runtime Log

The plugin writes a small diagnostic log to:

```text
sd:/smash-gamepad-overlay.log
```

This is useful when `cargo skyline listen` does not show output. The log records startup, Smash display version, draw-hook resolution, whether `info_melee` was seen, and the active display path.

## Known Limitations

- The visual HUD is intentionally rough programmer art. It uses cloned picture panes, not custom textures or labels.
- Training Modpack can conflict with this plugin's draw hook. Disable or move Training Modpack when testing this overlay.
- The visual HUD is pane-based. It does not use custom textures yet.

## Project Constraints

This project is Rust-only. It should not use C/C++, a PC overlay, OBS/browser sources, network streaming, RetroSpy hardware, or any external receiver. The overlay must render inside Smash so it is visible to capture cards.
