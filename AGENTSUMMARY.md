# Agent Summary

## Goal

Build the first milestone of a Rust-only cargo-skyline plugin that renders a P1 input overlay inside Super Smash Bros. Ultimate matches.

## Current Implementation

- Plugin crate: `smash-gamepad-overlay`.
- Entry point: `src/lib.rs`.
- The implementation is split into small modules:
  - `config`: runtime constants and display mode selection.
  - `input`: HID polling, raw snapshots, logical controls, and view-state mapping.
  - `ui`: match-layout draw dispatch plus safe wrappers around the required ui2d offsets.
  - `debug_text`: current working text fallback.
  - `visual` and `skin`: future non-text pane renderer and built-in skin data.
  - `offsets` and `logger`: draw-hook resolution and diagnostics.
- Uses Rust + Skyline only.
- Hooks `nn::ui2d::Layout::Draw` by scanning `.text` for a known instruction signature.
- Only draws when the current layout name is `info_melee`, so it appears in matches rather than menus/training-only UI.
- Polls P1 controller state using `skyline::nn::hid`.
- Tries Npad No1 first, then handheld Npad ID `0x20`.
- Supports FullKey, handheld, Joy-Con styles, and GameCube state.
- Shows P1 ID/style, pressed buttons, left stick, right stick, and GameCube analog trigger values when available.
- Has two display modes:
  - `DebugText`: original compact troubleshooting text.
  - `Visual`: skin-driven generic pane renderer.
- `Visual` currently falls back to `DebugText` because the built-in skin panes are not present yet.
- Skin abstraction added:
  - `ControlId`: logical controls/buttons/sticks/triggers.
  - `ControllerViewState`: maps `ControllerSnapshot` into pressed/released values plus normalized stick/trigger values.
  - `SkinElement`: maps a control to a pane name, base position, size, alpha/visibility states, and optional stick movement radius.
  - Built-in skin: `minimal_gamecube`.
- Writes diagnostics to `sd:/smash-gamepad-overlay.log`.

## Tested Context

- User tested on emulator and Switch.
- Smash display version reported by plugin: `13.0.4`.
- Draw signature resolved to `.text+0x4b620`.
- The plugin boots and draws on Switch after removing risky text-box flags.
- Current tested display may still be one compact line because only one reusable text pane was found in prior testing.
- Emulator test showed live-pane visual rendering worked, but it reused real HUD panes and surfaced Hero MP/other P1 UI.
- Cloned root-level panes then froze emulator at the VS screen after logging `sgpo_*` slots, so that approach was removed from the active renderer.
- Current visual renderer no longer reuses live HUD text panes or cloned textboxes. It only updates generic panes by skin pane name.

## Issues Encountered

- Initial draw signature lookup failed when Training Modpack was enabled.
- Training Modpack also scans/hooks `Layout::Draw`, so this plugin now detects the standard Training Modpack NRO path and skips installing its hook when present.
- Some text-box flags caused a Switch freeze right before the match countdown. Those were removed.
- `cargo skyline listen` was not reliable for logs in the user's setup, so file logging was added.
- Attached Joy-Cons initially reported as controller not ready; fallback polling for handheld Npad ID was added.
- Multi-pane visual rendering originally reused live HUD panes. That caused Hero MP gauge visibility and a Switch freeze.
- Cloning and appending textboxes also froze emulator, likely because raw `TextBox` cloning bypasses Smash/ui2d construction ownership.

## Relevant Offsets And Sources

- `Layout::Draw` is found dynamically by signature scan.
- The tested 13.0.4 offset is `.text+0x4b620`.
- Helper offsets currently used:
  - `find_pane_by_name_recursive`: `0x59970`
  - `pane_set_text_string`: `0x37a22f0`
- These helper offsets came from Rust Skyline/ui2d patterns already used by Smash plugin projects, not from new C/C++ code.

## Repo State

- Initial milestone commit `9837b0e` was pushed to `origin/main`.
- Tag `latest` was pushed at `9837b0e`.
- GitHub CLI is now set up in the user's environment.
- Current working tree changes clean up the `DebugText`/`Visual` refactor into separate modules.
- Local/editor/generated files are ignored:
  - `target/`
  - `.vscode/`
  - `.specstory/`
  - `.cursorindexingignore`
  - copied Skyline artifacts such as `*.nro`

## Useful Commands

```sh
cargo skyline check
cargo skyline build --release
```

Release artifact:

```text
target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro
```

Runtime log:

```text
sd:/smash-gamepad-overlay.log
```

## Likely Next Steps

- Revisit a custom `layout.arc`/BFLYT injection approach similar to Training Modpack for real visual HUD panes.
- Ensure that custom visual panes use the `minimal_gamecube` pane names or update the built-in skin constants.
- Keep Switch testing conservative; text-box flag changes can freeze at match start.
