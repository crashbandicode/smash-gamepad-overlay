# Switch Toolbox One-Pane Proof

This is a manual proof workflow for creating one PNG-backed Smash UI pane from a
RetroSpy skin asset. The goal is to produce a small known-good before/after diff
that can guide a future automated converter.

Do not commit or release anything under `local-assets/proof/`. These files are
user-owned extracted/modified game assets.

## Goal

Create one picture pane in `info_melee.bflyt`:

```text
pane:     sgpo_alt_face_a
material: mat_sgpo_alt_face_a
texture:  tex_sgpo_alt_face_a
source:   face_A.png
parent:   sgpo_root
```

Expected pane properties:

```text
x = 431.5
y = 137.5
z = 0
width = 99
height = 100
alpha = 0
```

The pane starts hidden by alpha. Runtime code will make active panes visible and
drive their alpha/position later.

## Inputs

Expected local paths:

```text
local-assets/modified/info_melee/unpacked/
/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt/face_A.png
```

Useful Windows paths from WSL:

```bash
wslpath -w "$PWD/local-assets/proof/switch-pro-alt-one-pane/unpacked"
wslpath -w "/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt/face_A.png"
```

## Prepare Proof Workdir

From the repo root:

```bash
rm -rf local-assets/proof/switch-pro-alt-one-pane
mkdir -p local-assets/proof/switch-pro-alt-one-pane
cp -a local-assets/modified/info_melee/unpacked local-assets/proof/switch-pro-alt-one-pane/
```

## Import Texture Into BNTX

In Switch Toolbox:

1. Open:

   ```text
   local-assets/proof/switch-pro-alt-one-pane/unpacked/timg/__Combined.bntx
   ```

2. Import or add:

   ```text
   face_A.png
   ```

3. Rename the imported texture to:

   ```text
   tex_sgpo_alt_face_a
   ```

4. Use an RGBA/alpha-capable texture format. If unsure, keep Toolbox defaults.

5. Save `__Combined.bntx`.

## Add Texture Name To BFLYT

Open:

```text
local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt
```

In the BFLYT tree, find the texture list. Add this texture name if it is not
already present:

```text
tex_sgpo_alt_face_a
```

The BNTX texture and the BFLYT texture-list entry are both needed.

## Create The Picture Pane

Preferred copy path:

1. In `info_melee.bflyt`, select:

   ```text
   RootPane > sgpo_root > sgpo_pro_a_marker
   ```

2. Focus the layout preview/viewport.

3. Press:

   ```text
   Ctrl+C
   Ctrl+V
   ```

4. Rename the copied pane:

   ```text
   sgpo_alt_face_a
   ```

5. Confirm its parent is:

   ```text
   sgpo_root
   ```

Alternative context-menu path:

1. Select `sgpo_pro_a_marker`.
2. Right-click in the layout preview viewport.
3. Choose `Copy (Experimental)`.
4. Right-click in the viewport again.
5. Choose `Paste (Experimental)`.
6. Rename the copied pane to `sgpo_alt_face_a`.

Fallback if copy/paste is unavailable:

1. Right-click the preview viewport.
2. Choose `Create Pane > Picture Pane`.
3. Rename it to `sgpo_alt_face_a`.
4. Reparent it under `sgpo_root`.

## Set Pane Transform

For `sgpo_alt_face_a`, set:

```text
translate x = 431.5
translate y = 137.5
translate z = 0
scale x = 1
scale y = 1
width = 99
height = 100
alpha = 0
visible = true
```

## Create And Bind Material

The pane must use a material that references the imported texture.

1. Create or duplicate a material for the pane.

2. Name it:

   ```text
   mat_sgpo_alt_face_a
   ```

3. Set the pane `sgpo_alt_face_a` to use:

   ```text
   mat_sgpo_alt_face_a
   ```

4. In `mat_sgpo_alt_face_a`, add or set one texture map/reference:

   ```text
   tex_sgpo_alt_face_a
   ```

5. Make sure the material has a texture map count of one, not zero.

6. Keep material colors neutral:

   ```text
   white color = 255,255,255,255
   black color = 0,0,0,0
   ```

7. Save `info_melee.bflyt`.

The common failure case is a material that exists but has no texture reference.
That looks like this in inspection output:

```text
mat_sgpo_alt_face_a tex=[]
```

The expected result is one texture reference:

```text
mat_sgpo_alt_face_a tex=[tex_sgpo_alt_face_a]
```

## Rename Material If Switch Toolbox Cannot

Some Switch Toolbox builds do not expose a material rename UI. If the material
still has an auto-generated name such as `sgpo_alt_face_a`, rename the material
slot with the helper script before repacking.

Always list first:

```bash
python tools/tweak_bflyt_material.py \
  --input local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt \
  --list
```

Dry run the exact old name from the list output:

```bash
python tools/tweak_bflyt_material.py \
  --input local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt \
  --material-old-name sgpo_alt_face_a \
  --material-new-name mat_sgpo_alt_face_a \
  --dry-run
```

Apply:

```bash
python tools/tweak_bflyt_material.py \
  --input local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt \
  --material-old-name sgpo_alt_face_a \
  --material-new-name mat_sgpo_alt_face_a
```

This only renames the material. It does not add or bind texture references.

## Repack Proof Layout

From the repo root:

```bash
python -m sarc create \
  --base-path "$PWD/local-assets/proof/switch-pro-alt-one-pane/unpacked" \
  "$PWD/local-assets/proof/switch-pro-alt-one-pane/unpacked" \
  "$PWD/local-assets/proof/switch-pro-alt-one-pane/layout.arc"
```

## Files To Leave In Place

Leave these files in place for inspection/diffing:

```text
local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt
local-assets/proof/switch-pro-alt-one-pane/unpacked/timg/__Combined.bntx
local-assets/proof/switch-pro-alt-one-pane/layout.arc
```

## Expected Inspection Signals

After the edit, these strings should exist:

```bash
strings -a local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt | rg 'sgpo_alt_face_a|mat_sgpo_alt_face_a|tex_sgpo_alt_face_a'
strings -a local-assets/proof/switch-pro-alt-one-pane/unpacked/timg/__Combined.bntx | rg 'tex_sgpo_alt_face_a'
strings -a local-assets/proof/switch-pro-alt-one-pane/layout.arc | rg 'sgpo_alt_face_a|mat_sgpo_alt_face_a|tex_sgpo_alt_face_a'
```

A deeper binary inspection should show:

```text
sgpo_alt_face_a parent=sgpo_root alpha=0 pos=(431.5,137.5,0) size=(99,100)
sgpo_alt_face_a material -> mat_sgpo_alt_face_a
mat_sgpo_alt_face_a texture -> tex_sgpo_alt_face_a
```
