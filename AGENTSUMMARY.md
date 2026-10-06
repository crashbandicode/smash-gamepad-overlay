# Agent Summary

## Goal

Maintain a Rust-only cargo-skyline plugin that renders a P1 input overlay inside Super Smash Bros. Ultimate matches. The current stable baseline is the square-pane visual overlay plus DebugText fallback.

## Current Implementation

- Plugin crate: `smash-gamepad-overlay`.
- Entry point: `src/lib.rs`.
- The implementation is split into small modules:
  - `config`: runtime constants and display mode selection.
- `input`: HID polling, raw snapshots, controller-family detection, logical controls, and view-state mapping; draw and non-draw paths poll one `ControllerSnapshot` and convert it through `ControllerViewState::from_optional_snapshot()`.
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
- `Visual` targets the active `default_simple` square pane skin under `sgpo_root` by default.
- `default_simple_gamecube` is a matching no-PNG GameCube subset that reuses the same `sgpo_pro_*` panes while omitting controls the GameCube controller does not have.
- The modified match layout provides 24 child `sgpo_pro_*` picture panes for face buttons, shoulders/triggers, stick clicks, plus/minus, d-pad cardinals/diagonals, stick gates, and stick dots.
- `sgpo_pro_a_marker` keeps the previously tested A-button pane name.
- Pressed buttons dim/brighten and scale; stick dots move from normalized stick positions.
- `Visual` falls back to `DebugText` if injected panes are missing on the normal draw path.
- `visual::VISUAL_CACHE` (a `VisualCache` static) caches resolved `sgpo_root` and `SkinElement` pane pointers per `info_melee` layout/root pointer pair. Access is serialized via a `VisualRuntimeGuard` spin lock so the draw hook and the match start/end reset hooks cannot race; the wrapper struct that used to own this cache was removed in favor of free `render_into_cache`/`resolve_into_cache` functions.
- If the live `info_melee` root pointer changes, the visual cache re-resolves from that root. Cached pane addresses are forgotten when `Pane::Finalize` reports them; that check does not read the pane.
- On the supported display version, the normal draw path also installs match start/end reset hooks so the visual cache is cleared between recreated match HUD layouts even if Smash reuses an allocator slot.
- If required visual panes are missing, the missing result is cached for that root and the UI path falls back to DebugText without repeated visual pane searches.
- DebugText fallback now caches its text panes per root, uses a narrower text-pane candidate list, and rejects panes that do not look like usable textboxes.
- `hud::HudVisualRuntime` separately caches panes found through captured P1 HUD parts layout data so the visual skin can be updated without `Layout::Draw`; it keeps slots for both P1 HUD parts variants (`p1` and `p1_2`) because one can be hidden depending on match HUD mode.
- `hud::HudVisualRuntime` access is guarded by a small spin lock because Training Modpack mode can touch it from HUD capture, scene-update, and match reset hooks.
- Training Modpack pane capture happens from the HUD set-info-alpha hook; live input updates are driven from the scene-update hook because the capture hook does not fire continuously enough for input display. Scene update still writes cached panes that have not been finalized. Eviction itself does not read or hide panes. A 13.0.5 inline observer at `.text+0x57f10` (`Pane::Finalize`, byte-guarded) drops a cache entry when the retiring address matches its skin root, a cached child, or the draw-path layout root. Capture, scene update, draw, and Finalize may run on different game threads. The per-cache lock serializes pane use and eviction. Thread changes are counted, not treated as a fatal shutdown. Fresh HUD capture resolves panes before taking the cache lock and publishes only if no Pane::Finalize started during that resolve. Invalid and replaced slots are discarded without hiding. A missing HUD handle is not resolved from the remembered layout handle. Same-root skin swaps may still hide the previous skin. Capture rejects `PaneFlag::UserAllocated`. Debug text resolves its panes from the live draw root on each callback and does not keep a pane cache.
- The Training Modpack path logs missing visual panes and stays inactive instead of falling back to DebugText, because the text fallback requires the intentionally skipped draw hook.
- The Training Modpack path currently places the skin in P1/P1_2 HUD-local space because that follows the player HUD pause visibility behavior. It cannot reach true bottom-right through this path without being clipped by the P1 HUD parts container.
- The current Training Modpack-compatible placement is intentionally P1-adjacent and lowered so it is less likely to cover match action. Tune `TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG` and `TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG` for non-training placement, and `TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG` / `TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG` for Training-mode-only placement.
- In the Training Modpack path, SGPO renders in non-training matches and in Smash Training mode by default. It suppresses itself only in Training mode when `sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD` exists.
- Skin abstraction added:
  - `ControlId`: logical controls/buttons/sticks/triggers.
  - `ControllerViewState`: maps `ControllerSnapshot` into pressed/released values plus normalized stick/trigger values.
  - `SkinElement`: maps a control to a pane name, optional source image/material names, base position, size, alpha/visibility/scale states, and optional x/y stick movement range.
  - Active built-in no-asset skins: `default_simple` and `default_simple_gamecube`.
  - `minimal_debug` remains accepted as a compatibility alias for `default_simple`.
  - Generated-asset built-in skin targets: `switch_pro_alt_builtin`, which mirrors the RetroSpy `switch-pro-alt` Switch layout, includes the RetroSpy `background.png` shell as a static element, and uses a skin-specific root scale; and `gamecube_tron_builtin`, which mirrors the RetroSpy `gamecube-tron` layout.
- `ControlId` is `#[repr(u8)]`; a compile-time assertion keeps `LOGICAL_CONTROL_COUNT` synchronized with the enum.
- Skin/layout mismatch logging reports the active skin, first missing pane, expected layout flavor, and the regeneration/staging commands when patched panes do not match the selected built-in skin.
- Active skins with more than `MAX_RESOLVED_SKIN_ELEMENTS` are rejected explicitly before pane resolution.
- Runtime skin selection reads `sd:/ultimate/mods/smash-gamepad-overlay/config.json` at startup and on match start. The config selects a built-in skin by name or `active_skin: "auto"` with `default_skins.switch` and `default_skins.gamecube`. Missing/invalid config falls back to `default_simple`; arbitrary generated manifest elements are not loaded by the plugin yet.
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
- User tested on Smash display version `13.0.4`. The plugin now targets `13.0.5`; that port has not been confirmed on Switch or emulator yet.
- Draw signature resolved to `.text+0x4b620` on 13.0.4. That address did not move in the 13.0.5 port, so the same signature is still expected.
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
- `tools/analyze_retrospy_skin.py` parses RetroSpy-style `skin.xml`, emits repo-safe dry-run manifest/report files under `target/`, and validates the default Switch Pro manifest against `switch_pro_alt_builtin`. It can also generate other manifests without built-in validation; `gamecube_tron_builtin` currently uses the `gamecube` section and `sgpo_gct_*` pane names.
- `tools/tests/test_analyze_retrospy_skin.py` covers the analyzer's Rust-source parsers with fixtures plus live-source backstops for `src/input.rs` and `src/skin.rs`.
- `tools/sgpo_installer/` is the preferred Rust user-facing installer. It accepts a user-owned `data.arc` or extracted `info_melee/layout.arc`, prepares the patched square-pane baseline, optionally parses RetroSpy preset skins, uses `nx-layout-toolbox` as a library to generate PNG-backed panes/textures/materials, validates generated panes, builds/stages `layout.arc`, writes config, backs up installed layout/config/NRO, and copies the NRO. It also supports `--sd-zip <path>` plus `--no-sd-stage` to build an SD-root archive containing the NRO, ARCropolis layout, and config. With no `--include-skin`, it installs only the no-PNG `default_simple` skins. Texture import defaults to known-good `bc7-srgb`; `--texture-format rgba8-srgb` can be used for uncompressed texture quality tests, though it did not materially improve the tested `switch-pro-alt` lettering. Current local dependency path points at `local-checkouts/Toolbox-Cli`; update it to a crates.io dependency after `nx-layout-toolbox` is published.
- `tools/build_skin_layout.py` remains as the legacy/reference Python implementation of the PNG-backed layout build. Keep it around for comparison while the Rust installer is hardened.
- `tools/sgpo_skin_tool.py` remains as a compatibility wrapper for older commands; it now forwards arguments to the Rust installer.
- `docs/release-bundle-strategy.md` records the intended release shape: ship the NRO plus PC-side tooling, require user-owned `data.arc`, and optionally accept RetroSpy skin folders.
- The BNTX corruption bug was traced to Toolbox-Cli rebuilding `_DIC` in string-pool order instead of BRTI texture order. Fixed Toolbox-Cli commit: `b4fe9ec Fix BNTX _DIC rebuild to follow texture order, not string order`.
- `tools/tweak_bflyt_material.py` is a narrow BFLYT helper for the one-pane proof. It can list materials, list `pic1`/`txt1` pane material bindings, verify a pane binding, rename a material slot, and rebind a pane to a material without changing section sizes.
- `tools/diff_one_pane_proof.py` compares the current square-pane layout against `local-assets/proof/switch-pro-alt-one-pane/unpacked/` and writes `target/layout-inspection/one-pane-proof-diff.md`. It confirms the proof pane, material, `txl1` entry, pane-material binding, material texture reference, and BNTX texture-name presence, but does not decode or write BNTX texture payloads.
- Current generated skins:
  - `default_simple`: `sgpo_pro_*` square panes, Switch-family controls.
  - `default_simple_gamecube`: `sgpo_pro_*` square panes, GameCube subset.
  - `switch_pro_alt_builtin`: `sgpo_alt_*` panes from RetroSpy `switch-pro-alt`.
  - `gamecube_tron_builtin`: `sgpo_gct_*` panes from RetroSpy `gamecube-tron`.
- Config hot-swap is match-start based: edit `sd:/ultimate/mods/smash-gamepad-overlay/config.json` and start a new match. The plugin still does not parse arbitrary skin manifests or PNGs at runtime.
- `active_skin: "auto"` selects `default_skins.switch` for Switch/Pro/Joy-Con/handheld controller styles and `default_skins.gamecube` for GameCube controller style. The installer writes `default_simple` / `default_simple_gamecube` when no optional RetroSpy skins are included, or generated defaults for presets that are included. The runtime checks the current P1 controller family while rendering, so a mid-match family change switches skins and clears the previous skin's panes. The non-draw HUD path resolves the new skin from the cached `sgpo_root` pane when possible, because cached HUD layout-data handles can stop resolving `sgpo_root` after controller-family changes. If a swap cannot resolve, SGPO keeps the previous skin cache instead of blanking the overlay.
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

Supported Smash display version: `13.0.5`.

How the 13.0.4 addresses were carried to 13.0.5:

- There is no in-repo writeup of the original 13.0.4 hunt. `Layout::Draw` has always been a `.text` signature scan (`src/offsets.rs`). The other hooks were absolute addresses in the HDR / local-latency-slider style, first added in `ba52ef4`.
- Smashline `fdc151f` and HewDraw-Remix `ea1eb702` ("13.0.5 offsets") are the public ports. Their before/after pairs show piecewise `.text` slides, not one global delta.
- Early code through about `0x14f99cc` did not move. A short online/packet region around `0x16b0000`–`0x17e0000` moved backward, then the net slide returned to zero by `0x184de10` and stayed zero through at least `0x1ee9edc`. From about `0x30f6e00` through `0x38f86a0`, code moved by `+0x5B0`. Data pointers near `0x4f80000` and later moved by `+0x1000`.
- Two SGPO helpers are the same sites HDR updated: layout pane lookup `0x3776360 -> 0x3776910`, and `set_text_string` `0x37a22f0 -> 0x37a28a0`.

Current 13.0.5 addresses:

- `Layout::Draw` is found dynamically by signature scan. The expected address is still `.text+0x4b620`. `LEGACY_DRAW_OFFSET` is only the diagnostic fallback.
- Non-draw HUD path:
  - set-info-alpha HUD capture/update: `0x1b6cc08` (unchanged)
  - scene update: `0x374812c` (`0x3747b7c + 0x5B0`)
  - match start reset: `0x1345558` (unchanged)
  - match end reset: `0x1d68b94` (unchanged)
  - layout-data pane lookup helper: `0x3776910`
- Training-mode detection calls `app::smashball::is_training_mode` by mangled symbol. Treat it as version-sensitive like the numeric offsets and re-check it when updating supported Smash versions.
- Normal draw-path ui2d helper offsets are still absolute and are therefore gated to Smash display version `13.0.5` before installing `Layout::Draw`:
  - `find_pane_by_name_recursive`: `0x59970` (unchanged)
  - `pane_set_text_string`: `0x37a28a0`

These 13.0.5 addresses are ported from the public hook map. They still need one on-device or emulator log from display version `13.0.5` showing the draw signature hit and the non-draw scene-update line. The local `data.arc` notes remain a 13.0.4 dump; this update did not require a new layout archive.

## Repo State

- Initial milestone commit `9837b0e` was pushed to `origin/main`.
- Tag `latest` was pushed at `9837b0e`.
- GitHub CLI is now set up in the user's environment.
- Current working tree includes the injected `info_melee` visual marker path and patch tooling.
- Public git should contain only Rust code, docs, and patch tooling. Do not commit `data.arc`, unpacked BFLYTs/BFLANs/BNTX, or modified `layout.arc`.
- Release assets should not include `layout.arc`; the installer builds it from the user's local `data.arc` or extracted layout.
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

- Confirm the 13.0.5 port on Switch or emulator: display version `13.0.5`, `Layout::Draw` at `.text+0x4b620`, and a non-draw scene-update log line.
- Polish the programmer-art visual layout after more Switch/emulator testing.
- Run the alpha validation checklist after cache/layout changes.
- Keep the A marker as the rollback/stability baseline.
- Keep Switch testing conservative; text-box flag changes can freeze at match start.
- Next packaging milestone should be publishing/packaging the Rust installer so `data.arc` plus optional skins are the normal user-facing input path without Python.


## 2026-10-06 Switch compatibility investigation checkpoint

This supersedes assumptions of native stability for the current 13.0.5 port.
Repository HEAD31994bc and pre-existing dirty port preserved; no SGPO source changes
made by this investigation. Rewrite worktree /home/intpa/ssbu-online-deluxe-rewrite
main/e57cebe62a60c714c1c05020a3c515b22e40fa1a contains the main HANDOFF.md and
docs/SWITCH_CRASH.md. No push authorized for this investigation.

Native NROa64d4c87dcc84d95b122a3327d8ad37957f616e0aa2f9e1c6c7e0bdb0fa5b5f6
had a proven invalid cached-pane read at SGPO+0xbb00/pane_name_matches, via
ResolvedHudSkin::is_valid/update_runtime. Exact ELF executable spans matched.
Do not dereference a remembered pane name as proof its lifetime is valid; the
invalid-cache and replacement paths also hide through old raw pointers.

NRO-only removal allowed one match but still crashed returning from results to CSS:
main+0x5986c reads pane.ext_user_data_list+8 through value0x2000000.
Training draw caller alone does not establish cause. The subsequently removed
HUD asset was SHA82a674e7f218f318fdc5d1834d4473f08934d8fba810a8da904f80a36de0e7df,
size4,795,344, matching local-assets/generated/sgpo-skins-rs-emu-bc7-final/layout.arc.
With BOTH NRO and this asset absent, operator completed five match/results/CSS cycles.
Six remaining plugins, updated Skyline/ARC411, Training7.1.1 and rewrite stayed unchanged.
Rewrite was Vanilla/FXOn; render_backend not-installed, zero LessLag/Ultra hook counts.
This supports asset involvement but is not a root-cause fix or complete exoneration.

The Switch currently keeps this successful six-plugin setup. Disabled files are at
sd:/backup-deluxe-rewrite-20261006-114001/isolation-sgpo/{libsmash_gamepad_overlay.nro,layout.arc}.
Do not restore while Smash is running or without a concrete isolation/repair test.
No changes made to the user's main Eden profile in this investigation.
FTP192.168.0.226:5000 anonymous works while console awake; server can return transient
451/450 responses, recorded in evidence. Do not print/store credential fields.

Evidence folders:
- /home/intpa/.local/share/ssbu-mod-backups/switch-crash-20261006-121423
- /home/intpa/.local/share/ssbu-mod-backups/switch-ftp-after-isolation-20261006
- /home/intpa/.local/share/ssbu-mod-backups/switch-rewrite-deploy-20261006-114001

Static comparison to original/unpacked finds only three BFLYTs + __Combined.bntx changed;
all stock pane bodies in those three BFLYTs match, existing usd1 counts preserved.
Source marker panes have no adjacent usd1; copied pane user-data bytes are zero.
Fable's speculative missing-usd1/special-flag attribution was rejected on actual full
control flow; see review-acceptance.txt before relying on fable-followup.md.
The file original/info_melee/layout.arc has zeroed entries unlike original/unpacked;
do not use that archive blindly as a canonical control. No original asset was modified.

Next: finish exact deployed BNTX/resource audit, then test minimal asset variants before
restoring the NRO. Keep Nintendo assets and compiled binaries outside Git.

## 2026-10-06 candidate pane-lifetime change

Not native-accepted. Root supplied the boundary: observe `Pane::Finalize` at
`.text+0x57f10` before the original prologue, guarded by display version 13.0.5
and the first 16 instructions (64 bytes) from main build `21450C64...`. The inline
hook does not change registers. Both the non-draw HUD cache and the ordinary
draw visual cache forget matching entries from the address alone. They do not
read `pane.name`, hide through a retiring pointer, or dereference `layout_data`
while evicting. Scene update remains the live-input path. When Training Modpack is detected, SGPO still skips its own Layout::Draw hook.

If the version/byte guard fails, SGPO does not cache panes and logs that
explicitly. Capture, update, and render may run on different threads; a thread
change is an observation and does not disable the cache. Guard failure is a
diagnostic, not a passing native test. `Layout::Finalize` at `.text+0x48e50` is a second guarded counter
only; it does not invalidate unrelated layouts. The finalizer records counters
and flushes them from later frame or match-reset callbacks. A bounded address
watchlist survives match reset until Finalize reports those integers. Overflow
means coverage is incomplete. The same pane address seen with a different
layout handle is an observation, not a proven stale-pointer violation.
Replaced slots are not hidden, so a dropped layout can keep its last alpha
until the game finalizes those panes. A missing-handle skin change waits for
the next set-info-alpha capture. The separate post-results crash with the NRO
absent is not claimed fixed.


## 2026-10-06 root CLI acceptance checkpoint: candidate2

Grok implemented root's reviewed correction; Fable reviewed the threading
design. Five regular Kirby/Wario Small Battlefield match/results/CSS cycles
passed in isolated Eden with the full HUD archive, Training7.1.1, rewrite and
updated Skyline/ARCropolis loaded. Input overlay press/release remained live.
Sixth match start flushed stored=110, retired=110, live=0, overflow=false.
This is emulator coverage, not native acceptance or an asset-only crash fix.

Tested NRO: 3a746514f3895da32374d1a7d1c5b9b7300c30b6ed08686cea480434180f251d.
Debug ELF: 8ce4b120fb675b89fa4f283efbc8de0db5ab40f5ec3b4355a6000d2409fc03d5.
Artifacts/logs/screenshots/receipt:
~/.local/share/ssbu-mod-backups/switch-ftp-after-isolation-20261006/sgpo-lifetime-candidate2/.
Eleven metadata tests, helper Clippy, Skyline check/release build passed.
Full host Clippy and no-default-features build are blocked by nnsdk E0554;
Skyline Clippy component absent. Do not mark every Cargo gate complete.

Run ended at the 1200-second supervisor deadline while a stop confirmation
was pending. 8 GiB peak, zero OOM; no guest crash observed in completed cycles.
Owned Eden stopped; isolated mod/config paths restored. Console remains stable
with SGPO NRO AND HUD archive disabled; no candidate deployed, committed or pushed.
Root handoff is /home/intpa/ssbu-online-deluxe-rewrite/HANDOFF.md, including
the 10 GiB future-run allowance (recheck RAM first) and exact remaining limits.
Training mode/menu/savestate on candidate2, standalone draw, DebugText,
live skin changes, GameCube skin and native acceptance remain untested.


## 2026-10-06 native probe pre-deployment checkpoint
Diagnostic feature implementation and root/Fable review complete. NRO
20e9e062c9ace6230b3fadda6fb9a1ffdb76659479f3b32de0f1100e9e72a494,
ELF c4905b30f3496c4c7d50db499045366e3a59f215178f00b5f405e822c279611d.
Artifacts, source patch, untracked-source snapshot and reports archived under
switch-ftp-after-isolation-20261006/native-userdata-probe/. Feature Switch-target
Clippy -D warnings and release pass; root 17 metadata tests pass.
Grok/Fable wrappers stopped; no native acceptance yet, no commit/push.
Eden smoke run20261006-124712-567461, unit eden-control-283c36ecc7e6, 10GiB,
600s deadline16:57:12Z. Probe-only byteguard armed, header written, regular Smash
loading for first callback sample. Profile journal userdata-probe-eden/journal.json
must be restored AFTER owned Eden exit via audittemp/userdata-probe-stage.py restore.
User confirmed Smash fully closed, Switch HOME/FTP. Next authorized mutation:
audittemp/deploy-userdata-probe.py --nro <archived artifact> --sha256 <above>.
Installs marker, exact known HUD asset82a674..., NRO last, verified FTP hashes;
six baseline plugins untouched. Rollback moves those three originally-absent
paths to recorded SD backup. Actual deployment receipt will be deployment.json.
No Switch mutation yet at this checkpoint. First native test is observer-only:
NO visible overlay, replay regular match and results-to-CSS, then collect logs.
User should not relaunch after crash before collection (probe log truncates).

## 2026-10-06T17:01Z diagnostic deployed, native user test pending
FTP deployment completed and read-back SHA256 verified:
- SGPO diagnostic NRO20e9e062c9ace6230b3fadda6fb9a1ffdb76659479f3b32de0f1100e9e72a494.
- HUD asset82a674e7f218f318fdc5d1834d4473f08934d8fba810a8da904f80a36de0e7df.
- Empty PROBE_ONLY marker in ultimate/mods/smash-gamepad-overlay.
Six baseline plugins/settings retained. SD journal/rollback destination:
sd:/backup-deluxe-rewrite-probe-20261006-165836/deployment.json.
Local receipt: switch-ftp-after-isolation-20261006/native-userdata-probe/deployment.json,
state installed_verified. First attempt timed out during active HUD readback;
fresh FTP confirmed correct HUD hash, then NRO was uploaded/verified/renamed last.
All three originally absent. Rollback must move (not delete originals) these three
known candidate paths back into the new SD backup after verifying hashes.
User asked to launch Kirby-v-Wario regular match, finish and return results-to-CSS.
No overlay expected; observer-only. Do not relaunch after crash before collecting
sd:/smash-gamepad-overlay-probe.log (truncated on next launch) and creport.
No Switch acceptance yet, no commit/push.

Eden smoke reached regular CSS, guard armed and header written; callback sample
NOT yet observed. Run hit its configured600s runtime bound, not a guest crash.
Peak8087699456B, memory.events max/oom/oom_kill all0. Profile journal restored.
Run/log/screenshots saved in userdata-probe-eden. Explicit limit: do not describe
this as full-match probe verification. Candidate2 prior five-cycle run is separate.
Model watcher2082660 and delivery watcher2108638 still active; delivery state watching.

## 2026-10-06T17:16Z native probe-only first cycle passed
User reports regular match returned to CSS without crashing, then confirmed
Smash fully closed and Switch HOME. Captured native dedicated probe + main logs:
native-userdata-probe/capture-171527/. Real sample at LR main+5f540 (same call site
as prior crash), pane name base, ext_userdata0, flags81, flagEx10, flagsword0.
No suspect record. This proves native observer callback and persistent log work;
one clean cycle does not resolve intermittent asset fault.
Next authorized write: move empty active PROBE_ONLY marker into existing SD
backup as PROBE_ONLY.completed-probe-test, verify active marker absent and
same diagnostic NRO20e9e... intact. Same NRO then runs corrected overlay+probe.
User will repeat regular match/results/CSS, check overlay input responsiveness,
then several cycles if clean. Native acceptance still pending.

Eden probe-only second run20261006-130448-299c44 completed Zelda-v-IceClimbers
SmallBattlefield 3stock match and results-to-CSS, no guest crash; no getter sample.
Clean game.stop confirmed Yes, powered_off, ui.close exit0 at550.57s of600s bound.
Peak7909486592B, no memory cap or OOM events. Journal userdata-probe-eden2 restored;
logs/screenshots/run/control preserved there. Original first run only reached CSS.
No running owned Eden. No commits/push. Awaiting updated Eden delivery marker.

## 2026-10-06T17:21Z corrected overlay native test active
Removed only PROBE_ONLY marker after user confirmed Smash closed.
native-userdata-probe/enable-overlay.json state overlay_plus_probe_ready; marker
preserved on SD backup. NRO hash reverified, same20e9e... build. Seven plugins.
Live main log overlay-capture-172031 confirms both lifetime guards matched,
TM compatibility non-draw path selected, scene hook fired, match-start reset,
skin captured. User testing controls + results/CSS now; do not mutate SD.
Detailed probe remains open during game and may reject FTP reads until closure.
Fable6a30 thread resumed same name switch-crash-fable-1006, Fable5.1/xhigh
danger-full-access transport with explicit read-only audit scope. Investigating
native getter sample vs Eden header-only to identify exact branch predicates.
8min firstpass, send1200s deadline~17:40Z, functions exec cell1951 pending.
Report audittemp/fable-native-emu-probe-path.md. No builds, repo/FTP/Eden edits.
Original Grok wrapper remains stopped. New Eden watcher still waiting.


## 2026-10-06T17:29Z initial corrected-overlay Switch validation passed
User completed TWO matches (Kirby, then Palutena), confirmed live overlay input
updates and both returns to CSS, then closed Smash. User declined further repeated
matches; two is accepted for this checkpoint. Do not ask for three/five more.
native-userdata-probe/test-result.json records exact scope/limits. Final evidence:
native-userdata-probe/final-overlay-two-matches/{smash-gamepad-overlay.log,
smash-gamepad-overlay-probe.log,targeted-collection.json}. New creports relative
to pre-install baseline: NONE. Probe sample normal, no suspect line. First match:
all22 watched addresses retired before second match start, live0, no overflow.
Second match last summary happens at match-end BEFORE teardown; no later summary
exists, so do not claim its final retirement count was observed. User saw CSS.
FTP collector encountered451 after historical report downloads; four relevant
current logs were recovered successfully using fresh sessions, manifest has0errors.

CORRECTION to previous Eden header-only notes: the completed archived eden2 probe
log DOES contain a normal base-pane sample, same LR5f540/vtable5240748/ext0/flags
as native. Earlier live reads were before that record appeared (no per-record
timestamp to locate it more precisely). No demonstrated native/Eden branch divergence.
Fable report fable-native-emu-probe-path.md archived under native-userdata-probe.
Root agrees with direct evidence and branch predicate; does NOT adopt claims that
post-NUL name bytes prove emulator-specific initialization (second native run also
zeros) or that an alpha predicate excludes an injected pane under possible memory
corruption. Both are at most conditional hypotheses. No parent-pointer dereference
or expanded logger is warranted absent a suspect record; preserve current tested NRO.
Fable exact6a30 wrapper stopped, own watcher reaped. Grok stopped. No code changes,
commits, or pushes since diagnostic acceptance. Native corrected overlay + probe
remain installed; PROBE_ONLY preserved in SD backup. All other plugins unchanged.
Switch HOME, Smash closed; all owned Eden stopped; both probe-run profiles restored.
Separate asset-only root cause still unproven/unreproduced. Initial two-match native
success supports lifecycle fix; do not call every historical crash resolved.
