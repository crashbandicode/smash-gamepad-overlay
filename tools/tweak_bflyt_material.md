# Tweak BFLYT Material — Agent Runbook

Companion runbook for `tools/tweak_bflyt_material.py`. Read this end-to-end
before invoking the script.

## Purpose

This script does three things:

1. **Rename** one material entry in a BFLYT (Cafe Layout) file in place.
   Used after Switch Toolbox auto-creates a duplicated material on Ctrl+V
   (the GUI does not expose a material rename UI), and as a substitute for
   `bflyt-rs` which stores the `mat1` section as opaque bytes.
2. **Verify** that a named pane (`pic1` / `txt1`) binds to the expected
   material via its `material_idx`. Used to confirm the rename did not break
   the pane→material reference, and to assert the spec was met.
3. **Rebind** a pane to a specific material. Used when the pane's
   `material_idx` points at the wrong entry — for example, after a manual
   Switch Toolbox edit, or when paste reused an unintended source material.

Renames overwrite the material's fixed-length name slot directly. Byte
length, section sizes, and offsets are preserved.

Rebinds overwrite the pane's `material_idx` (uint16). Both old and new
indices are uint16, so byte length is preserved and no offsets shift.

Verification follows the pane's `material_idx` into the `mat1` section's
offset table and reads the resolved material's name slot. It does **not**
look up by string anywhere — it only inspects the actual binary references
the runtime would use.

## Prerequisites

- Python 3.8+ (no third-party packages required).
- A BFLYT file the agent can read and (for rename / rebind) write.
- The agent already knows the **target new material name** (e.g.
  `mat_sgpo_alt_face_a`) and the **target pane name** (e.g.
  `sgpo_alt_face_a`) from upstream task spec.
- The agent does **not** know the **current material name** in the file. The
  current name is whatever Switch Toolbox auto-assigned when the source pane
  was pasted, typically `<source_pane_name>_0`, `<source_pane_name>_1`, etc.
  It must be discovered with `--list` (Step 1 below), not guessed.

## Workflow

Run from the repo root (`/home/intpa/smash-gamepad-overlay`).

Replace `<BFLYT_PATH>` with the actual file path, e.g.
`local-assets/proof/switch-pro-alt-one-pane/unpacked/blyt/info_melee.bflyt`.

### Step 1 — Inspect materials

```bash
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --list
```

Prints every material name and its byte offset. Use the output to determine
the exact `--material-old-name` for Step 3. Do **not** assume the old name;
read it from this output.

### Step 2 — Inspect pane→material bindings

```bash
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --list-panes
```

Prints each `pic1` / `txt1` pane with its bound material name (resolved
through `material_idx`). Confirms which material the new pane is currently
referencing **before** the rename — this is the entry whose name needs to
change.

Output looks like:

```text
pic1 0x000123ab  sgpo_alt_face_a              material_idx=42   -> sgpo_pro_a_marker_0
```

In this example the agent should rename `sgpo_pro_a_marker_0` to
`mat_sgpo_alt_face_a` in Step 4.

### Step 3 — Dry run the rename

```bash
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --material-old-name <NAME_FROM_STEP_2> \
  --material-new-name mat_sgpo_alt_face_a \
  --dry-run
```

Validates that:

- Exactly one material has the old name.
- The new name fits in the 28-byte slot.
- The mat1 section parses cleanly.

No file is written. Stop and resolve any reported error before continuing.

### Step 4 — Apply the rename

```bash
python tools/tweak_bflyt_material.py \
  --input  <BFLYT_PATH> \
  --material-old-name <NAME_FROM_STEP_2> \
  --material-new-name mat_sgpo_alt_face_a
```

If `--output` is omitted, the script overwrites `--input`. Pass `--output
<OTHER_PATH>` to write a new file instead.

### Step 5 — Verify the binding

This is the authoritative check. It exits 0 if and only if the named pane's
`material_idx` resolves to the expected material name in the on-disk file.

```bash
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --verify-pane sgpo_alt_face_a \
  --expected-material mat_sgpo_alt_face_a
```

Exit codes:

- **0** — `OK: pic1 'sgpo_alt_face_a' -> material_idx=N -> 'mat_sgpo_alt_face_a'`
- **1** — `FAIL: ... -> '<actual>' (expected '<expected>')`. Bindings are
  wrong. Continue to Step 5b to fix.
- **2** — pane not found, or invocation error. Re-check pane name with
  `--list-panes`.

Also confirm the string is present at the binary level:

```bash
strings -a <BFLYT_PATH> | rg 'mat_sgpo_alt_face_a'
```

### Step 5b — Rebind the pane (only if Step 5 failed)

If `--verify-pane` exits 1, the pane's `material_idx` points at a different
material than expected. This happens when, for example, Ctrl+V duplicated
the source pane but Switch Toolbox did not auto-clone the material (rare),
or when manual edits changed the index.

Rebind the pane explicitly:

```bash
# Dry run first — confirms the named target material exists and is unique.
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --set-pane-material sgpo_alt_face_a \
  --target-material mat_sgpo_alt_face_a \
  --dry-run

# Apply.
python tools/tweak_bflyt_material.py \
  --input <BFLYT_PATH> \
  --set-pane-material sgpo_alt_face_a \
  --target-material mat_sgpo_alt_face_a
```

What this does: looks up `mat_sgpo_alt_face_a` in the `mat1` offset table to
get its index, then writes that uint16 into the pane's `material_idx` field.
File size, section sizes, and all offsets are preserved.

After rebinding, **re-run Step 5** (`--verify-pane`) to confirm.

### Caveat: Use rebind only as a fix, not as a workflow shortcut

The intended workflow is:

1. Paste pane in Switch Toolbox (auto-clones material).
2. Rename the cloned material to the spec name (Step 4 above).
3. Verify the binding is intact (Step 5).

If Step 5 passes, the rename already preserved the binding because the
`material_idx` was untouched. Step 5b is **only** for the case where the
binding was wrong before the rename, or the pane is referencing a different
material entirely. Do not skip the paste-and-rename flow in favor of
"create-then-bind" — Switch Toolbox does the heavy lifting (per-pane vertex
colors, texture map structures, blending, etc.) that this script does not
recreate.

### Step 6 — Repack the SARC

The script edits the unpacked `.bflyt` only. The bundle archive must be
regenerated for the change to take effect at runtime:

```bash
python -m sarc create \
  --base-path "$PWD/local-assets/proof/switch-pro-alt-one-pane/unpacked" \
  "$PWD/local-assets/proof/switch-pro-alt-one-pane/unpacked" \
  "$PWD/local-assets/proof/switch-pro-alt-one-pane/layout.arc"
```

(Path-substitute as appropriate for the actual workdir.)

## Constraints (do not violate)

1. **Slot size.** Switch BFLYT v8 (Smash Ultimate, MK8DX, etc.) uses a 28-byte
   material name slot — the script default. Pass `--slot-size 20` only for
   Wii U BFLYT v5. 3DS v7+ keeps default 28.
2. **Name length.** New name must be strictly shorter than the slot size
   (default: max 27 ASCII chars). `mat_sgpo_alt_face_a` is 19 chars and fits.
3. **Always inspect first.** Run `--list` and `--list-panes` before
   renaming. Switch Toolbox paste suffixes are not deterministic.
4. **Always `--dry-run` before applying** (rename or rebind).
5. **Always run `--verify-pane` after applying.** A successful rename is not
   the same as a correct binding — verify the pane still references the
   intended material.
6. **Use rebind (`--set-pane-material`) only when verification fails.** It is
   a corrective tool, not a substitute for the paste-and-rename workflow.
7. **Repack the SARC after editing.** A renamed/rebinded BFLYT inside an
   unpacked directory has no runtime effect until the archive is rebuilt.

## Failure Modes

| Symptom | Likely cause | Resolution |
|---|---|---|
| `error: no material named '...' found` | Old name was guessed, not read from `--list` | Re-run `--list`, copy the exact string |
| `error: multiple materials named '...'` | Two materials share the same name (rare) | Disambiguate by hex offset, or rename source in Switch Toolbox before pasting again |
| `error: --material-new-name '...' is too long` | New name exceeds 27 ASCII chars | Shorten the name |
| `not a BFLYT file (magic=...)` | Path points at a SARC/SZS or wrong file | Unpack the archive first, then point at the `.bflyt` inside |
| `mat1 section not found` | File is truncated or not a layout | Verify file integrity; re-extract from source |
| `--verify-pane` exit 1 (`FAIL`) | The pane's `material_idx` resolves to a different material than expected | Either rename that resolved material to the expected name, or use Step 5b (`--set-pane-material`) to rebind the pane to the correct material |
| `--verify-pane` exit 2, "pane not found" | Pane name typo, or the pane is `wnd1` / `prt1` / `pan1` (not supported by the verifier) | Confirm the pane is a `pic1` (picture pane) — only those and `txt1` have a single material index this verifier reads |
| `--set-pane-material` "material not found" | Target material doesn't exist in mat1, or the rename step was skipped | Run `--list` to see what's actually in the file. If the spec material is missing, rename an existing material to the spec name (Step 4) before rebinding |
| `--set-pane-material` "multiple materials named '...'" | Two materials share the same name | Disambiguate first: rename one of them to a unique name, then rebind |

## What this script does NOT do

- Does **not** add a new material entry. Switch Toolbox already creates the
  cloned material on Ctrl+V; this script only renames, rebinds, and
  inspects.
- Does **not** verify or rebind `wnd1` (window panes) or `prt1` (parts).
  Those use a more complex multi-material structure. This script supports
  `pic1` and `txt1` only, which is what the proof workflow needs.
- Does **not** touch BNTX (texture data), texture-list entries, SARC, or any
  other file in the bundle. Use the existing tools for those concerns.

## Where this fits in the broader workflow

Slot Steps 1–5 of this runbook into `switch-toolbox-one-pane-proof.md`
between **"Save `info_melee.bflyt`"** (the Switch Toolbox manual save) and
**"Repack Proof Layout"** (the `python -m sarc create` invocation, which
corresponds to Step 6 here).
