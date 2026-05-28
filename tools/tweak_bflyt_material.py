#!/usr/bin/env python3
"""Inspect, rename, and rebind materials in a BFLYT (Cafe Layout) file.

Switch Toolbox auto-creates a duplicated material when a picture pane is
pasted, but does not expose a UI to rename it or rebind a pane. `bflyt-rs`
parses the `mat1` section as opaque bytes and cannot help either. This
script writes the new name into the material's fixed-length name slot or
overwrites a pane's `material_idx` directly, preserving every byte length,
section size and offset table in the file. It also exposes read-only
inspection modes for confirming that a pane references the expected
material.

Operations (mutually exclusive):

    --list                                 list all materials
    --list-panes                           list pic1/txt1 panes and their bound material
    --verify-pane <pane>                   verify a pane->material binding (use with --expected-material)
    --set-pane-material <pane>             rebind a pane to a different material (use with --target-material)
    (default)                              rename a material (requires --material-old-name + --material-new-name)

Examples:
    # Rename
    python tools/tweak_bflyt_material.py \
        --input  unpacked/blyt/info_melee.bflyt \
        --material-old-name sgpo_pro_a_marker_0 \
        --material-new-name mat_sgpo_alt_face_a

    # Inspect materials
    python tools/tweak_bflyt_material.py \
        --input unpacked/blyt/info_melee.bflyt --list

    # Inspect pane->material bindings
    python tools/tweak_bflyt_material.py \
        --input unpacked/blyt/info_melee.bflyt --list-panes

    # Assert a specific binding (exit 0 on match, non-zero otherwise)
    python tools/tweak_bflyt_material.py \
        --input unpacked/blyt/info_melee.bflyt \
        --verify-pane sgpo_alt_face_a \
        --expected-material mat_sgpo_alt_face_a

    # Rebind a pane to a specific material
    python tools/tweak_bflyt_material.py \
        --input unpacked/blyt/info_melee.bflyt \
        --set-pane-material sgpo_alt_face_a \
        --target-material mat_sgpo_alt_face_a

Defaults assume Switch BFLYT v8 (Smash Ultimate, etc.) where the material
name slot is 28 bytes. Pass --slot-size 20 for Wii U BFLYT v5.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

BFLYT_MAGIC = b"FLYT"
BOM_LE = 0xFEFF
BOM_BE = 0xFFFE

# Switch BFLYT v8 material name field (matches Switch Toolbox MAT1.cs:
#   Name = reader.ReadString(0x1C, true);
# 3DS v7 also uses 0x1C. Wii U v5 uses 0x14.
DEFAULT_SLOT_SIZE = 0x1C

# Pane-name field length inside ResPane. Switch v8 uses 24 bytes. This matches
# bflyt-rs ResPaneTest.name and Switch Toolbox CAFE Pane.cs.
PANE_NAME_SIZE = 0x18

# Section-data offsets for material_idx (relative to the section magic).
# section header (8) + pane (0x4C) + vtx_cols (0x10) = 0x64 for pic1.
# section header (8) + pane (0x4C) + text_buf_bytes (2) + text_str_bytes (2) = 0x58 for txt1.
PIC_MATERIAL_INDEX_OFFSET = 0x64
TXT_MATERIAL_INDEX_OFFSET = 0x58
PANE_NAME_OFFSET = 0x0C  # section header (8) + flag/base/alpha/flag_ex (4)


def parse_header(data: bytes) -> tuple[str, int, int]:
    """Return (endian, header_size, section_count) for a BFLYT header."""
    if len(data) < 0x14:
        raise ValueError("file is too small to be a BFLYT")
    if data[:4] != BFLYT_MAGIC:
        raise ValueError(f"not a BFLYT file (magic={data[:4]!r})")

    bom = struct.unpack("<H", data[4:6])[0]
    if bom == BOM_LE:
        endian = "<"
    elif bom == BOM_BE:
        endian = ">"
    else:
        raise ValueError(f"unknown BOM 0x{bom:04x}")

    header_size = struct.unpack(endian + "H", data[6:8])[0]
    section_count = struct.unpack(endian + "H", data[0x10:0x12])[0]
    file_size = struct.unpack(endian + "I", data[0x0C:0x10])[0]
    if file_size != len(data):
        raise ValueError(
            f"BFLYT header file size 0x{file_size:x} does not match actual 0x{len(data):x}"
        )
    if header_size < 0x14 or header_size > len(data):
        raise ValueError(f"invalid BFLYT header size 0x{header_size:x}")

    return endian, header_size, section_count


def find_mat1_section(
    data: bytes, endian: str, header_size: int, section_count: int
) -> tuple[int, int]:
    """Return (start_offset, size) of the mat1 section."""
    offset = header_size
    for _ in range(section_count):
        if offset + 8 > len(data):
            raise ValueError(f"section table is truncated at 0x{offset:x}")
        magic = data[offset : offset + 4]
        size = struct.unpack(endian + "I", data[offset + 4 : offset + 8])[0]
        if size < 8 or offset + size > len(data):
            raise ValueError(f"invalid section size 0x{size:x} at 0x{offset:x}")
        if magic == b"mat1":
            return offset, size
        offset += size
    raise ValueError("mat1 section not found")


def iter_sections(
    data: bytes, endian: str, header_size: int, section_count: int
):
    """Yield (offset, magic, size) for each top-level section."""
    offset = header_size
    for _ in range(section_count):
        magic = bytes(data[offset : offset + 4])
        size = struct.unpack(endian + "I", data[offset + 4 : offset + 8])[0]
        yield offset, magic, size
        offset += size


def read_pane_name(data: bytes, section_start: int) -> str:
    raw = data[
        section_start + PANE_NAME_OFFSET : section_start + PANE_NAME_OFFSET + PANE_NAME_SIZE
    ]
    return raw.split(b"\x00", 1)[0].decode("ascii", errors="replace")


def list_pane_material_bindings(
    data: bytes, endian: str, header_size: int, section_count: int
) -> list[tuple[str, str, int, int]]:
    """Return [(pane_kind, pane_name, section_offset, material_idx)].

    pane_kind is "pic1" or "txt1". Other pane kinds (pan1, prt1, wnd1, bnd1)
    are skipped because they either have no material (pan1, bnd1) or use a
    multi-material structure (wnd1, prt1) that this simple verifier does not
    decode.
    """
    out: list[tuple[str, str, int, int]] = []
    for sect_off, magic, _ in iter_sections(data, endian, header_size, section_count):
        if magic == b"pic1":
            mat_idx = struct.unpack(
                endian + "H",
                data[
                    sect_off + PIC_MATERIAL_INDEX_OFFSET : sect_off
                    + PIC_MATERIAL_INDEX_OFFSET
                    + 2
                ],
            )[0]
            out.append(("pic1", read_pane_name(data, sect_off), sect_off, mat_idx))
        elif magic == b"txt1":
            mat_idx = struct.unpack(
                endian + "H",
                data[
                    sect_off + TXT_MATERIAL_INDEX_OFFSET : sect_off
                    + TXT_MATERIAL_INDEX_OFFSET
                    + 2
                ],
            )[0]
            out.append(("txt1", read_pane_name(data, sect_off), sect_off, mat_idx))
    return out


def list_material_names(
    data: bytes, endian: str, mat1_start: int, slot_size: int
) -> list[tuple[int, str]]:
    """Return [(absolute_name_offset, name)] for every material in mat1.

    Layout (mat1 section):
        0x00 magic 'mat1'
        0x04 section_size : uint32
        0x08 material_count : uint16
        0x0A padding/flags  : uint16
        0x0C uint32[material_count] offsets, relative to start of mat1
    """
    mat1_size = struct.unpack(endian + "I", data[mat1_start + 4 : mat1_start + 8])[0]
    if mat1_size < 0x0C:
        raise ValueError(f"mat1 section is too small: 0x{mat1_size:x}")

    mat_count = struct.unpack(endian + "H", data[mat1_start + 8 : mat1_start + 10])[0]
    table_start = mat1_start + 0x0C
    if table_start + 4 * mat_count > mat1_start + mat1_size:
        raise ValueError("mat1 material offset table extends past section")

    offsets = struct.unpack(
        endian + f"{mat_count}I",
        data[table_start : table_start + 4 * mat_count],
    )

    out: list[tuple[int, str]] = []
    for off in offsets:
        name_abs = mat1_start + off
        if name_abs < mat1_start or name_abs + slot_size > mat1_start + mat1_size:
            raise ValueError(
                f"mat1 material name slot at 0x{name_abs:x} extends past section"
            )
        raw = data[name_abs : name_abs + slot_size]
        name = raw.split(b"\x00", 1)[0].decode("ascii", errors="replace")
        out.append((name_abs, name))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(
        description=(
            "Inspect, rename, or rebind materials in a BFLYT file. Renames "
            "overwrite the material's fixed-length name slot; rebinds "
            "overwrite a pane's material_idx (uint16). All section sizes and "
            "offsets are preserved."
        ),
    )
    ap.add_argument("--input", "-i", required=True, type=Path, help="Input .bflyt file.")
    ap.add_argument(
        "--output",
        "-o",
        type=Path,
        help="Output .bflyt path (default: overwrite --input).",
    )
    ap.add_argument(
        "--material-old-name",
        help="Existing material name to rename. Required unless --list is set.",
    )
    ap.add_argument(
        "--material-new-name",
        help="Replacement material name. Required unless --list is set.",
    )
    ap.add_argument(
        "--slot-size",
        type=int,
        default=DEFAULT_SLOT_SIZE,
        help=(
            "Material name slot size in bytes. "
            "28 (0x1C) for Switch v8 / 3DS v7+ (default), 20 (0x14) for Wii U v5."
        ),
    )
    ap.add_argument(
        "--list",
        action="store_true",
        help="List all materials in the file and exit (no writes).",
    )
    ap.add_argument(
        "--list-panes",
        action="store_true",
        help=(
            "List pic1/txt1 panes and the material name they bind to, then exit. "
            "Other pane kinds (pan1, prt1, wnd1, bnd1) are skipped."
        ),
    )
    ap.add_argument(
        "--verify-pane",
        metavar="PANE_NAME",
        help=(
            "Verify that pane PANE_NAME binds to --expected-material. "
            "Exits 0 on match, 1 on mismatch, 2 on missing pane / bad args."
        ),
    )
    ap.add_argument(
        "--expected-material",
        metavar="MATERIAL_NAME",
        help="Material name expected by --verify-pane.",
    )
    ap.add_argument(
        "--set-pane-material",
        metavar="PANE_NAME",
        help=(
            "Rebind pane PANE_NAME to the material named by --target-material. "
            "Writes the material's index into the pane's material_idx field. "
            "All section sizes/offsets are preserved."
        ),
    )
    ap.add_argument(
        "--target-material",
        metavar="MATERIAL_NAME",
        help="Material name to bind to (used by --set-pane-material).",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="Validate and report the planned change without writing the output file.",
    )
    args = ap.parse_args()

    src: Path = args.input
    if not src.is_file():
        print(f"error: input file not found: {src}", file=sys.stderr)
        return 2

    data = bytearray(src.read_bytes())
    endian, header_size, section_count = parse_header(bytes(data))
    mat1_start, mat1_size = find_mat1_section(
        bytes(data), endian, header_size, section_count
    )
    materials = list_material_names(bytes(data), endian, mat1_start, args.slot_size)

    if args.list:
        print(
            f"{src} (mat1 @ 0x{mat1_start:x}, "
            f"size={mat1_size}, count={len(materials)})"
        )
        for off, name in materials:
            print(f"  0x{off:08x}  {name}")
        return 0

    if args.list_panes:
        bindings = list_pane_material_bindings(
            bytes(data), endian, header_size, section_count
        )
        print(f"{src} (panes={len(bindings)}, materials={len(materials)})")
        for kind, pname, sect_off, mat_idx in bindings:
            mat_name = (
                materials[mat_idx][1] if 0 <= mat_idx < len(materials) else "<INVALID INDEX>"
            )
            print(
                f"  {kind} 0x{sect_off:08x}  {pname:<28s} material_idx={mat_idx:<4d} -> {mat_name}"
            )
        return 0

    if args.verify_pane:
        if not args.expected_material:
            print(
                "error: --verify-pane requires --expected-material",
                file=sys.stderr,
            )
            return 2
        bindings = list_pane_material_bindings(
            bytes(data), endian, header_size, section_count
        )
        hits = [b for b in bindings if b[1] == args.verify_pane]
        if not hits:
            print(
                f"error: pane '{args.verify_pane}' not found "
                f"(or is not pic1/txt1 — only those carry a material index)",
                file=sys.stderr,
            )
            return 2
        if len(hits) > 1:
            print(
                f"error: multiple panes named '{args.verify_pane}' (count={len(hits)})",
                file=sys.stderr,
            )
            return 2
        kind, pname, sect_off, mat_idx = hits[0]
        if not (0 <= mat_idx < len(materials)):
            print(
                f"FAIL: pane '{pname}' has invalid material_idx={mat_idx} "
                f"(materials count={len(materials)})",
                file=sys.stderr,
            )
            return 1
        actual = materials[mat_idx][1]
        if actual == args.expected_material:
            print(
                f"OK: {kind} '{pname}' -> material_idx={mat_idx} -> '{actual}'"
            )
            return 0
        print(
            f"FAIL: {kind} '{pname}' -> material_idx={mat_idx} -> '{actual}' "
            f"(expected '{args.expected_material}')",
            file=sys.stderr,
        )
        return 1

    if args.set_pane_material:
        if not args.target_material:
            print(
                "error: --set-pane-material requires --target-material",
                file=sys.stderr,
            )
            return 2

        bindings = list_pane_material_bindings(
            bytes(data), endian, header_size, section_count
        )
        pane_hits = [b for b in bindings if b[1] == args.set_pane_material]
        if not pane_hits:
            print(
                f"error: pane '{args.set_pane_material}' not found "
                f"(or is not pic1/txt1 — only those carry a material index)",
                file=sys.stderr,
            )
            return 2
        if len(pane_hits) > 1:
            print(
                f"error: multiple panes named '{args.set_pane_material}' "
                f"(count={len(pane_hits)})",
                file=sys.stderr,
            )
            return 2
        kind, pname, sect_off, current_idx = pane_hits[0]

        mat_hits = [
            (idx, name)
            for idx, (_, name) in enumerate(materials)
            if name == args.target_material
        ]
        if not mat_hits:
            print(
                f"error: material '{args.target_material}' not found in mat1",
                file=sys.stderr,
            )
            print("known materials:", file=sys.stderr)
            for _, name in materials:
                print(f"  - {name}", file=sys.stderr)
            return 1
        if len(mat_hits) > 1:
            print(
                f"error: multiple materials named '{args.target_material}' "
                f"(count={len(mat_hits)}); refusing to bind ambiguously",
                file=sys.stderr,
            )
            return 1
        target_idx, _ = mat_hits[0]
        if target_idx > 0xFFFF:
            print(
                f"error: target material index {target_idx} does not fit in uint16",
                file=sys.stderr,
            )
            return 1

        idx_field_off = sect_off + (
            PIC_MATERIAL_INDEX_OFFSET if kind == "pic1" else TXT_MATERIAL_INDEX_OFFSET
        )
        current_name = materials[current_idx][1] if 0 <= current_idx < len(materials) else "<INVALID>"

        print(
            f"rebinding {kind} '{pname}' (section 0x{sect_off:08x}, "
            f"material_idx field 0x{idx_field_off:08x}): "
            f"idx {current_idx} ('{current_name}') -> "
            f"idx {target_idx} ('{args.target_material}')"
        )

        if current_idx == target_idx:
            print("(already bound to the target material; no change needed)")
            return 0

        if args.dry_run:
            print("(dry run; no write performed)")
            return 0

        data[idx_field_off : idx_field_off + 2] = struct.pack(endian + "H", target_idx)

        dst: Path = args.output or args.input
        dst.parent.mkdir(parents=True, exist_ok=True)
        dst.write_bytes(bytes(data))
        print(f"wrote {dst}")
        return 0

    if not args.material_old_name or not args.material_new_name:
        print(
            "error: --material-old-name and --material-new-name are required "
            "(or pass --list / --list-panes / --verify-pane / --set-pane-material "
            "to inspect or rebind)",
            file=sys.stderr,
        )
        return 2

    old_name: str = args.material_old_name
    new_name: str = args.material_new_name

    new_bytes = new_name.encode("ascii")
    if len(new_bytes) >= args.slot_size:
        print(
            f"error: --material-new-name '{new_name}' is too long for "
            f"{args.slot_size}-byte slot (max {args.slot_size - 1} chars + null terminator)",
            file=sys.stderr,
        )
        return 2

    matches = [(off, name) for off, name in materials if name == old_name]
    if not matches:
        print(
            f"error: no material named '{old_name}' found in {src}",
            file=sys.stderr,
        )
        print("known materials:", file=sys.stderr)
        for _, name in materials:
            print(f"  - {name}", file=sys.stderr)
        return 1
    if len(matches) > 1:
        print(
            f"error: multiple materials named '{old_name}' "
            f"(count={len(matches)}); refusing to rename ambiguously",
            file=sys.stderr,
        )
        return 1

    name_off, _ = matches[0]
    new_slot = new_bytes.ljust(args.slot_size, b"\x00")

    print(
        f"renaming material at 0x{name_off:08x}: "
        f"'{old_name}' -> '{new_name}' (slot={args.slot_size} bytes)"
    )

    if args.dry_run:
        print("(dry run; no write performed)")
        return 0

    data[name_off : name_off + args.slot_size] = new_slot

    dst: Path = args.output or args.input
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_bytes(bytes(data))
    print(f"wrote {dst}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
