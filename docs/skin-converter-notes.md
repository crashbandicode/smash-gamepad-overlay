# Skin Converter Notes

The runtime renderer should stay simple: poll controller state, map it to `ControllerViewState`, and update named Smash UI panes from `SkinElement` data.

Do not make the Skyline plugin parse arbitrary PNGs or build full UI assets at runtime.

## Current Built-Ins

- `minimal_debug`: active square-based skin that uses the patched `sgpo_pro_*` panes.
- `switch_pro_alt_builtin`: inactive data-only skin that mirrors RetroSpy's `switch-pro-alt` Switch layout. It records pane names, source image names, dimensions, positions, alpha states, and stick movement ranges, but it does not become active until matching panes/assets exist in the patched layout.

## Future Converter Flow

```text
RetroSpy skin.xml + PNG assets
  -> PC-side converter
  -> generated layout.arc panes/materials/textures
  -> generated skin manifest or Rust skin table
  -> plugin updates panes by ControlId
```

The converter should:

- read the RetroSpy `skin.xml` control positions and image names;
- copy or convert PNG assets into Smash-compatible texture/material data;
- inject one named pane per `SkinElement` under `sgpo_root`;
- emit a manifest/table that maps `ControlId` to pane name, image/material name, x/y, size, alpha values, and stick movement range;
- keep generated `layout.arc`, extracted BFLYT/BFLAN/BNTX assets, and game dumps out of git.

The plugin should only require the generated panes to already exist.

## Dry-Run Pipeline

The first converter milestone is metadata-only:

```bash
python tools/analyze_retrospy_skin.py
```

Provide a RetroSpy skin directory either with `--skin-dir` or with
`SGPO_RETROSPY_SKIN_DIR` in `.env`:

```bash
python tools/analyze_retrospy_skin.py --skin-dir "/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt"
```

It writes repo-safe planning artifacts only:

- `target/skin-analysis/switch-pro-alt.md`: parsed control report and comparison against `switch_pro_alt_builtin`.
- `target/skin-build/switch-pro-alt/skin_manifest.json`: generated dry-run skin manifest.
- `target/skin-build/switch-pro-alt/skin_manifest.md`: human-readable manifest summary.

The manifest contains one entry per mapped control:

- `ControlId`
- pane name
- image filename
- generated material name
- source x/y and centered Smash base x/y
- width/height
- pressed/released alpha defaults
- pressed/released scale defaults
- optional stick movement range

The tool validates that the generated manifest exactly matches the inactive
`switch_pro_alt_builtin` Rust target. It does not copy PNGs, generate
Nintendo layout assets, or write a modified `layout.arc`.

Parser regression tests cover the Rust-source parsing used by the analyzer:

```bash
python -m unittest tools.tests.test_analyze_retrospy_skin
```

The next planning document is `docs/skin-layout-generation-plan.md`. It records
the BFLYT/BNTX/layout work needed before converting the dry-run manifest into
real PNG-backed Smash UI panes.
