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
