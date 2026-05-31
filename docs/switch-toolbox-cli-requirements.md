# Rust Layout CLI Requirements For SGPO

This is the historical requirements document that guided the Rust Toolbox-Cli
work. The core SGPO path now uses `nx-layout-toolbox` as a library through a
local checkout at `local-checkouts/Toolbox-Cli`; keep this document as a
feature checklist and reference for future toolbox regressions.

The original intent was to port the needed Switch Toolbox-style asset logic
into a maintained Rust tool that can live alongside SGPO and be licensed under
MIT where our own code permits it.

The goal is not to move runtime logic into the CLI. SGPO should remain a simple
in-game Skyline plugin that polls input and updates already-existing named UI
panes. The CLI should be a PC-side asset tool that performs the BFLYT/BNTX/SARC
operations currently done by hand in Switch Toolbox.

## Project Context

SGPO currently has:

- a working square-pane overlay in patched `info_melee/layout.arc`;
- a RetroSpy dry-run analyzer that emits a skin manifest;
- generated PNG-backed Switch Pro and GameCube preset skins;
- a Rust installer that applies manifests, imports PNGs into BNTX, validates
  generated panes/materials/textures, and stages the final layout/NRO/config;
- legacy Python helpers for inspection and reference comparison.

The CLI should let SGPO convert a RetroSpy-style skin into local ignored Smash
layout assets without using the Switch Toolbox GUI or depending on the
unmaintained C# codebase at runtime.

Do not make the CLI responsible for runtime input, networking, OBS overlays, or
anything that runs on the Switch. This is a build/conversion tool only.

## Design Requirements

- Scriptable from Python or shell.
- Deterministic output for the same inputs.
- JSON input/output for automation, plus readable text for humans.
- Clear exit codes:
  - `0`: success
  - non-zero: validation failure or write failure
- `--dry-run` support for write commands.
- No destructive default behavior.
- Idempotent operations where practical:
  - adding an existing texture ref can no-op;
  - adding an existing pane can fail clearly or update only with an explicit flag.
- Preserve Smash/Switch Toolbox-compatible layout and texture structures.
- Preserve alpha-capable PNG behavior.
- Never require Nintendo assets to be committed or redistributed.

## Critical Requirement: JSON Inspect Output

The write commands matter, but reliable JSON inspection is just as important.
SGPO's Python converter should eventually stop parsing raw BFLYT/BNTX bytes
directly and instead consume stable CLI output.

At minimum, the CLI should expose JSON inspection for:

- BFLYT pane tree and pane transforms;
- `pic1` pane material indices and resolved material names;
- `txl1` texture references;
- `mat1` material names and texture references;
- BNTX texture names, dimensions, formats, and mip/alpha metadata when available.

This lets the converter validate every generated skin without depending on
duplicated binary parsers in Python.

## Priority 1: One-Pane Proof Automation

These are the highest-value features. They replace the manual GUI steps for the
known-good `sgpo_alt_face_a` proof.

### `bntx import-png`

Import a PNG into `timg/__Combined.bntx`.

Inputs:

- BNTX path
- PNG path
- target texture name

Expected behavior:

- add or replace a texture named `tex_sgpo_alt_face_a`;
- preserve alpha;
- use sane Switch Toolbox defaults for format/mipmaps/swizzle;
- save the updated BNTX.

Useful options:

```text
--input __Combined.bntx
--png face_A.png
--texture-name tex_sgpo_alt_face_a
--output __Combined.bntx
--replace
--dry-run
--json
```

### `bflyt add-texture-ref`

Add a BFLYT `txl1` texture reference.

Inputs:

- BFLYT path
- texture name

Expected behavior:

- add `tex_sgpo_alt_face_a` to `txl1`;
- no-op if it already exists, unless strict mode is requested;
- update offsets/section sizes/file size safely.

### `bflyt add-material`

Create or duplicate a material and bind it to a texture ref.

Inputs:

- BFLYT path
- material name
- texture name
- optional source/template material name

Expected behavior:

- create `mat_sgpo_alt_face_a`;
- bind its first texture map/reference to `tex_sgpo_alt_face_a`;
- keep neutral color values;
- keep alpha-capable settings;
- preferably clone from a known-good material and patch only name/texture.

The manual proof material currently validates as:

```text
material: mat_sgpo_alt_face_a
texture:  tex_sgpo_alt_face_a
flags:    25
wrap:     6/6
```

### `bflyt add-picture-pane`

Add a `pic1` pane under an existing parent pane.

Inputs:

- BFLYT path
- parent pane name
- pane name
- material name
- x/y/z
- width/height
- alpha
- optional source/template pane name

Expected behavior:

- add `sgpo_alt_face_a` under `sgpo_root`;
- set `material_idx` to `mat_sgpo_alt_face_a`;
- set transform and size;
- set alpha to `0` for initially hidden panes;
- preserve `pas1`/`pae1` tree structure.

Known one-pane proof target:

```text
parent:   sgpo_root
pane:     sgpo_alt_face_a
material: mat_sgpo_alt_face_a
x:        431.5
y:        137.5
z:        0
width:    99
height:   100
alpha:    0
```

### `bflyt set-pane-material`

Set a pane's material binding.

Inputs:

- BFLYT path
- pane name
- material name

Expected behavior:

- find the material index in `mat1`;
- write that index to the `pic1` pane's material index field;
- verify after writing.

SGPO already has a Python helper for this, but having it in the Rust CLI avoids
format drift.

## Priority 2: Inspection And Validation

These features make the converter safe and debuggable.

### `bflyt inspect`

Read a BFLYT and output JSON plus optional readable text.

Should include:

- BFLYT file metadata:
  - file size
  - section count
  - section list
- pane tree:
  - pane kind
  - pane name
  - parent name
  - transform
  - size
  - alpha/visibility
- `pic1` panes:
  - material index
  - material name
- `txl1` texture references
- `mat1` materials:
  - material index
  - material name
  - texture refs
  - flags/settings if available

### `bntx inspect`

Read a BNTX and output JSON plus optional readable text.

Should include:

- texture names
- width/height
- format
- mip count
- array/layer info if available
- alpha/channel information if available

### `layout validate-manifest`

Validate an unpacked layout directory against an SGPO skin manifest.

Inputs:

- unpacked layout directory
- manifest JSON

Expected checks:

- every manifest pane exists;
- every pane has the expected parent;
- every pane binds the expected material;
- every material references the expected texture;
- every texture ref exists in `txl1`;
- every texture exists in `__Combined.bntx`;
- texture dimensions match the manifest when BNTX metadata is available.

Output should make the first failure obvious.

## Priority 3: Batch Skin Generation

Once one PNG-backed pane is proven, SGPO needs to apply the same operation for
every manifest element.

### `layout apply-manifest`

Apply a generated SGPO skin manifest to an unpacked `info_melee` layout.

Inputs:

- unpacked layout directory
- manifest JSON
- source skin asset directory

Expected behavior:

- import all PNGs into `timg/__Combined.bntx`;
- add all BFLYT `txl1` refs;
- add all BFLYT `mat1` materials;
- add all `pic1` panes under `sgpo_root`;
- set pane transforms, sizes, alpha, and material bindings;
- validate the result.

This command can internally call the lower-level BNTX/BFLYT operations.

## Priority 4: Maintenance Operations

These are useful once skins are rebuilt repeatedly.

### Texture Replacement

Replace an existing texture by name without changing pane/material structure.

Use case:

- hot-swap a skin image while pane names and material names stay stable.

### Prefix Cleanup

Remove generated SGPO assets by prefix.

Examples:

```text
sgpo_alt_*
mat_sgpo_alt_*
tex_sgpo_alt_*
```

This is useful for rebuilding a layout cleanly without re-extracting from
`data.arc`.

### Rename Support

Rename related objects safely:

- pane names;
- material names;
- texture refs;
- BNTX texture names.

If a texture/material rename is performed, dependent references should update
together or the command should refuse unless explicitly told otherwise.

### Tree Operations

Move or reparent panes:

- move pane under a parent;
- insert before/after sibling;
- preserve `pas1`/`pae1` nesting correctly.

## Priority 5: Packaging

These are nice to have after the core asset editing is reliable.

### SARC Repack

Repack an unpacked layout directory into:

```text
ui/layout/info/info_melee/info_melee/layout.arc
```

Python `sarc` already works, so this is not urgent.

### Diff Reports

Produce JSON/Markdown diffs for:

- original vs modified BFLYT;
- original vs modified BNTX;
- manifest vs actual generated layout.

This helps confirm automation matches the known-good Switch Toolbox proof.

## Suggested Command Shape

Command names are suggestions, not strict requirements.

```bash
sgpo-layout-cli bntx inspect \
  --input unpacked/timg/__Combined.bntx \
  --json

sgpo-layout-cli bntx import-png \
  --input unpacked/timg/__Combined.bntx \
  --png "/path/to/face_A.png" \
  --texture-name tex_sgpo_alt_face_a \
  --output unpacked/timg/__Combined.bntx

sgpo-layout-cli bflyt add-texture-ref \
  --input unpacked/blyt/info_melee.bflyt \
  --texture-name tex_sgpo_alt_face_a \
  --output unpacked/blyt/info_melee.bflyt

sgpo-layout-cli bflyt add-material \
  --input unpacked/blyt/info_melee.bflyt \
  --material-name mat_sgpo_alt_face_a \
  --texture-name tex_sgpo_alt_face_a \
  --template-material set_rep_stock_01 \
  --output unpacked/blyt/info_melee.bflyt

sgpo-layout-cli bflyt add-picture-pane \
  --input unpacked/blyt/info_melee.bflyt \
  --parent sgpo_root \
  --pane sgpo_alt_face_a \
  --material mat_sgpo_alt_face_a \
  --template-pane sgpo_pro_a_marker \
  --x 431.5 \
  --y 137.5 \
  --z 0 \
  --width 99 \
  --height 100 \
  --alpha 0 \
  --output unpacked/blyt/info_melee.bflyt

sgpo-layout-cli layout validate-manifest \
  --layout-dir local-assets/proof/switch-pro-alt-one-pane/unpacked \
  --manifest target/skin-build/switch-pro-alt/skin_manifest.json \
  --json
```

## Minimum Acceptance Test

The first milestone is complete when the CLI can reproduce the manually created
one-pane proof:

```text
pane:     sgpo_alt_face_a
material: mat_sgpo_alt_face_a
texture:  tex_sgpo_alt_face_a
source:   face_A.png
parent:   sgpo_root
```

SGPO should then be able to run:

```bash
python tools/diff_one_pane_proof.py --fail-on-missing
```

and see:

```text
Pane sgpo_alt_face_a present: yes
Material mat_sgpo_alt_face_a present: yes
Pane binds to material: yes
Material references texture tex_sgpo_alt_face_a: yes
BNTX contains texture name tex_sgpo_alt_face_a: yes
```

After that, SGPO can runtime-test a temporary one-pane skin before attempting a
full RetroSpy skin conversion.

## Out Of Scope

The CLI does not need to:

- run on Switch;
- parse controller input;
- modify SGPO Rust runtime behavior;
- implement OBS/browser/network overlays;
- redistribute Nintendo assets or RetroSpy PNGs;
- generate arbitrary UI at runtime.
