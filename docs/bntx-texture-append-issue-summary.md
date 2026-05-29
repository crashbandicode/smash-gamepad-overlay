# BNTX Texture Append Issue Summary

This is a handoff note for the Toolbox-Cli/Rust layout-tooling work. The
runtime overlay is now proving that the `switch_pro_alt_builtin` skin panes can
be found and updated, but appending PNG-backed textures to Smash's existing
`__Combined.bntx` still appears to corrupt unrelated HUD textures in-game.

## Latest Runtime Observation

- Screenshot tested:
  `C:\Games\Eden-Windows-MSVC-0.0.1-pre-alpha-amd64\eden-windows-msvc\user\screenshots\01006a800016e000_2026-05-28_15-50-58-937.png`
- Runtime log:
  `C:\Games\Eden-Windows-MSVC-0.0.1-pre-alpha-amd64\eden-windows-msvc\user\sdmc\smash-gamepad-overlay.log`
- Build line:
  `c20-b111-40638a3-dirty-502c6ec2b8e62c04`
- Active skin:
  `switch_pro_alt_builtin`
- Runtime path:
  Training Modpack compatibility mode, non-draw HUD hooks on Smash `13.0.4`.

The controller skin itself is mostly correct and input updates work. The green
background is now mostly transparent, with only a thin green fringe around the
controller shell. However, unrelated Smash HUD textures are still wrong:

- timer textures in the top-right are incorrect/misaligned;
- damage percent textures are wrong;
- player portrait/HUD areas look darkened or texture-corrupted.

That points away from HID polling or pane update logic and toward the generated
layout/BNTX asset changes.

## What We Changed

The current skin pipeline is:

1. Parse RetroSpy `switch-pro-alt/skin.xml` into a 21-element manifest.
2. Include the RetroSpy background as `ControlId::SkinBackground`.
3. Copy the RetroSpy PNGs into an ignored generated workdir.
4. Chroma-key `background.png` from green to transparent before import.
5. Use `toolbox-cli layout-apply-manifest` to add panes/materials/texture refs.
6. Import all generated skin PNGs into `timg/__Combined.bntx`.
7. Validate all three BFLYTs:
   - `blyt/info_melee.bflyt`
   - `blyt/info_melee_lct_player_00.bflyt`
   - `blyt/info_melee_lct_player_01.bflyt`
8. Pack and stage the generated `layout.arc` through ARCropolis.

Important generated files:

- `local-assets/generated/switch-pro-alt/layout.arc`
- `local-assets/generated/switch-pro-alt/unpacked/timg/__Combined.bntx`
- `local-assets/generated/switch-pro-alt/unpacked/blyt/info_melee.bflyt`
- `local-assets/generated/switch-pro-alt/unpacked/blyt/info_melee_lct_player_00.bflyt`
- `local-assets/generated/switch-pro-alt/unpacked/blyt/info_melee_lct_player_01.bflyt`

Baseline source files for comparison:

- `local-assets/modified/info_melee/unpacked/timg/__Combined.bntx`
- `local-assets/modified/info_melee/unpacked/blyt/*.bflyt`

## Checks Already Passing

These checks passing means the obvious metadata is probably not the issue:

- `cargo fmt --check`
- `cargo skyline check`
- `cargo skyline build --release`
- `python -m unittest tools.tests.test_analyze_retrospy_skin`
- `python tools/analyze_retrospy_skin.py --skin-dir "/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt"`
- `toolbox-cli bflyt-roundtrip-test` on the generated BFLYTs
- `toolbox-cli bntx-roundtrip-test` on the generated `__Combined.bntx`
- `toolbox-cli bntx-dict-test` on the generated `__Combined.bntx`
- manifest validation finds all 21 expected panes/materials/textures

Manual metadata comparison also showed:

- original texture count: `206`
- generated texture count: `227`
- original texture entries `0..205` kept the same inspected metadata;
- appended SGPO textures start at index `206`;
- generated BNTX keeps `alignment_shift = 12` (`4096` byte alignment);
- `tex_sgpo_alt_background` imports as a padded `1280x968` BC7 texture.

## Suspected BNTX Append Issue

The in-game symptoms look like Smash can load the archive but some existing HUD
materials sample the wrong data after the append. Since original texture names
and basic metadata appear unchanged, likely suspects are lower-level BNTX binary
details that our current tests do not prove against Nintendo's loader:

- `_RLT` relocation section contents after appending or shifting texture data;
- BRTI/BRTD pointer relocation values for appended textures;
- texture data block offsets/sizes after the file grows;
- `_DIC` Patricia dictionary node ordering/flags/indices despite lookup tests
  passing in our own reader;
- string table or name pointer relocation after adding many names;
- BC7 block padding and size calculations for non-multiple-of-four dimensions;
- large appended texture behavior, especially the `1280x965 -> 1280x968`
  background texture;
- format/channel defaults for imported PNGs, such as `BC7_UNORM` versus
  existing Smash UI texture settings.

The material clone source could also matter, but material-state problems should
mostly affect SGPO panes. The fact that timer, percent, and portrait textures
are wrong makes the BNTX append/layout texture binding a stronger suspect.

## Thin Green Outline

The green fringe is probably separate from the HUD texture corruption.

Current preprocessing zeroes the alpha for obvious chroma-key pixels in
`background.png`, and inspection showed no fully opaque green key pixels left.
The remaining outline is likely one of:

- anti-aliased edge pixels that are green-ish but not inside the current key
  threshold;
- BC7 interpolation/compression bleeding green RGB from transparent pixels;
- transparent pixels retaining green RGB even with alpha `0`.

Suggested fix for the image path:

- widen the chroma-key threshold using hue/distance-to-green instead of only a
  narrow RGB threshold;
- after keying, bleed nearby non-green edge colors into transparent pixels
  before BC7 compression;
- consider premultiplied-alpha-safe RGB cleanup for transparent pixels.

## Suggested Next Experiments

To isolate the BNTX problem, please run the smallest possible generated layout
variants:

1. **BFLYT-only control:** add `sgpo_alt_*` panes/materials/texture refs but do
   not modify `__Combined.bntx`. Expected: SGPO art missing, but stock HUD
   textures should remain correct. If stock HUD is still broken, the issue is
   likely BFLYT material/texture-reference indexing.
2. **One tiny appended texture:** append only one small face-button PNG and bind
   only one pane. If this works, the bulk append or background texture is the
   trigger.
3. **Background-only append:** append only `background.png`. If this breaks HUD
   textures, focus on large texture sizing/alignment/relocations.
4. **All small controls, no background:** append all button/stick PNGs except
   `background.png`. If this works, the background texture is the trigger.
5. **Separate BNTX test:** if Smash accepts multiple `timg/*.bntx` files for
   this layout, put SGPO textures in a separate BNTX instead of mutating
   `__Combined.bntx`.
6. **Switch Toolbox comparison:** create or save an equivalent BNTX through a
   known-good tool and diff section layout, `_RLT`, dictionaries, BRTI/BRTD
   offsets, alignment, and format/channel settings.

## Files and Tools to Inspect

Runtime/plugin files involved:

- `src/input.rs`
- `src/skin.rs`
- `src/visual.rs`
- `src/hud/cache.rs`

PC-side tooling involved:

- `tools/analyze_retrospy_skin.py`
- `tools/build_skin_layout.py`
- `tools/tests/test_analyze_retrospy_skin.py`

Toolbox-Cli fixture bundle:

- `local-assets/switch-toolbox-cli-agent-fixtures/README.md`

The runtime plugin should stay simple: it only polls input and updates already
existing named panes. The asset generation problem should be solved on the
PC-side tooling path.
