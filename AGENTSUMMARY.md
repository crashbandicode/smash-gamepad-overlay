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
  - `hud`: experimental non-draw visual capture/update path for Training Modpack coexistence.
  - `debug_text`: current working text fallback.
  - `visual` and `skin`: non-text pane renderer and built-in skin data.
  - `layout_inject`: optional inline hook that swaps in the modified `info_melee` `layout.arc`.
  - `offsets` and `logger`: hook resolution and diagnostics.
- `build.rs` embeds `local-assets/modified/info_melee/layout.arc` only when `SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT=1` is set. The default build expects the patched layout to be installed as a normal data replacement.
- Uses Rust + Skyline only.
- Hooks `nn::ui2d::Layout::Draw` by scanning `.text` for a known instruction signature when Training Modpack is not present.
- Optionally hooks the layout-arc handoff by scanning for the HDR-style `layout.arc malloc handoff` signature only when built with `SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT=1`.
- When Training Modpack is detected at the standard Skyline plugin path, SGPO skips `Layout::Draw` and installs an HDR/local-latency-slider-style non-draw capture/update path.
- Training Modpack compatibility requires `local-assets/modified/info_melee/layout.arc` to be installed as a normal Smash data replacement at `ui/layout/info/info_melee/info_melee/layout.arc`; keep `SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT` unset with Training Modpack.
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
- `Visual` falls back to `DebugText` if injected panes are missing on the normal draw path.
- `visual::VisualRuntime` caches resolved `sgpo_root` and `SkinElement` pane pointers per `info_melee` layout/root pointer pair.
- If the `info_melee` root changes, or cached pane metadata is not valid, the visual runtime re-resolves once for the new root.
- If required visual panes are missing, the missing result is cached for that root and the UI path falls back to DebugText without repeated visual pane searches.
- `hud::HudVisualRuntime` separately caches panes found through captured P1 HUD parts layout data so the visual skin can be updated without `Layout::Draw`; it keeps slots for both P1 HUD parts variants (`p1` and `p1_2`) because one can be hidden depending on match HUD mode.
- The Training Modpack path logs missing visual panes and stays inactive instead of falling back to DebugText, because the text fallback requires the intentionally skipped draw hook.
- The Training Modpack path currently places the skin in P1/P1_2 HUD-local space because that follows the player HUD pause visibility behavior. It cannot reach true bottom-right through this path without being clipped by the P1 HUD parts container.
- The current Training Modpack-compatible placement is intentionally P1-adjacent and lowered so it is less likely to cover match action. Tune `TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG` and `TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG` for non-training placement, and `TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG` / `TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG` for Training-mode-only placement.
- In the Training Modpack path, SGPO renders in non-training matches and in Smash Training mode by default. It suppresses itself only in Training mode when `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD` exists.
- Skin abstraction added:
  - `ControlId`: logical controls/buttons/sticks/triggers.
  - `ControllerViewState`: maps `ControllerSnapshot` into pressed/released values plus normalized stick/trigger values.
  - `SkinElement`: maps a control to a pane name, base position, size, alpha/visibility states, and optional stick movement radius.
  - Built-in skin: `minimal_pro_controller_full`.
- Writes diagnostics to `sd:/smash-gamepad-overlay.log`.
- Startup logs include a build ID so stale installed NROs can be identified.
- `CHANGE_NUMBER` is tracked and starts at `0`; increment it with each commit. Runtime build IDs include the change number plus a local Cargo build counter.

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
- Cached visual pane lookup was tested by the user and worked on both Switch and emulator.

## Issues Encountered

- Initial draw signature lookup failed when Training Modpack was enabled.
- Training Modpack also scans/hooks `Layout::Draw`, so this plugin detects the standard Training Modpack NRO path and skips installing its draw hook when present.
- Training Modpack also scans the same layout-arc handoff. Installing this plugin's layout hook first makes Training Modpack fail with `Failed to find offset for LAYOUT_ARC_MALLOC`; installing it later appears to break Training Modpack's own `info_training` panes and can trigger `Could not find pane TrModInputLog`.
- Because that same-offset layout hook is not safe to chain, Training Modpack compatibility now leaves embedded layout injection out of default builds and expects a normal data replacement for the patched `info_melee/layout.arc`.
- Experimental non-draw rendering initially showed all root-level SGPO panes white and not updating. The fix was to:
  - set injected panes hidden by default with alpha `0`;
  - inject a second SGPO pane tree into `info_melee_lct_player_00.bflyt` and `info_melee_lct_player_01.bflyt`;
  - capture only P1/P1_2 HUD parts layout data;
  - use the scene-update hook from local-latency-slider (`0x3747b7c`) and apply current state immediately after pane capture.
- The non-draw path is still new and should be treated as experimental until tested with Training Modpack enabled. If placement is wrong, adjust `TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG`.
- A fresh build still logged `injected modified info_melee layout.arc`; because `main()` did not call the install function, default builds now compile the embedded layout hook out entirely. If that log line appears in a default build, there is probably another old SGPO NRO loaded.
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
  - patches `blyt/info_melee.bflyt`, `blyt/info_melee_lct_player_00.bflyt`, and `blyt/info_melee_lct_player_01.bflyt`;
  - clones the existing `RootPane` as `sgpo_root`;
  - clones `set_rep_stock_01` picture panes for the root layout;
  - clones `set_rep_01` picture panes for player-parts layouts, but swaps their material/vertex-color fields to match `set_rep_stock_01` so the Training Modpack path keeps the visible/pause behavior while avoiding the red player marker color;
  - inserts `sgpo_root` before the root close section;
  - adds 24 child `sgpo_pro_*` picture panes under `sgpo_root`;
  - sets injected panes to alpha `0` so stale/unupdated copies do not appear as white boxes;
  - avoids BFLAN animation edits, `layout.info` edits, external skin files, and runtime pane allocation.
- `tools/stage_arcropolis_layout.py` stages the generated layout at `target/arcropolis/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc` for copying into `sd:/ultimate/mods/`.
- `tools/stage_arcropolis_layout.py` also supports optional local emulator deployment through a gitignored `.env`:
  - `SGPO_DEPLOY_EMU=1` enables emulator copies.
  - `SGPO_EMU_PLUGIN_DIR` receives `target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro`.
  - `SGPO_EMU_MODS_DIR` receives `smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc`.
  - `SGPO_EMU_LOG_PATH` can point at the emulator `sdmc/smash-gamepad-overlay.log` for quick reference.
  - Use `--no-emu` to force target-only staging, or `--skip-nro` for layout-only emulator staging.
- Machine-specific emulator paths belong in the local ignored `.env`, not in git.
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
- `build.rs` embeds `local-assets/modified/info_melee/layout.arc` into the NRO only when `SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT=1`, but `local-assets/` and `data.arc` stay ignored. Do not enable this with Training Modpack.

## Relevant Offsets And Sources

- `Layout::Draw` is found dynamically by signature scan.
- The tested 13.0.4 offset is `.text+0x4b620`.
- Non-draw HUD path references HDR/local-latency-slider-style hooks on Smash 13.0.4:
  - set-info-alpha HUD capture: `0x1b6cc08`
  - scene update: `0x3747b7c`
  - match start reset: `0x1345558`
  - match end reset: `0x1d68b94`
  - layout-data pane lookup helper: `0x3776360`
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

Eden/WSL local log path:

```text
<eden>/user/sdmc/smash-gamepad-overlay.log
```

## Likely Next Steps

- Polish the programmer-art visual layout after more Switch/emulator testing.
- Run the alpha validation checklist after cache/layout changes.
- Keep the A marker as the rollback/stability baseline.
- Keep Switch testing conservative; text-box flag changes can freeze at match start.
