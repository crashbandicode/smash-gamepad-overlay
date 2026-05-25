# Agent Summary

## Goal

Build the first milestone of a Rust-only cargo-skyline plugin that renders a P1 input overlay inside Super Smash Bros. Ultimate matches.

## Current Implementation

- Plugin crate: `smash-gamepad-overlay`.
- Entry point: `src/lib.rs`.
- Uses Rust + Skyline only.
- Hooks `nn::ui2d::Layout::Draw` by scanning `.text` for a known instruction signature.
- Only draws when the current layout name is `info_melee`, so it appears in matches rather than menus/training-only UI.
- Polls P1 controller state using `skyline::nn::hid`.
- Tries Npad No1 first, then handheld Npad ID `0x20`.
- Supports FullKey, handheld, Joy-Con styles, and GameCube state.
- Shows P1 ID/style, pressed buttons, left stick, right stick, and GameCube analog trigger values when available.
- Writes diagnostics to `sd:/smash-gamepad-overlay.log`.

## Tested Context

- User tested on emulator and Switch.
- Smash display version reported by plugin: `13.0.4`.
- Draw signature resolved to `.text+0x4b620`.
- The plugin boots and draws on Switch after removing risky text-box flags.
- Current display is one compact line because only one reusable text pane is being found.

## Issues Encountered

- Initial draw signature lookup failed when Training Modpack was enabled.
- Training Modpack also scans/hooks `Layout::Draw`, so this plugin now detects the standard Training Modpack NRO path and skips installing its hook when present.
- Some text-box flags caused a Switch freeze right before the match countdown. Those were removed.
- `cargo skyline listen` was not reliable for logs in the user's setup, so file logging was added.
- Attached Joy-Cons initially reported as controller not ready; fallback polling for handheld Npad ID was added.

## Relevant Offsets And Sources

- `Layout::Draw` is found dynamically by signature scan.
- The tested 13.0.4 offset is `.text+0x4b620`.
- Helper offsets currently used:
  - `find_pane_by_name_recursive`: `0x59970`
  - `pane_set_text_string`: `0x37a22f0`
- These helper offsets came from Rust Skyline/ui2d patterns already used by Smash plugin projects, not from new C/C++ code.

## Repo State

- Intended first-commit files are staged:
  - `.gitattributes`
  - `.gitignore`
  - `AGENTSUMMARY.md`
  - `Cargo.lock`
  - `Cargo.toml`
  - `README.md`
  - `src/lib.rs`
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

- Find a safer way to allocate or clone dedicated overlay text panes so the display can be multi-line.
- Replace the compact debug text with a bottom-right controller layout.
- Keep Switch testing conservative; text-box flag changes can freeze at match start.
