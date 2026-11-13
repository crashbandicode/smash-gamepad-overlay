# Technical notes

Moved here from the README on 2026-11-13 and checked against the code at
`a0dc8fc` (v0.3.1).

## Runtime skin config

The plugin reads an optional config at startup and again at each match start
(not every frame), `sd:/ultimate/mods/smash-gamepad-overlay/config.json`
(`src/config.rs`, `src/skin.rs`). The release SD zip ships:

```json
{
  "active_skin": "auto",
  "default_skins": {
    "gamecube": "default_simple_gamecube",
    "switch": "default_simple"
  }
}
```

If the file is missing, unreadable, has no `active_skin` string, or names an
unknown skin, the plugin falls back to `default_simple`.

With `active_skin` set to `auto`, the skin follows P1's controller: a GameCube
controller uses `default_skins.gamecube`, every other style (Pro Controller,
Joy-Con, handheld) `default_skins.switch`. The skin can change during a match
if P1's controller changes. To pin one skin, set `active_skin` to one built-in
name and start a new match.

Built-in skin names:

- `default_simple`: square-pane Switch/Pro/Joy-Con skin (`sgpo_pro_*` panes).
- `default_simple_gamecube`: the same panes with non-GameCube controls omitted.
- `minimal_debug`: an alias for `default_simple`.
- `switch_pro_alt_builtin`: generated from RetroSpy's `switch-pro-alt` skin.
  Renders only if the installed layout has the matching `sgpo_alt_*` panes.
- `gamecube_tron_builtin`: generated from RetroSpy's `gamecube-tron` skin.
  Renders only if the installed layout has the matching `sgpo_gct_*` panes.

The two RetroSpy-based skins need RetroSpy's skin folders when you generate the
layout ([below](#optional-retrospy-skins)).

## How it renders

The plugin captures the layout it draws into from the match HUD's
set-info-alpha hook (P1's HUD parts `p1` or `p1_2`, or the root `info_melee`
layout if the hook reports it; `src/hud/capture.rs`) and drives live updates
from the scene-update hook. It does not hook `Layout::Draw`. Cached panes are
tracked through a guarded `Pane::Finalize` observer, so a pane is forgotten
when the game frees it; a guarded `Layout::Finalize` observer only counts. The
skin root's place in each captured layout is the host-tested table in
`crates/sgpo-ui-cache-metadata/src/placement.rs`: beside P1's HUD in a match,
to the right of the CPU's HUD in Training and waiting-room practice. Each
frame it reads the game's final input for P1 (`src/final_input.rs`, nine
64-byte windows checked at boot; the raw pad when they differ or no player
uses the polled controller).

Up to v0.3.0 this HUD path was chosen only when the plugin found Training
Modpack's NRO by file name; otherwise it hooked `Layout::Draw` and drew into
the root `info_melee` layout (bottom right, full size). The path choice was
removed on 2026-11-05 ([STATUS](STATUS.md)).

## Runtime log

`sd:/smash-gamepad-overlay.log` (in Eden, `<eden>/sdmc/smash-gamepad-overlay.log`)
records startup, the build ID, the Smash display version, the hook and guard
results, the HUD capture and update status and the render path. It is
rewritten at each boot.

If the log says `non-draw HUD path could not find skin '…' pane 'sgpo_root'`,
Smash is loading an unpatched `info_melee` layout; the next lines say so and
point at `tools/sgpo_installer`. Regenerate and reinstall the layout (README,
Install).

## Build ID and tests

Runtime logs include a build ID such as `c26-b3-1234abc`: `c26` is
`git rev-list --count HEAD`, `b3` a local build counter in
`target/sgpo-local-build-number` (reset by `cargo clean`), and a
`-dirty-<hash>` suffix marks a build from an uncommitted tree.
`SMASH_GAMEPAD_OVERLAY_BUILD_ID` overrides the whole ID (`build.rs`).

Host tests:

```sh
cargo test --manifest-path crates/sgpo-ui-cache-metadata/Cargo.toml
cargo test --manifest-path crates/sgpo-final-input/Cargo.toml
cargo test --manifest-path tools/sgpo_installer/Cargo.toml
```

Host `cargo clippy` on the plugin crate itself fails on stable Rust because
`nnsdk` uses `#![feature]`; use `cargo skyline clippy`.

## Memory

The plugin is mapped into the game's memory for the whole session, so the
release profile favours size: `opt-level = "s"` with fat LTO and
`panic = "abort"` (`Cargo.toml`). Resident size of each release NRO
(`text`+`rodata`+`data`+`bss`, the playbook's `tools/build/nro_size.py`,
2026-11-13): v0.1.1–v0.1.3 116 KiB, v0.2.0 120 KiB, v0.3.0 124 KiB, v0.3.1
104 KiB.

The v0.1.1 memory pass, measured on the aarch64 ELF with the same fixed build
ID (`readelf -S`; NRO size from the file):

| build | `.text` | `.rodata` | `.data` | `.bss` | `.rela.dyn` | NRO |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| v0.1.0 (`opt-level = 3`) | 87744 | 22032 | 13296 | 4016 | 8544 | 143360 |
| v0.1.0 code, `opt-level = "s"` | 63952 | 21912 | 13304 | 4048 | 8496 | 118784 |
| v0.1.1 code, `opt-level = 3` | 89984 | 21592 | 10936 | 4016 | 7776 | 139264 |
| v0.1.1 (`opt-level = "s"`) | 64544 | 21496 | 10944 | 4048 | 7728 | 114688 |

Besides the profile, v0.1.1 dropped the two converter-only name fields from
every `SkinElement` (the tables lost 2360 bytes of `.data` and 32 dynamic
relocations, per the table), formatted `DebugText` lines into fixed 256-byte
stack buffers and passed text to `set_text_string` from a stack buffer. The
`DebugText` fallback itself was removed in v0.3.1.

## Layout generation options

With `.env` configured (copy `.env.example`), the installer reads
`SGPO_DATA_ARC` (or `SGPO_INFO_MELEE_LAYOUT_ARC`) and `SGPO_SD_ROOT`, generates
the layout, writes `config.json`, backs up any existing layout, config and NRO,
and stages the NRO from `target/` to the SD root:

```sh
cargo run --manifest-path tools/sgpo_installer/Cargo.toml -- --force
```

Explicit inputs, including the NRO to stage:

```sh
cargo run --manifest-path tools/sgpo_installer/Cargo.toml -- \
  --sd-root /path/to/sdmc \
  --data-arc /path/to/data.arc \
  --nro target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro \
  --force
```

Add `--sd-zip <file> --no-sd-stage` to write an SD-root ZIP (NRO, layout and
config) instead of copying to an SD root.

The patcher adds a hidden `sgpo_root` with 24 square panes to the root
`info_melee` layout and to both player HUD parts layouts
(`info_melee_lct_player_00`/`_01`). The overlay draws in the player-parts copy;
the root copy served the `Layout::Draw` path removed on 2026-11-05 and is used
by the HUD path only if the HUD hook reports the root layout. The layout notes
in `docs/` were taken from a 13.0.4 dump; the 13.0.5 change was a code-offset
update.

The older Python scripts (`tools/patch_info_melee_layout.py`,
`tools/stage_arcropolis_layout.py`) remain as reference tooling. See
[release-bundle-strategy](release-bundle-strategy.md) for the bundle plan.

## Optional RetroSpy skins

```sh
cargo run --manifest-path tools/sgpo_installer/Cargo.toml -- --force --include-all-presets
```

This builds `switch_pro_alt_builtin` and `gamecube_tron_builtin` into the same
layout. Point `--switch-pro-alt-dir` and `--gamecube-tron-dir` at RetroSpy's
`skins/switch-pro-alt` and `skins/gamecube-tron` folders. Generated textures
default to `--texture-format bc7-srgb`; `rgba8-srgb` is available for
diagnosing compression artifacts at the cost of a larger layout.

## Skin model

The renderer is skin-driven and does not create or decode PNGs at runtime.
Each skin is a Rust table of `SkinElement` entries that map a logical
`ControlId` to an existing named Smash UI pane, with base position, size,
released/pressed alpha and scale, and an optional stick movement range
(`src/skin.rs`). PNG conversion, texture packing and pane injection happen on
the PC. See [skin-converter-notes](skin-converter-notes.md).

## Clips in the README

Both clips were recorded with Eden's recorder at 12 fps and 640 pixels wide,
6 s each (73 frames), in Eden `9a5d704c9c` with Smash 13.0.5 and the
`switch_pro_alt_builtin` skin; a default install shows the square-pane
`default_simple` skin in the same place. The Training clip (2026-10-28, commit
`fa0a3fb`) ran the v0.1.3 release NRO with the Training Modpack build from the
tested Switch, with `libnro_hook.nro` and `libparam_hook.nro` in the plugin
folder, as on the Switch; in the v0.1.3 validation that Training Modpack
build had faulted at boot in Eden (`training_modpack+0x59c08`, v0.1.3
`manifest.json`). Cropped stills of the same scenes are in
`docs/media/gamepad-training.png` and `docs/media/gamepad-match.png`.

## Agent files and constraints

- `AGENTS.md`: project instructions for coding agents; `CLAUDE.md`: the Claude
  Code entry point; `sgpo-handoff.mdc`: a portable Cursor rule (copy it to
  `.cursor/rules/sgpo-handoff.mdc`). Handoffs are kept outside this
  repository.
- Before using a build as a new baseline, run the
  [Alpha Validation Checklist](alpha-validation-checklist.md).
- Rust only. The overlay must render inside Smash so a capture card records
  it: no PC overlay, OBS/browser source, network streaming, RetroSpy hardware
  or external receiver. Do not commit or publish Nintendo assets: `data.arc`,
  extracted BFLYT/BFLAN/BNTX files, textures, or generated `layout.arc` files.
