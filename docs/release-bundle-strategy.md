# Release Bundle Strategy

The release should stay split between runtime code and user-owned assets:

```text
release NRO + PC-side install tool
  + user-owned Smash data.arc
  + optional user-owned RetroSpy skin folders
  -> staged Skyline plugin + ARCropolis layout replacement + config.json
```

Do not ship Nintendo assets, copied RetroSpy PNGs, unpacked BFLYT/BFLAN/BNTX
files, or generated `layout.arc` files in Forgejo releases.

## Release Contents

The v0.1.1 release on Forgejo carries four assets:

- `libsmash_gamepad_overlay.nro`
- `smash-gamepad-overlay-v0.1.1-sd.zip`, an SD-root archive holding the NRO at its Skyline plugin path, `config.json` and a short README text, with no game data
- `manifest.json`
- `SHA256SUMS.txt`

The installer (`tools/sgpo_installer`) and the Python reference tools are not
release assets; users clone the tagged source and run the installer from it, as
`README.md` describes.

The Rust installer uses `nx-layout-toolbox` as a library. Until that crate is
published, local development uses a Toolbox-Cli checkout at
`local-checkouts/Toolbox-Cli`. Once the crate is published, normal releases
should not require users to install Python or manage a separate `toolbox-cli`
executable.

## Current Install Flow

For the default square skins, the user provides their own Smash `data.arc` and
the built NRO:

```bash
sgpo-installer \
  --sd-root /path/to/sdmc \
  --data-arc /path/to/data.arc \
  --nro target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro \
  --force
```

This extracts `info_melee/layout.arc`, injects the `default_simple` and
`default_simple_gamecube` square panes, repacks the layout, writes config, and
stages everything to the SD root.

With `.env` configured, the command can be shortened:

```bash
sgpo-installer --force
```

To produce a ZIP that users extract at the SD card root instead of copying
directly to a mounted SD path:

```bash
sgpo-installer \
  --data-arc /path/to/data.arc \
  --sd-zip sgpo-default-simple.zip \
  --no-sd-stage \
  --force
```

The ZIP contains:

- `atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmash_gamepad_overlay.nro`
- `ultimate/mods/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc`
- `ultimate/mods/smash-gamepad-overlay/config.json`

RetroSpy PNG-backed skins are optional. To include every currently supported
preset, add:

```bash
sgpo-installer --force --include-all-presets
```

The installer:

- extracts `info_melee/layout.arc` from `data.arc` when `--data-arc` is used;
- runs the square-pane baseline patcher automatically for original layout input;
- parses selected RetroSpy `skin.xml` files only when optional presets are included;
- emits ignored manifest/report files under `target/` for selected presets;
- uses `nx-layout-toolbox` library APIs to generate the final `layout.arc`;
- stages the generated ARCropolis replacement to
  `sd:/ultimate/mods/smash-gamepad-overlay/ui/layout/info/info_melee/info_melee/layout.arc`;
- writes `sd:/ultimate/mods/smash-gamepad-overlay/config.json`;
- backs up an existing layout/config/NRO before overwriting;
- copies the NRO to the Skyline plugin folder unless `--skip-nro` is passed.

## User Inputs

Required today:

- A user-owned Smash `data.arc`, or an already extracted
  `ui/layout/info/info_melee/info_melee/layout.arc`. Layout notes in this
  repo were taken from a 13.0.4 dump. Plugin code offsets target 13.0.5.
- A built SGPO NRO, unless `--skip-nro` is passed.
- SD root path, either `--sd-root`, `SGPO_SD_ROOT`, or inferred from
  `SGPO_EMU_MODS_DIR`, unless `--sd-zip` is passed.

Optional:

- RetroSpy-style skin folders containing `skin.xml` and PNG assets, selected
  with `--include-skin` or `--include-all-presets`.

## Near-Term Packaging Improvements

The clean next step is to reduce user input friction without changing runtime
behavior:

- publish `nx-layout-toolbox` and switch `tools/sgpo_installer` from the
  `local-checkouts/Toolbox-Cli` path dependency to a crates.io dependency;
- publish/build prebuilt `sgpo-installer` binaries for Windows, Linux, and
  macOS;
- optionally produce a portable `sgpo-tools.zip` with the Rust installer,
  reference Python tooling, and docs.

The runtime plugin should remain simple: it reads `config.json`, polls P1 input,
and updates already-existing named panes.
