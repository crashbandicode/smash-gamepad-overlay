#!/usr/bin/env python3
"""Add SGPO Pro Controller visual panes to an unpacked info_melee layout.

This patches the root match HUD and the P1 player-parts layouts. It does not
repack the SARC; use the Python `sarc` package after running this script.
"""

from __future__ import annotations

import argparse
import shutil
import struct
from pathlib import Path


ROOT_PANE_NAME = "sgpo_root"

SGPO_ROOT_POS = (760.0, -330.0, 0.0)
SGPO_ROOT_SIZE = (360.0, 300.0)
INITIAL_PANE_ALPHA = 0

ROOT_BFLYT = "info_melee.bflyt"
ROOT_MARKER_SOURCE_NAME = "set_rep_stock_01"
ROOT_MARKER_MATERIAL_SOURCE_NAME = None
PLAYER_PARTS_BFLYTS = [
    "info_melee_lct_player_00.bflyt",
    "info_melee_lct_player_01.bflyt",
]
PLAYER_PARTS_MARKER_SOURCE_NAME = "set_rep_01"
PLAYER_PARTS_MARKER_MATERIAL_SOURCE_NAME = "set_rep_stock_01"
PIC_VERTEX_COLOR_OFFSET = 8 + 0x4C
PIC_MATERIAL_INDEX_OFFSET = 8 + 0x5C
PIC_TEXTURE_COORD_COUNT_OFFSET = 8 + 0x5E

# Keep these names in sync with src/skin.rs. The A marker keeps its previously
# tested name to avoid changing the known-good hardware path.
PANE_SPECS = [
    ("sgpo_pro_lt", (-145.0, 122.0, 0.0), (54.0, 20.0)),
    ("sgpo_pro_lb", (-145.0, 95.0, 0.0), (54.0, 20.0)),
    ("sgpo_pro_rt", (65.0, 122.0, 0.0), (54.0, 20.0)),
    ("sgpo_pro_rb", (65.0, 95.0, 0.0), (54.0, 20.0)),
    ("sgpo_pro_minus", (-38.0, 45.0, 0.0), (20.0, 20.0)),
    ("sgpo_pro_plus", (20.0, 45.0, 0.0), (20.0, 20.0)),
    ("sgpo_pro_l3", (-68.0, -30.0, 0.0), (18.0, 18.0)),
    ("sgpo_pro_r3", (62.0, -100.0, 0.0), (18.0, 18.0)),
    ("sgpo_pro_ls_gate", (-105.0, -30.0, 0.0), (58.0, 58.0)),
    ("sgpo_pro_ls_dot", (-105.0, -30.0, 0.0), (14.0, 14.0)),
    ("sgpo_pro_rs_gate", (30.0, -100.0, 0.0), (58.0, 58.0)),
    ("sgpo_pro_rs_dot", (30.0, -100.0, 0.0), (14.0, 14.0)),
    ("sgpo_pro_du", (-105.0, -78.0, 0.0), (16.0, 16.0)),
    ("sgpo_pro_dd", (-105.0, -128.0, 0.0), (16.0, 16.0)),
    ("sgpo_pro_dl", (-130.0, -103.0, 0.0), (16.0, 16.0)),
    ("sgpo_pro_dr", (-80.0, -103.0, 0.0), (16.0, 16.0)),
    ("sgpo_pro_dul", (-130.0, -78.0, 0.0), (13.0, 13.0)),
    ("sgpo_pro_dur", (-80.0, -78.0, 0.0), (13.0, 13.0)),
    ("sgpo_pro_ddl", (-130.0, -128.0, 0.0), (13.0, 13.0)),
    ("sgpo_pro_ddr", (-80.0, -128.0, 0.0), (13.0, 13.0)),
    ("sgpo_pro_btn_y", (15.0, -20.0, 0.0), (24.0, 24.0)),
    ("sgpo_pro_btn_x", (45.0, 10.0, 0.0), (24.0, 24.0)),
    ("sgpo_pro_btn_b", (45.0, -50.0, 0.0), (24.0, 24.0)),
    ("sgpo_pro_a_marker", (75.0, -20.0, 0.0), (28.0, 28.0)),
]


def read_u32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def write_u16(data: bytearray, offset: int, value: int) -> None:
    struct.pack_into("<H", data, offset, value)


def write_u32(data: bytearray, offset: int, value: int) -> None:
    struct.pack_into("<I", data, offset, value)


def write_f32(data: bytearray, offset: int, value: float) -> None:
    struct.pack_into("<f", data, offset, value)


def section_count(data: bytes | bytearray) -> int:
    return struct.unpack_from("<H", data, 0x10)[0]


def file_size(data: bytes | bytearray) -> int:
    return struct.unpack_from("<I", data, 0x0C)[0]


def header_size(data: bytes | bytearray) -> int:
    return struct.unpack_from("<H", data, 0x06)[0]


def set_pane_name(section: bytearray, name: str) -> None:
    encoded = name.encode("ascii")
    if len(encoded) > 23:
        raise ValueError(f"pane name is too long: {name}")

    name_offset = 8 + 4
    section[name_offset : name_offset + 24] = b"\0" * 24
    section[name_offset : name_offset + len(encoded)] = encoded


def set_pane_alpha(section: bytearray, alpha: int) -> None:
    section[8 + 2] = alpha


def set_pane_transform(
    section: bytearray,
    pos: tuple[float, float, float],
    size: tuple[float, float],
) -> None:
    base = 8
    write_f32(section, base + 0x24, pos[0])
    write_f32(section, base + 0x28, pos[1])
    write_f32(section, base + 0x2C, pos[2])
    write_f32(section, base + 0x3C, 1.0)
    write_f32(section, base + 0x40, 1.0)
    write_f32(section, base + 0x44, size[0])
    write_f32(section, base + 0x48, size[1])


def set_picture_material_from_source(section: bytearray, material_source: bytes | bytearray) -> None:
    section[PIC_VERTEX_COLOR_OFFSET : PIC_VERTEX_COLOR_OFFSET + 16] = material_source[
        PIC_VERTEX_COLOR_OFFSET : PIC_VERTEX_COLOR_OFFSET + 16
    ]
    section[PIC_MATERIAL_INDEX_OFFSET : PIC_MATERIAL_INDEX_OFFSET + 2] = material_source[
        PIC_MATERIAL_INDEX_OFFSET : PIC_MATERIAL_INDEX_OFFSET + 2
    ]
    section[PIC_TEXTURE_COORD_COUNT_OFFSET] = material_source[PIC_TEXTURE_COORD_COUNT_OFFSET]


def make_picture_pane(
    source: bytes | bytearray,
    material_source: bytes | bytearray | None,
    name: str,
    pos: tuple[float, float, float],
    size: tuple[float, float],
    alpha: int,
) -> bytearray:
    pane = bytearray(source)
    set_pane_name(pane, name)
    set_pane_alpha(pane, alpha)
    set_pane_transform(pane, pos, size)
    if material_source is not None:
        set_picture_material_from_source(pane, material_source)
    return pane


def iter_sections(data: bytes | bytearray):
    offset = header_size(data)
    while offset + 8 <= len(data):
        tag = bytes(data[offset : offset + 4])
        size = read_u32(data, offset + 4)
        if size < 8 or offset + size > len(data):
            raise ValueError(f"invalid section at 0x{offset:x}")
        yield offset, tag, size
        offset += size


def find_section_by_pane_name(data: bytes | bytearray, tag: bytes, name: str) -> tuple[int, int]:
    expected = name.encode("ascii") + b"\0"
    for offset, section_tag, size in iter_sections(data):
        if section_tag != tag:
            continue
        pane_name = bytes(data[offset + 12 : offset + 36])
        if pane_name.startswith(expected):
            return offset, size
    raise ValueError(f"could not find {tag.decode()} pane {name}")


def find_root_close_offset(data: bytes | bytearray) -> int:
    depth = 0
    last_root_child_close = None
    for offset, tag, _size in iter_sections(data):
        if tag in {b"pan1", b"pic1", b"txt1", b"prt1", b"wnd1", b"bnd1"}:
            continue
        if tag == b"pas1":
            depth += 1
        elif tag == b"pae1":
            if depth == 1:
                return offset
            depth -= 1
            last_root_child_close = offset

    raise ValueError(f"could not find RootPane close; last pae1={last_root_child_close}")


def patch_bflyt(
    path: Path, marker_source_name: str, marker_material_source_name: str | None
) -> None:
    data = bytearray(path.read_bytes())
    if data[:4] != b"FLYT":
        raise ValueError(f"not a BFLYT file: {path}")
    if file_size(data) != len(data):
        raise ValueError(f"BFLYT header size does not match file length for {path}")
    pane_names = [name for name, _pos, _size in PANE_SPECS]
    expected_names = [ROOT_PANE_NAME, *pane_names]
    present_names = [
        name for name in expected_names if name.encode("ascii") + b"\0" in data
    ]
    if len(present_names) == len(expected_names):
        print(f"all SGPO Pro Controller panes already present; leaving {path} unchanged")
        return
    if present_names:
        raise ValueError(
            "source layout already contains a partial SGPO patch; "
            "use an unpatched source or regenerate local-assets/original"
        )

    root_offset, root_size = find_section_by_pane_name(data, b"pan1", "RootPane")
    marker_offset, marker_size = find_section_by_pane_name(
        data, b"pic1", marker_source_name
    )
    material_source = None
    if marker_material_source_name is not None:
        material_offset, material_size = find_section_by_pane_name(
            data, b"pic1", marker_material_source_name
        )
        material_source = data[material_offset : material_offset + material_size]

    sgpo_root = bytearray(data[root_offset : root_offset + root_size])
    set_pane_name(sgpo_root, ROOT_PANE_NAME)
    set_pane_alpha(sgpo_root, INITIAL_PANE_ALPHA)
    set_pane_transform(sgpo_root, SGPO_ROOT_POS, SGPO_ROOT_SIZE)

    marker_source = data[marker_offset : marker_offset + marker_size]

    pas1 = b"pas1" + struct.pack("<I", 8)
    pae1 = b"pae1" + struct.pack("<I", 8)
    markers = b"".join(
        make_picture_pane(marker_source, material_source, name, pos, size, INITIAL_PANE_ALPHA)
        for name, pos, size in PANE_SPECS
    )
    inserted = sgpo_root + pas1 + markers + pae1

    insert_offset = find_root_close_offset(data)
    patched = data[:insert_offset] + inserted + data[insert_offset:]

    write_u32(patched, 0x0C, len(patched))
    write_u16(patched, 0x10, section_count(data) + len(PANE_SPECS) + 3)
    path.write_bytes(patched)

    print(
        f"inserted {ROOT_PANE_NAME} with {len(PANE_SPECS)} visual panes into {path} "
        f"at 0x{insert_offset:x}"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--source",
        type=Path,
        default=Path("local-assets/original/info_melee/unpacked"),
        help="unpacked original info_melee layout directory",
    )
    parser.add_argument(
        "--dest",
        type=Path,
        default=Path("local-assets/modified/info_melee/unpacked"),
        help="destination unpacked layout directory to create",
    )
    args = parser.parse_args()

    if args.dest.exists():
        shutil.rmtree(args.dest)
    shutil.copytree(args.source, args.dest)

    blyt_dir = args.dest / "blyt"
    patch_bflyt(blyt_dir / ROOT_BFLYT, ROOT_MARKER_SOURCE_NAME, ROOT_MARKER_MATERIAL_SOURCE_NAME)
    for bflyt_name in PLAYER_PARTS_BFLYTS:
        patch_bflyt(
            blyt_dir / bflyt_name,
            PLAYER_PARTS_MARKER_SOURCE_NAME,
            PLAYER_PARTS_MARKER_MATERIAL_SOURCE_NAME,
        )


if __name__ == "__main__":
    main()
