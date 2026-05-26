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
- `Visual` mode is configured by default and currently targets one Switch Pro Controller A marker named `sgpo_pro_a_marker`.
- The visual marker comes from a modified `info_melee` `layout.arc` embedded at build time when `local-assets/modified/info_melee/layout.arc` exists.
- Pressing A dims/brightens and scales the marker; missing injected panes fall back to `DebugText`.
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

- The visual HUD currently has one injected A-button marker. The rest of the controller HUD still needs panes added to the modified layout.
- Training Modpack can conflict with this plugin's draw hook. Disable or move Training Modpack when testing this overlay.
- The visual HUD is pane-based. It does not use custom textures yet.

## Project Constraints

This project is Rust-only. It should not use C/C++, a PC overlay, OBS/browser sources, network streaming, RetroSpy hardware, or any external receiver. The overlay must render inside Smash so it is visible to capture cards.
