# Agent Summary

## Goal

Maintain a Rust-only cargo-skyline plugin that renders a P1 input overlay inside Super Smash Bros. Ultimate matches. The current stable baseline is the square-pane visual overlay plus DebugText fallback.

## Current Implementation

- Plugin crate: `smash-gamepad-overlay`.
- Entry point: `src/lib.rs`.
- The implementation is split into small modules:
  - `config`: runtime constants and display mode selection.
  - `input`: HID polling, raw snapshots, logical controls, and view-state mapping; exports `poll_view_state_now()` and `ControllerViewState::from_optional_snapshot()` so the draw and non-draw paths share a single snapshot-to-view conversion.
  - `ui`: match-layout draw dispatch plus draw-path ui2d helper wrappers (`find_pane_by_name`, `set_textbox_text`).
  - `hud`: experimental non-draw visual capture/update path for Training Modpack coexistence.
  - `debug_text`: text fallback for the normal draw path.
  - `visual` and `skin`: non-text pane renderer, draw-path visual cache lifecycle hooks, and built-in skin data; skin element constructors share a `BLANK_ELEMENT` const via struct-update syntax.
  - `pane_utils`: shared `pane_name_matches(*mut Pane, &[u8])` and `cstr_bytes_to_str(&[u8])` helpers used by `visual`, `debug_text`, `ui`, and `hud`.
  - `offsets` and `logger`: hook resolution and diagnostics. `logger::log_startup_banner(StartupBanner { .. })` emits the multi-line startup trace block from one call site.
- `build.rs` generates build metadata only. The patched layout is installed as a normal data replacement.
- Uses Rust + Skyline only.
- Hooks `nn::ui2d::Layout::Draw` by scanning `.text` for a known instruction signature when Training Modpack is not present.
- When Training Modpack compatibility is detected, SGPO skips `Layout::Draw` and installs a non-draw HUD capture/update path. Compatibility detection checks the standard Training Modpack NRO path, `*training*modpack*.nro` files in the Skyline plugin folder, and the force flag `sd:/ultimate/mods/smash-gamepad-overlay/FORCE_TRAINING_MODPACK_COMPAT`.
- Training Modpack compatibility requires `local-assets/modified/info_melee/layout.arc` to be installed as a normal Smash data replacement at `ui/layout/info/info_melee/info_melee/layout.arc`.
- Only draws when the current layout name is `info_melee`, so it appears in matches rather than menus/training-only UI.
- Polls P1 controller state using `skyline::nn::hid`.
- Tries Npad No1 first, then handheld Npad ID `0x20`.
- Supports FullKey, handheld, Joy-Con styles, and GameCube state.
- Shows P1 ID/style, pressed buttons, left stick, right stick, and GameCube analog trigger values when available.
- Has two display modes:
  - `DebugText`: original compact troubleshooting text.
  - `Visual`: skin-driven generic pane renderer.
- `Visual` targets the active `minimal_debug` square pane skin under `sgpo_root`.
- The modified match layout provides 24 child `sgpo_pro_*` picture panes for face buttons, shoulders/triggers, stick clicks, plus/minus, d-pad cardinals/diagonals, stick gates, and stick dots.
- `sgpo_pro_a_marker` keeps the previously tested A-button pane name.
- Pressed buttons dim/brighten and scale; stick dots move from normalized stick positions.
- `Visual` falls back to `DebugText` if injected panes are missing on the normal draw path.
- `visual::VISUAL_CACHE` (a `VisualCache` static) caches resolved `sgpo_root` and `SkinElement` pane pointers per `info_melee` layout/root pointer pair. Access is serialized via a `VisualRuntimeGuard` spin lock so the draw hook and the match start/end reset hooks cannot race; the wrapper struct that used to own this cache was removed in favor of free `render_into_cache`/`resolve_into_cache` functions.
- If the `info_melee` root changes, or cached pane metadata is not valid, the visual cache re-resolves once for the new root.
- On 13.0.4, the normal draw path also installs match start/end reset hooks so the visual cache is cleared between recreated match HUD layouts even if Smash reuses an allocator slot.
- If required visual panes are missing, the missing result is cached for that root and the UI path falls back to DebugText without repeated visual pane searches.
- DebugText fallback now caches its text panes per root, uses a narrower text-pane candidate list, and rejects panes that do not look like usable textboxes.
- `hud::HudVisualRuntime` separately caches panes found through captured P1 HUD parts layout data so the visual skin can be updated without `Layout::Draw`; it keeps slots for both P1 HUD parts variants (`p1` and `p1_2`) because one can be hidden depending on match HUD mode.
- `hud::HudVisualRuntime` access is guarded by a small spin lock because Training Modpack mode can touch it from HUD capture, scene-update, and match reset hooks.
- Training Modpack pane capture happens from the HUD set-info-alpha hook; live input updates are driven from the scene-update hook because the capture hook does not fire continuously enough for input display. Cached panes are revalidated by expected pane name before update.
- The Training Modpack path logs missing visual panes and stays inactive instead of falling back to DebugText, because the text fallback requires the intentionally skipped draw hook.
- The Training Modpack path currently places the skin in P1/P1_2 HUD-local space because that follows the player HUD pause visibility behavior. It cannot reach true bottom-right through this path without being clipped by the P1 HUD parts container.
- The current Training Modpack-compatible placement is intentionally P1-adjacent and lowered so it is less likely to cover match action. Tune `TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG` and `TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG` for non-training placement, and `TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG` / `TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG` for Training-mode-only placement.
- In the Training Modpack path, SGPO renders in non-training matches and in Smash Training mode by default. It suppresses itself only in Training mode when `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD` exists.
- Skin abstraction added:
  - `ControlId`: logical controls/buttons/sticks/triggers.
  - `ControllerViewState`: maps `ControllerSnapshot` into pressed/released values plus normalized stick/trigger values.
  - `SkinElement`: maps a control to a pane name, optional source image/material names, base position, size, alpha/visibility/scale states, and optional x/y stick movement range.
  - Active built-in skin: `minimal_debug`.
  - Inactive built-in skin target: `switch_pro_alt_builtin`, which mirrors the RetroSpy `switch-pro-alt` Switch layout as data only.
- `ControlId` is `#[repr(u8)]`; a compile-time assertion keeps `LOGICAL_CONTROL_COUNT` synchronized with the enum.
- Skin/layout mismatch logging reports the active skin, first missing pane, expected layout flavor, and the regeneration/staging commands when patched panes do not match `ACTIVE_SKIN`.
- Active skins with more than `MAX_RESOLVED_SKIN_ELEMENTS` are rejected explicitly before pane resolution.
- Custom-skin direction:
  - the plugin should not parse arbitrary PNGs or construct complete visual assets at runtime;
  - a future PC-side converter should read RetroSpy-style `skin.xml` plus PNG assets;
  - that converter should generate the patched Smash `layout.arc` panes/materials/textures and a matching skin manifest/table;
  - the plugin should then continue to update already-existing named panes by `ControlId`.
- Writes diagnostics to `sd:/smash-gamepad-overlay.log`.
- Startup logs include a build ID so stale installed NROs can be identified.
- Build IDs use `git rev-list --count HEAD` as the `c*` change count plus a local Cargo build counter. The local build counter lives under `target/` and resets after `cargo clean`.

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
- Training Modpack also scans/hooks `Layout::Draw`, so this plugin enables compatibility mode and skips installing its draw hook when Training Modpack is detected or forced.
- Training Modpack also scans the layout-arc handoff. Earlier embedded-layout experiments conflicted with that, so SGPO now relies on a normal data replacement for the patched `info_melee/layout.arc`.
- Experimental non-draw rendering initially showed all root-level SGPO panes white and not updating. The fix was to:
  - set injected panes hidden by default with alpha `0`;
  - inject a second SGPO pane tree into `info_melee_lct_player_00.bflyt` and `info_melee_lct_player_01.bflyt`;
  - capture P1/P1_2 HUD parts layout data and root match HUD layout data when available;
  - apply current input state from the scene-update hook after pane capture so inputs update every frame.
- The non-draw path is still new and should be treated as experimental until tested with Training Modpack enabled. If placement is wrong, adjust `TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG`.
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
- `tools/patch_info_melee_layout.py --self-test` validates the source BFLYT header/section walk and the `pic1` field offsets used for vertex color, material index, and texture coordinate count before patching. The self-test and the real patch run both go through one shared `load_bflyt()` helper, so the self-test exercises the same parser used in production.
- `tools/patch_info_melee_layout.py` refuses source/destination combinations where one path contains the other, to avoid deleting local source assets before copying.
- `tools/stage_arcropolis_layout.py` stages the generated layout at `target/arcropolis/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc` for copying into `sd:/ultimate/mods/`.
- `tools/stage_arcropolis_layout.py` validates every pane name in the generated active skin before copying the layout.
- `tools/stage_arcropolis_layout.py` also supports optional local emulator deployment through a gitignored `.env`:
  - `SGPO_DEPLOY_EMU=1` enables emulator copies.
  - `SGPO_EMU_PLUGIN_DIR` receives `target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro`.
  - `SGPO_EMU_MODS_DIR` receives `smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc`.
  - `SGPO_EMU_LOG_PATH` can point at the emulator `sdmc/smash-gamepad-overlay.log` for quick reference.
  - Use `--no-emu` to force target-only staging, or `--skip-nro` for layout-only emulator staging.
- Machine-specific emulator paths belong in the local ignored `.env`, not in git.
- `tools/analyze_retrospy_skin.py` parses RetroSpy-style `skin.xml`, emits repo-safe dry-run manifest/report files under `target/`, and validates the generated manifest against `switch_pro_alt_builtin`.
- `tools/tests/test_analyze_retrospy_skin.py` covers the analyzer's Rust-source parsers with fixtures plus live-source backstops for `src/input.rs` and `src/skin.rs`.
- Next converter work should stay narrow: prove one PNG-backed pane end to end before generating a full skin.
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

## Relevant Offsets And Sources

- `Layout::Draw` is found dynamically by signature scan.
- The tested 13.0.4 offset is `.text+0x4b620`.
- Non-draw HUD path references HDR/local-latency-slider-style hooks on Smash 13.0.4:
  - set-info-alpha HUD capture/update: `0x1b6cc08`
  - scene update: `0x3747b7c`
  - match start reset: `0x1345558`
  - match end reset: `0x1d68b94`
  - layout-data pane lookup helper: `0x3776360`
- Training-mode detection calls `app::smashball::is_training_mode` by mangled symbol. Treat it as version-sensitive like the numeric offsets and re-check it when updating supported Smash versions.
- Normal draw-path ui2d helper offsets are still absolute and are therefore gated to Smash display version `13.0.4` before installing `Layout::Draw`:
  - `find_pane_by_name_recursive`: `0x59970`
  - `pane_set_text_string`: `0x37a22f0`

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
- Next skin milestone should be a one-pane PNG-backed proof for `sgpo_alt_face_a`, generated into ignored local output only, with `minimal_debug` still active.
