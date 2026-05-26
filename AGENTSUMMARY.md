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
  - `visual` and `skin`: non-text pane renderer and built-in skin data.
  - `layout_inject`: inline hook that swaps in the modified `info_melee` `layout.arc`.
  - `offsets` and `logger`: hook resolution and diagnostics.
- `build.rs` embeds `local-assets/modified/info_melee/layout.arc` when it exists. The local dump/extracted assets stay ignored.
- Uses Rust + Skyline only.
- Hooks `nn::ui2d::Layout::Draw` by scanning `.text` for a known instruction signature.
- Hooks the layout-arc handoff by scanning for the Training Modpack/HDR-style `layout.arc malloc handoff` signature.
- Only draws when the current layout name is `info_melee`, so it appears in matches rather than menus/training-only UI.
- Polls P1 controller state using `skyline::nn::hid`.
- Tries Npad No1 first, then handheld Npad ID `0x20`.
- Supports FullKey, handheld, Joy-Con styles, and GameCube state.
- Shows P1 ID/style, pressed buttons, left stick, right stick, and GameCube analog trigger values when available.
- Has two display modes:
  - `DebugText`: original compact troubleshooting text.
  - `Visual`: skin-driven generic pane renderer.
- `Visual` targets a minimal full Switch Pro Controller pane skin under `sgpo_root`.
- The modified match layout provides 24 child `sgpo_pro_*` picture panes for face buttons, shoulders/triggers, stick clicks, plus/minus, d-pad cardinals/diagonals, stick gates, and stick dots.
- `sgpo_pro_a_marker` keeps the previously tested A-button pane name.
- Pressed buttons dim/brighten and scale; stick dots move from normalized stick positions.
- `Visual` falls back to `DebugText` if injected panes are missing.
- `visual::VisualRuntime` caches resolved `sgpo_root` and `SkinElement` pane pointers per `info_melee` layout/root pointer pair.
- If the `info_melee` root changes, or cached pane metadata is not valid, the visual runtime re-resolves once for the new root.
- If required visual panes are missing, the missing result is cached for that root and the UI path falls back to DebugText without repeated visual pane searches.
- Skin abstraction added:
  - `ControlId`: logical controls/buttons/sticks/triggers.
  - `ControllerViewState`: maps `ControllerSnapshot` into pressed/released values plus normalized stick/trigger values.
  - `SkinElement`: maps a control to a pane name, base position, size, alpha/visibility states, and optional stick movement radius.
  - Built-in skin: `minimal_pro_controller_full`.
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
- Injected-pane visual path was tested by the user with the A marker: a white square appears and dims/brightens as expected when A is pressed/released.
- Expanded full Pro Controller visual skin was tested by the user and worked.

## Issues Encountered

- Initial draw signature lookup failed when Training Modpack was enabled.
- Training Modpack also scans/hooks `Layout::Draw`, so this plugin now detects the standard Training Modpack NRO path and skips installing its hook when present.
- Some text-box flags caused a Switch freeze right before the match countdown. Those were removed.
- `cargo skyline listen` was not reliable for logs in the user's setup, so file logging was added.
- Attached Joy-Cons initially reported as controller not ready; fallback polling for handheld Npad ID was added.
- Multi-pane visual rendering originally reused live HUD panes. That caused Hero MP gauge visibility and a Switch freeze.
- Cloning and appending textboxes also froze emulator, likely because raw `TextBox` cloning bypasses Smash/ui2d construction ownership.

## Layout Tooling And Pane Injection

- The user dumped `data.arc` locally. It is ignored and should not be committed.
- `smash-arc` was used to extract:
  - `ui/layout/info/info_melee/info_melee/layout.arc`
- Python `sarc` was used to unpack/repack the layout SARC:
  - unpacked original: `local-assets/original/info_melee/unpacked/`
  - modified output: `local-assets/modified/info_melee/layout.arc`
- `python -m sarc list/extract/create` plus small Python BFLYT inspection scripts were used to inspect `blyt/*.bflyt`, `anim/*.bflan`, and pane section counts.
- `strings`/`rg` were used to confirm injected pane names in the modified BFLYT.
- `docs/info-melee-layout-notes.md` records the discovered `info_melee` root layout structure and expected SGPO pane names.
- `tools/patch_info_melee_layout.py` performs the reproducible local patch:
  - copies the original unpacked layout tree to `local-assets/modified/info_melee/unpacked/`;
  - patches only `blyt/info_melee.bflyt`;
  - clones the existing `RootPane` as `sgpo_root`;
  - clones `set_rep_stock_01` picture panes as the programmer-art SGPO visual elements;
  - inserts `sgpo_root` before the root close section;
  - adds 24 child `sgpo_pro_*` picture panes under `sgpo_root`;
  - avoids BFLAN animation edits, `layout.info` edits, external skin files, and runtime pane allocation.
- Current custom pane tree:
  - `sgpo_root`
  - `sgpo_pro_lt`, `sgpo_pro_lb`, `sgpo_pro_rt`, `sgpo_pro_rb`
  - `sgpo_pro_minus`, `sgpo_pro_plus`
  - `sgpo_pro_l3`, `sgpo_pro_r3`
  - `sgpo_pro_ls_gate`, `sgpo_pro_ls_dot`
  - `sgpo_pro_rs_gate`, `sgpo_pro_rs_dot`
  - `sgpo_pro_du`, `sgpo_pro_dd`, `sgpo_pro_dl`, `sgpo_pro_dr`
  - `sgpo_pro_dul`, `sgpo_pro_dur`, `sgpo_pro_ddl`, `sgpo_pro_ddr`
  - `sgpo_pro_btn_y`, `sgpo_pro_btn_x`, `sgpo_pro_btn_b`, `sgpo_pro_a_marker`
- `sgpo_pro_a_marker` intentionally keeps the first known-good A-button pane name.
- `build.rs` embeds `local-assets/modified/info_melee/layout.arc` into the NRO if present, but `local-assets/` and `data.arc` stay ignored.

## Relevant Offsets And Sources

- `Layout::Draw` is found dynamically by signature scan.
- The tested 13.0.4 offset is `.text+0x4b620`.
- Helper offsets currently used:
  - `find_pane_by_name_recursive`: `0x59970`
  - `pane_set_text_string`: `0x37a22f0`
- Layout injection signature currently resolves to `.text+0x3774154` on the tested 13.0.4 setup.
- These helper offsets came from Rust Skyline/ui2d patterns already used by Smash plugin projects, not from new C/C++ code.

## Repo State

- Initial milestone commit `9837b0e` was pushed to `origin/main`.
- Tag `latest` was pushed at `9837b0e`.
- GitHub CLI is now set up in the user's environment.
- Current working tree includes the injected `info_melee` visual marker path and patch tooling.
- Public git should contain only Rust code, docs, and patch tooling. Do not commit `data.arc`, unpacked BFLYTs/BFLANs/BNTX, or modified `layout.arc`.
- Release assets should not include `layout.arc`; the alpha NRO is built from the user's local patched layout.
- `docs/alpha-validation-checklist.md` tracks the current manual alpha validation matrix.
- Local/editor/generated files are ignored:
  - `target/`
  - `data.arc`
  - `local-assets/`
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

- Polish the programmer-art visual layout after more Switch/emulator testing.
- Run the alpha validation checklist after cache/layout changes.
- Keep the A marker as the rollback/stability baseline.
- Keep Switch testing conservative; text-box flag changes can freeze at match start.
