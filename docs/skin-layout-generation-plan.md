# Skin Layout Generation Plan

This document plans how to turn
`target/skin-build/switch-pro-alt/skin_manifest.json` into real Smash UI
picture panes. It is intentionally documentation-only for this milestone.

Do not write a modified `layout.arc` yet. Do not change runtime plugin
behavior yet. Keep the current `minimal_debug` square-pane visual overlay as
the stable fallback until one PNG-backed pane is proven in game.

## Source Inputs

- Manifest: `target/skin-build/switch-pro-alt/skin_manifest.json`
- Skin name: `switch_pro_alt_builtin`
- Root pane: `sgpo_root`
- RetroSpy source skin: `switch-pro-alt`
- Layout source: user-owned extracted `info_melee/layout.arc`
- Smash data-replacement path after repack:
  `ui/layout/info/info_melee/info_melee/layout.arc`

Target files inside the unpacked layout:

- `blyt/info_melee.bflyt`
- `blyt/info_melee_lct_player_00.bflyt`
- `blyt/info_melee_lct_player_01.bflyt`
- `timg/__Combined.bntx`

The direct-pane approach should not modify `info/layout.info` or any BFLAN
animation files.

## Layout Changes Needed

Use the same parent strategy as the current square-pane patch:

- Insert `sgpo_root` as a direct `RootPane` child in all three target BFLYTs.
- Insert one PNG-backed `pic1` child per manifest element under `sgpo_root`.
- Keep injected `sgpo_root` and child panes hidden by default with `alpha=0`.
- Leave pane visibility flags cloned from the stable source pane unless Switch
  Toolbox verification shows a specific flag needs to change.
- Treat manifest `base_x` and `base_y` as child-local coordinates under
  `sgpo_root`.
- Keep `sgpo_root` position/scale controlled by the runtime overlay config.

The generated pane, material, and texture names must be deterministic:

- Pane name: manifest `pane_name`
- Material name: manifest `material_name`
- Texture name: `tex_{pane_name}`
- Source image: manifest `image_filename`

For each PNG-backed element, the layout asset generation eventually needs:

- a `pic1` pane with the manifest pane name, base position, size, alpha `0`,
  and material index pointing to the generated material;
- a `mat1` material entry using the manifest material name;
- a `txl1` texture reference using `tex_{pane_name}`;
- a `__Combined.bntx` texture imported from the manifest image file.

The new `pic1` vertex colors should start white/opaque. Runtime alpha controls
released/pressed visibility, so baked-in tinting should not be used.

## Manifest Pane Map

| Control | Pane | Material | Texture | PNG | Size | Base |
| --- | --- | --- | --- | --- | --- | --- |
| A | `sgpo_alt_face_a` | `mat_sgpo_alt_face_a` | `tex_sgpo_alt_face_a` | `face_A.png` | 99x100 | 431.5,137.5 |
| B | `sgpo_alt_face_b` | `mat_sgpo_alt_face_b` | `tex_sgpo_alt_face_b` | `face_B.png` | 100x99 | 333,52 |
| X | `sgpo_alt_face_x` | `mat_sgpo_alt_face_x` | `tex_sgpo_alt_face_x` | `face_X.png` | 99x100 | 332.5,222.5 |
| Y | `sgpo_alt_face_y` | `mat_sgpo_alt_face_y` | `tex_sgpo_alt_face_y` | `face_Y.png` | 100x100 | 235,137.5 |
| L | `sgpo_alt_l` | `mat_sgpo_alt_l` | `tex_sgpo_alt_l` | `trigger_L.png` | 327x113 | -334.5,347 |
| R | `sgpo_alt_r` | `mat_sgpo_alt_r` | `tex_sgpo_alt_r` | `trigger_R.png` | 327x113 | 335.5,347 |
| ZL | `sgpo_alt_zl` | `mat_sgpo_alt_zl` | `tex_sgpo_alt_zl` | `trigger_ZL.png` | 227x133 | -364.5,408 |
| ZR | `sgpo_alt_zr` | `mat_sgpo_alt_zr` | `tex_sgpo_alt_zr` | `trigger_ZR.png` | 227x133 | 364.5,408 |
| L3 | `sgpo_alt_l3` | `mat_sgpo_alt_l3` | `tex_sgpo_alt_l3` | `stick_LS_Press.png` | 206x204 | -347,137.5 |
| R3 | `sgpo_alt_r3` | `mat_sgpo_alt_r3` | `tex_sgpo_alt_r3` | `stick_RS_Press.png` | 205x204 | 165.5,-36.5 |
| Plus | `sgpo_alt_plus` | `mat_sgpo_alt_plus` | `tex_sgpo_alt_plus` | `center_Plus.png` | 61x62 | 155.5,232.5 |
| Minus | `sgpo_alt_minus` | `mat_sgpo_alt_minus` | `tex_sgpo_alt_minus` | `center_Minus.png` | 61x62 | -155.5,232.5 |
| Home | `sgpo_alt_home` | `mat_sgpo_alt_home` | `tex_sgpo_alt_home` | `center_Home.png` | 63x63 | 89.5,137 |
| Capture | `sgpo_alt_capture` | `mat_sgpo_alt_capture` | `tex_sgpo_alt_capture` | `center_Capture.png` | 58x58 | -89,137.5 |
| DpadUp | `sgpo_alt_dpad_up` | `mat_sgpo_alt_dpad_up` | `tex_sgpo_alt_dpad_up` | `dpad_Up.png` | 68x97 | -194,11 |
| DpadDown | `sgpo_alt_dpad_down` | `mat_sgpo_alt_dpad_down` | `tex_sgpo_alt_dpad_down` | `dpad_Down.png` | 68x97 | -194,-83 |
| DpadLeft | `sgpo_alt_dpad_left` | `mat_sgpo_alt_dpad_left` | `tex_sgpo_alt_dpad_left` | `dpad_Left.png` | 98x68 | -241,-36.5 |
| DpadRight | `sgpo_alt_dpad_right` | `mat_sgpo_alt_dpad_right` | `tex_sgpo_alt_dpad_right` | `dpad_Right.png` | 98x67 | -147,-36 |
| LeftStickDot | `sgpo_alt_left_stick` | `mat_sgpo_alt_left_stick` | `tex_sgpo_alt_left_stick` | `stick_Left.png` | 164x164 | -347,137.5 |
| RightStickDot | `sgpo_alt_right_stick` | `mat_sgpo_alt_right_stick` | `tex_sgpo_alt_right_stick` | `stick_Right.png` | 164x164 | 165,-36.5 |

Stick movement ranges from the manifest:

- `sgpo_alt_left_stick`: 41x41
- `sgpo_alt_right_stick`: 41x41

## Coordinate Transform

The analyzer already converts RetroSpy top-left coordinates into centered
Smash-style pane coordinates:

```text
base_x = x + width / 2 - background_width / 2
base_y = background_height / 2 - (y + height / 2)
```

The future layout generator should use the manifest `base_x` and `base_y`
directly. It should not reapply the RetroSpy transform.

The runtime scales and positions `sgpo_root`; child pane positions remain in
unscaled root-local units. This avoids double-scaling when
`OVERLAY_CONFIG.scale` changes.

## Safe Automation Now

The following pieces are safe to automate before writing binary layout assets:

- Read `skin_manifest.json` and validate that every referenced PNG exists.
- Validate PNG dimensions against manifest `width` and `height`.
- Generate a repo-safe work-order report listing each pane, material, texture,
  source PNG, target BFLYT files, and expected defaults.
- Generate ignored local output under `target/` or `local-assets/`.
- Add a dry-run or proof mode that prepares one selected manifest element
  without repacking `layout.arc`.

The following pieces are safe to automate into ignored local output only after
manual structure verification:

- Clone the stable `sgpo_root`/`pic1` insertion behavior from
  `tools/patch_info_melee_layout.py`.
- Insert manifest-driven `sgpo_alt_*` panes hidden by alpha `0`.
- Preserve the current `sgpo_pro_*` square panes in the same layout so
  `minimal_debug` remains available.

## Requires Manual Switch Toolbox Verification

Do not fully automate these until a known-good single-pane diff is inspected:

- Import `face_A.png` into `timg/__Combined.bntx` with the correct Switch
  texture format, alpha behavior, and mip settings.
- Expand or edit BFLYT `txl1` safely and confirm the new texture name is
  referenced correctly.
- Expand or edit BFLYT `mat1` safely and confirm material texture binding
  offsets.
- Confirm `pic1` material index values point at the intended generated
  materials.
- Confirm panes render white/normal, not with the old red P1 marker tint.
- Confirm root and player-parts copies preserve pause/hide behavior.
- Compare one manually verified BFLYT/BNTX before teaching Python to generate
  all texture/material structures.

## Smallest Next Code Change

Add a one-pane proof mode, not a full converter:

- Inputs:
  - `target/skin-build/switch-pro-alt/skin_manifest.json`
  - RetroSpy skin directory containing `face_A.png`
  - original unpacked `local-assets/original/info_melee/unpacked`
- Control:
  - CLI-selected control, defaulting to `A`.
- Output:
  - ignored work directory under `target/skin-layout-proof/` or
    `local-assets/modified/`.
  - copied unpacked layout with `sgpo_root/sgpo_alt_face_a` inserted as a
    hidden `pic1` pane.
- Explicit non-goals:
  - do not repack `layout.arc`;
  - do not modify or generate BNTX textures automatically;
  - do not activate `switch_pro_alt_builtin` in Rust;
  - do not remove current `sgpo_pro_*` panes.

The generated instructions for that proof should tell the user to:

1. Open the output layout in Switch Toolbox.
2. Import `face_A.png` into `__Combined.bntx`.
3. Name the texture `tex_sgpo_alt_face_a`.
4. Bind it to `mat_sgpo_alt_face_a`.
5. Save the layout assets.
6. Repack manually with the Python `sarc` package.
7. Test in game while `minimal_debug` remains the stable fallback.

## Validation Checklist

- `python tools/analyze_retrospy_skin.py` still validates exactly against
  `switch_pro_alt_builtin`.
- `python tools/patch_info_melee_layout.py --self-test` still validates the
  current BFLYT section assumptions.
- `docs/skin-layout-generation-plan.md` lists the exact pane/material/texture
  naming policy.
- No tracked or release artifact includes `data.arc`, copied RetroSpy PNGs,
  extracted BFLYT/BFLAN/BNTX files, modified `layout.arc`, or generated game
  assets.
- Runtime baseline remains unchanged: `minimal_debug` is active and DebugText
  fallback remains available.

## Assumptions

- Direct `sgpo_root` children are the first PNG-backed target.
- A separate `sgpo_controller.bflyt` or `Parts` pane is deferred until direct
  PNG-backed panes are proven.
- Texture names use `tex_{pane_name}` for traceability and collision avoidance.
- `sgpo_root` can eventually contain both current `sgpo_pro_*` square panes and
  future `sgpo_alt_*` PNG panes; the active Rust skin chooses which set updates.
