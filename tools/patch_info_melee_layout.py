#!/usr/bin/env python3
"""Add a minimal SGPO visual marker pane to an unpacked info_melee layout.

This patches only `blyt/info_melee.bflyt`. It does not repack the SARC; use the
Python `sarc` package after running this script.
"""

from __future__ import annotations

import argparse
import shutil
import struct
from pathlib import Path


ROOT_PANE_NAME = "sgpo_root"
MARKER_PANE_NAME = "sgpo_pro_a_marker"

SGPO_ROOT_POS = (760.0, -330.0, 0.0)
SGPO_ROOT_SIZE = (220.0, 160.0)
MARKER_POS = (0.0, 0.0, 0.0)
MARKER_SIZE = (28.0, 28.0)


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


def patch_bflyt(path: Path) -> None:
    data = bytearray(path.read_bytes())
    if data[:4] != b"FLYT":
        raise ValueError(f"not a BFLYT file: {path}")
    if file_size(data) != len(data):
        raise ValueError(f"BFLYT header size does not match file length for {path}")
    if MARKER_PANE_NAME.encode("ascii") in data:
        print(f"{MARKER_PANE_NAME} already present; leaving {path} unchanged")
        return

    root_offset, root_size = find_section_by_pane_name(data, b"pan1", "RootPane")
    stock_offset, stock_size = find_section_by_pane_name(data, b"pic1", "set_rep_stock_01")

    sgpo_root = bytearray(data[root_offset : root_offset + root_size])
    set_pane_name(sgpo_root, ROOT_PANE_NAME)
    set_pane_alpha(sgpo_root, 255)
    set_pane_transform(sgpo_root, SGPO_ROOT_POS, SGPO_ROOT_SIZE)

    marker = bytearray(data[stock_offset : stock_offset + stock_size])
    set_pane_name(marker, MARKER_PANE_NAME)
    set_pane_alpha(marker, 255)
    set_pane_transform(marker, MARKER_POS, MARKER_SIZE)

    pas1 = b"pas1" + struct.pack("<I", 8)
    pae1 = b"pae1" + struct.pack("<I", 8)
    inserted = sgpo_root + pas1 + marker + pae1

    insert_offset = find_root_close_offset(data)
    patched = data[:insert_offset] + inserted + data[insert_offset:]

    write_u32(patched, 0x0C, len(patched))
    write_u16(patched, 0x10, section_count(data) + 4)
    path.write_bytes(patched)

    print(
        f"inserted {ROOT_PANE_NAME}/{MARKER_PANE_NAME} into {path} "
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

    patch_bflyt(args.dest / "blyt" / "info_melee.bflyt")


if __name__ == "__main__":
    main()
