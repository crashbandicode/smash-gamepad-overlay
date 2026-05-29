#!/usr/bin/env python3
"""Compare a one-pane PNG proof layout against the current square layout.

This is read-only tooling for the RetroSpy skin converter path. It inspects the
known-good manual proof produced with Switch Toolbox and writes a repo-safe
Markdown report describing the BFLYT/BNTX deltas that a future converter must
learn to reproduce.

It intentionally does not write or repack Nintendo assets.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
from dataclasses import dataclass
from pathlib import Path


DEFAULT_ORIGINAL_DIR = Path("local-assets/modified/info_melee/unpacked")
DEFAULT_PROOF_DIR = Path("local-assets/proof/switch-pro-alt-one-pane/unpacked")
DEFAULT_OUT = Path("target/layout-inspection/one-pane-proof-diff.md")
DEFAULT_BFLYT = Path("blyt/info_melee.bflyt")
DEFAULT_BNTX = Path("timg/__Combined.bntx")
DEFAULT_PANE = "sgpo_alt_face_a"
DEFAULT_MATERIAL = "mat_sgpo_alt_face_a"
DEFAULT_TEXTURE = "tex_sgpo_alt_face_a"

BFLYT_MAGIC = b"FLYT"
BOM_LE = 0xFEFF
BOM_BE = 0xFFFE
PANE_SECTION_TAGS = {b"pan1", b"pic1", b"txt1", b"prt1", b"wnd1", b"bnd1"}
MATERIAL_NAME_SIZE = 0x1C
PANE_NAME_SIZE = 0x18
PANE_NAME_OFFSET = 0x0C
PANE_ALPHA_OFFSET = 8 + 2
PANE_POS_OFFSET = 8 + 0x24
PANE_SCALE_OFFSET = 8 + 0x3C
PANE_SIZE_OFFSET = 8 + 0x44
PIC_MATERIAL_INDEX_OFFSET = 8 + 0x5C
TXT_MATERIAL_INDEX_OFFSET = 8 + 0x50
MATERIAL_FLAGS_OFFSET = 0x1C
FIRST_TEXTURE_REF_OFFSET = 0x2C


@dataclass(frozen=True)
class Section:
    index: int
    offset: int
    magic: bytes
    size: int


@dataclass(frozen=True)
class TextureRef:
    texture_index: int
    texture_name: str
    wrap_s: int
    wrap_t: int


@dataclass(frozen=True)
class Material:
    index: int
    name: str
    offset: int
    size: int
    flags: int | None
    texture_refs: tuple[TextureRef, ...]


@dataclass(frozen=True)
class Pane:
    kind: str
    name: str
    parent: str | None
    offset: int
    size: int
    alpha: int | None
    pos: tuple[float, float, float] | None
    scale: tuple[float, float] | None
    pane_size: tuple[float, float] | None
    material_index: int | None


@dataclass(frozen=True)
class BflytSummary:
    path: Path
    file_size: int
    sha256: str
    section_count: int
    sections: tuple[Section, ...]
    texture_refs: tuple[str, ...]
    materials: tuple[Material, ...]
    panes: tuple[Pane, ...]


def main() -> int:
    args = parse_args()

    original_bflyt = args.original / args.bflyt
    proof_bflyt = args.proof / args.bflyt
    original_bntx = args.original / args.bntx
    proof_bntx = args.proof / args.bntx

    original = parse_bflyt(original_bflyt)
    proof = parse_bflyt(proof_bflyt)
    report = build_report(
        original=original,
        proof=proof,
        original_bntx=original_bntx,
        proof_bntx=proof_bntx,
        pane_name=args.pane,
        material_name=args.material,
        texture_name=args.texture,
    )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(report, encoding="utf-8")
    print(f"Wrote {args.out}")

    proof_ok = proof_contains_expected_binding(
        proof,
        pane_name=args.pane,
        material_name=args.material,
        texture_name=args.texture,
    )
    texture_in_bntx = proof_bntx.is_file() and proof_bntx.read_bytes().find(
        args.texture.encode("ascii")
    ) >= 0

    if args.fail_on_missing and not (proof_ok and texture_in_bntx):
        return 1
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inspect a manually created one-pane PNG proof layout."
    )
    parser.add_argument(
        "--original",
        type=Path,
        default=DEFAULT_ORIGINAL_DIR,
        help="Unpacked current square-pane layout directory.",
    )
    parser.add_argument(
        "--proof",
        type=Path,
        default=DEFAULT_PROOF_DIR,
        help="Unpacked one-pane proof layout directory.",
    )
    parser.add_argument(
        "--bflyt",
        type=Path,
        default=DEFAULT_BFLYT,
        help="BFLYT path relative to --original/--proof.",
    )
    parser.add_argument(
        "--bntx",
        type=Path,
        default=DEFAULT_BNTX,
        help="BNTX path relative to --original/--proof.",
    )
    parser.add_argument("--pane", default=DEFAULT_PANE)
    parser.add_argument("--material", default=DEFAULT_MATERIAL)
    parser.add_argument("--texture", default=DEFAULT_TEXTURE)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument(
        "--fail-on-missing",
        action="store_true",
        help="Exit non-zero if the expected proof pane/material/texture is missing.",
    )
    return parser.parse_args()


def parse_bflyt(path: Path) -> BflytSummary:
    data = path.read_bytes()
    endian, header_size, section_count, file_size = parse_header(data)
    sections = tuple(iter_sections(data, endian, header_size, section_count))
    textures = parse_txl1(data, endian, sections)
    materials = parse_mat1(data, endian, sections, textures)
    panes = parse_panes(data, endian, sections)

    return BflytSummary(
        path=path,
        file_size=file_size,
        sha256=hashlib.sha256(data).hexdigest(),
        section_count=section_count,
        sections=sections,
        texture_refs=tuple(textures),
        materials=tuple(materials),
        panes=tuple(panes),
    )


def parse_header(data: bytes) -> tuple[str, int, int, int]:
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
        raise ValueError(f"unknown BFLYT BOM 0x{bom:04x}")

    header_size = struct.unpack(endian + "H", data[6:8])[0]
    file_size = struct.unpack(endian + "I", data[0x0C:0x10])[0]
    section_count = struct.unpack(endian + "H", data[0x10:0x12])[0]
    if file_size != len(data):
        raise ValueError(
            f"BFLYT header file size 0x{file_size:x} does not match actual 0x{len(data):x}"
        )
    return endian, header_size, section_count, file_size


def iter_sections(
    data: bytes, endian: str, header_size: int, section_count: int
) -> tuple[Section, ...]:
    sections = []
    offset = header_size
    for index in range(section_count):
        if offset + 8 > len(data):
            raise ValueError(f"section table is truncated at 0x{offset:x}")
        magic = bytes(data[offset : offset + 4])
        size = struct.unpack(endian + "I", data[offset + 4 : offset + 8])[0]
        if size < 8 or offset + size > len(data):
            raise ValueError(f"invalid section size 0x{size:x} at 0x{offset:x}")
        sections.append(Section(index, offset, magic, size))
        offset += size
    if offset != len(data):
        raise ValueError(
            f"section walk ended at 0x{offset:x}, but file is 0x{len(data):x} bytes"
        )
    return tuple(sections)


def parse_txl1(data: bytes, endian: str, sections: tuple[Section, ...]) -> list[str]:
    section = first_section(sections, b"txl1")
    count = struct.unpack(endian + "I", data[section.offset + 8 : section.offset + 12])[0]
    table_start = section.offset + 0x0C
    table_end = table_start + count * 4
    if table_end > section.offset + section.size:
        raise ValueError("txl1 offset table extends past section")

    out = []
    # BFLYT txl1 offsets are relative to the start of the offset table, not to
    # the section start. This means the first string starts at
    # table_start + first_offset.
    for rel_offset in struct.unpack(endian + f"{count}I", data[table_start:table_end]):
        string_offset = table_start + rel_offset
        out.append(read_cstr(data, string_offset, section.offset + section.size))
    return out


def parse_mat1(
    data: bytes, endian: str, sections: tuple[Section, ...], texture_names: list[str]
) -> list[Material]:
    section = first_section(sections, b"mat1")
    count = struct.unpack(endian + "H", data[section.offset + 8 : section.offset + 10])[0]
    table_start = section.offset + 0x0C
    table_end = table_start + count * 4
    if table_end > section.offset + section.size:
        raise ValueError("mat1 offset table extends past section")

    rel_offsets = list(struct.unpack(endian + f"{count}I", data[table_start:table_end]))
    out = []
    for index, rel_offset in enumerate(rel_offsets):
        start = section.offset + rel_offset
        end = section.offset + (rel_offsets[index + 1] if index + 1 < count else section.size)
        if start + MATERIAL_NAME_SIZE > end:
            raise ValueError(f"material {index} name slot extends past material data")
        name = read_fixed_cstr(data, start, MATERIAL_NAME_SIZE)
        flags = (
            struct.unpack(endian + "I", data[start + MATERIAL_FLAGS_OFFSET : start + 0x20])[0]
            if start + 0x20 <= end
            else None
        )
        texture_refs = parse_material_texture_refs(data, endian, start, end, texture_names)
        out.append(Material(index, name, start, end - start, flags, tuple(texture_refs)))
    return out


def parse_material_texture_refs(
    data: bytes, endian: str, start: int, end: int, texture_names: list[str]
) -> list[TextureRef]:
    """Parse the first simple TexMap slot used by the known proof material.

    This is intentionally conservative. The current one-pane proof uses a
    one-texture material where bytes at +0x2C are:

        texture_index : u16
        wrap_s        : u8
        wrap_t        : u8

    Full material writing still belongs to a later, verified converter step.
    """
    if start + FIRST_TEXTURE_REF_OFFSET + 4 > end:
        return []

    texture_index = struct.unpack(
        endian + "H",
        data[start + FIRST_TEXTURE_REF_OFFSET : start + FIRST_TEXTURE_REF_OFFSET + 2],
    )[0]
    if texture_index >= len(texture_names):
        return []

    wrap_s = data[start + FIRST_TEXTURE_REF_OFFSET + 2]
    wrap_t = data[start + FIRST_TEXTURE_REF_OFFSET + 3]
    return [TextureRef(texture_index, texture_names[texture_index], wrap_s, wrap_t)]


def parse_panes(data: bytes, endian: str, sections: tuple[Section, ...]) -> list[Pane]:
    out: list[Pane] = []
    parent_stack: list[str] = []
    pending_parent: str | None = None

    for section in sections:
        if section.magic in PANE_SECTION_TAGS:
            name = read_fixed_cstr(data, section.offset + PANE_NAME_OFFSET, PANE_NAME_SIZE)
            material_index = pane_material_index(data, endian, section)
            out.append(
                Pane(
                    kind=section.magic.decode("ascii"),
                    name=name,
                    parent=parent_stack[-1] if parent_stack else None,
                    offset=section.offset,
                    size=section.size,
                    alpha=data[section.offset + PANE_ALPHA_OFFSET]
                    if section.offset + PANE_ALPHA_OFFSET < section.offset + section.size
                    else None,
                    pos=read_f32_tuple(data, section.offset + PANE_POS_OFFSET, 3, section),
                    scale=read_f32_tuple(data, section.offset + PANE_SCALE_OFFSET, 2, section),
                    pane_size=read_f32_tuple(data, section.offset + PANE_SIZE_OFFSET, 2, section),
                    material_index=material_index,
                )
            )
            pending_parent = name
        elif section.magic == b"pas1":
            if pending_parent is not None:
                parent_stack.append(pending_parent)
            pending_parent = None
        elif section.magic == b"pae1":
            if parent_stack:
                parent_stack.pop()
            pending_parent = None
        else:
            pending_parent = None

    return out


def pane_material_index(data: bytes, endian: str, section: Section) -> int | None:
    if section.magic == b"pic1":
        offset = section.offset + PIC_MATERIAL_INDEX_OFFSET
    elif section.magic == b"txt1":
        offset = section.offset + TXT_MATERIAL_INDEX_OFFSET
    else:
        return None
    if offset + 2 > section.offset + section.size:
        return None
    return struct.unpack(endian + "H", data[offset : offset + 2])[0]


def read_f32_tuple(
    data: bytes, offset: int, count: int, section: Section
) -> tuple[float, ...] | None:
    if offset + count * 4 > section.offset + section.size:
        return None
    return struct.unpack("<" + "f" * count, data[offset : offset + count * 4])


def first_section(sections: tuple[Section, ...], magic: bytes) -> Section:
    for section in sections:
        if section.magic == magic:
            return section
    raise ValueError(f"{magic.decode('ascii')} section not found")


def read_fixed_cstr(data: bytes, offset: int, size: int) -> str:
    raw = data[offset : offset + size]
    return raw.split(b"\0", 1)[0].decode("ascii", errors="replace")


def read_cstr(data: bytes, offset: int, limit: int) -> str:
    end = data.find(b"\0", offset, limit)
    if end < 0:
        raise ValueError(f"unterminated string at 0x{offset:x}")
    return data[offset:end].decode("ascii", errors="replace")


def build_report(
    original: BflytSummary,
    proof: BflytSummary,
    original_bntx: Path,
    proof_bntx: Path,
    pane_name: str,
    material_name: str,
    texture_name: str,
) -> str:
    original_bntx_summary = summarize_binary(original_bntx)
    proof_bntx_summary = summarize_binary(proof_bntx)
    proof_bntx_strings = extract_ascii_strings(proof_bntx.read_bytes()) if proof_bntx.is_file() else set()
    original_bntx_strings = (
        extract_ascii_strings(original_bntx.read_bytes()) if original_bntx.is_file() else set()
    )

    added_textures = sorted(set(proof.texture_refs) - set(original.texture_refs))
    added_materials = sorted(
        {m.name for m in proof.materials} - {m.name for m in original.materials}
    )
    added_panes = sorted(pane.name for pane in proof.panes)
    added_panes = sorted(set(added_panes) - {p.name for p in original.panes})
    added_bntx_strings = sorted(proof_bntx_strings - original_bntx_strings)

    pane = find_pane(proof, pane_name)
    material = find_material(proof, material_name)
    pane_material = (
        proof.materials[pane.material_index]
        if pane and pane.material_index is not None and pane.material_index < len(proof.materials)
        else None
    )
    expected_binding_ok = (
        pane is not None and material is not None and pane_material is not None
        and pane_material.name == material_name
    )
    expected_texture_ref_ok = material_contains_texture(material, texture_name)
    expected_bntx_ok = texture_name in proof_bntx_strings

    lines = [
        "# One-Pane PNG Proof Diff",
        "",
        "This report is generated from ignored local assets. It is repo-safe",
        "metadata only and does not contain Nintendo asset bytes or RetroSpy PNGs.",
        "",
        "## Inputs",
        "",
        f"- Original layout: `{original.path}`",
        f"- Proof layout: `{proof.path}`",
        f"- Original BNTX: `{original_bntx}`",
        f"- Proof BNTX: `{proof_bntx}`",
        "",
        "## Expected Proof Status",
        "",
        f"- Pane `{pane_name}` present: `{yes_no(pane is not None)}`",
        f"- Material `{material_name}` present: `{yes_no(material is not None)}`",
        f"- Pane binds to material: `{yes_no(expected_binding_ok)}`",
        f"- Material references texture `{texture_name}`: `{yes_no(expected_texture_ref_ok)}`",
        f"- BNTX contains texture name `{texture_name}`: `{yes_no(expected_bntx_ok)}`",
        "",
        "## File Deltas",
        "",
        "| File | Original size | Proof size | Delta | Original SHA-256 | Proof SHA-256 |",
        "| --- | ---: | ---: | ---: | --- | --- |",
        file_delta_row("BFLYT", original.file_size, proof.file_size, original.sha256, proof.sha256),
        file_delta_row(
            "BNTX",
            original_bntx_summary[0],
            proof_bntx_summary[0],
            original_bntx_summary[1],
            proof_bntx_summary[1],
        ),
        "",
        "## BFLYT Structural Deltas",
        "",
        f"- Section count: `{original.section_count}` -> `{proof.section_count}`",
        f"- `txl1` texture refs: `{len(original.texture_refs)}` -> `{len(proof.texture_refs)}`",
        f"- `mat1` materials: `{len(original.materials)}` -> `{len(proof.materials)}`",
        f"- panes with names: `{len(original.panes)}` -> `{len(proof.panes)}`",
        "",
        "Added `txl1` texture refs:",
        *bullet_list(added_textures),
        "",
        "Added materials:",
        *bullet_list(added_materials),
        "",
        "Added panes:",
        *bullet_list(added_panes),
        "",
        "## Target Pane",
        "",
        target_pane_table(pane, pane_material),
        "",
        "## Target Material",
        "",
        target_material_table(material),
        "",
        "## BNTX String Delta",
        "",
        "This is only a string-level BNTX check. It proves the imported texture",
        "name exists in the proof BNTX, but it does not decode image format,",
        "swizzle, mipmaps, or pixel payload.",
        "",
        *bullet_list([s for s in added_bntx_strings if s == texture_name] or added_bntx_strings),
        "",
        "## Converter Implications",
        "",
        "A future converter needs to reproduce these known-good changes:",
        "",
        f"- add BFLYT `txl1` texture reference `{texture_name}`;",
        f"- add BFLYT `mat1` material `{material_name}` with a texture map to `{texture_name}`;",
        f"- add `pic1` pane `{pane_name}` under `sgpo_root`; ",
        f"- set `{pane_name}` `material_idx` to the index of `{material_name}`;",
        "- import the PNG into `timg/__Combined.bntx` under the same texture name;",
        "- keep generated layout/BNTX outputs ignored until runtime-tested.",
        "",
        "The next automation step should still be narrow: generate or verify one",
        "PNG-backed pane end-to-end before attempting all RetroSpy controls.",
        "",
    ]
    return "\n".join(lines)


def proof_contains_expected_binding(
    proof: BflytSummary, pane_name: str, material_name: str, texture_name: str
) -> bool:
    pane = find_pane(proof, pane_name)
    material = find_material(proof, material_name)
    if pane is None or material is None or pane.material_index is None:
        return False
    if pane.material_index >= len(proof.materials):
        return False
    return proof.materials[pane.material_index].name == material_name and material_contains_texture(
        material, texture_name
    )


def summarize_binary(path: Path) -> tuple[int, str]:
    data = path.read_bytes()
    return len(data), hashlib.sha256(data).hexdigest()


def extract_ascii_strings(data: bytes) -> set[str]:
    return {
        match.decode("ascii", errors="replace")
        for match in re.findall(rb"[\x20-\x7e]{4,}", data)
    }


def find_pane(summary: BflytSummary, name: str) -> Pane | None:
    return next((pane for pane in summary.panes if pane.name == name), None)


def find_material(summary: BflytSummary, name: str) -> Material | None:
    return next((material for material in summary.materials if material.name == name), None)


def material_contains_texture(material: Material | None, texture_name: str) -> bool:
    if material is None:
        return False
    return any(ref.texture_name == texture_name for ref in material.texture_refs)


def yes_no(value: bool) -> str:
    return "yes" if value else "no"


def short_hash(value: str) -> str:
    return value[:12]


def file_delta_row(label: str, old_size: int, new_size: int, old_sha: str, new_sha: str) -> str:
    return (
        f"| {label} | {old_size} | {new_size} | {new_size - old_size:+d} | "
        f"`{short_hash(old_sha)}` | `{short_hash(new_sha)}` |"
    )


def bullet_list(values: list[str]) -> list[str]:
    if not values:
        return ["- none"]
    return [f"- `{value}`" for value in values]


def fmt_tuple(values: tuple[float, ...] | None) -> str:
    if values is None:
        return "-"
    return ", ".join(f"{value:g}" for value in values)


def target_pane_table(pane: Pane | None, material: Material | None) -> str:
    if pane is None:
        return "_Pane not found._"
    material_name = material.name if material is not None else "-"
    return "\n".join(
        [
            "| Field | Value |",
            "| --- | --- |",
            f"| kind | `{pane.kind}` |",
            f"| name | `{pane.name}` |",
            f"| parent | `{pane.parent or '-'}` |",
            f"| offset | `0x{pane.offset:x}` |",
            f"| section size | `{pane.size}` |",
            f"| alpha | `{pane.alpha}` |",
            f"| position | `{fmt_tuple(pane.pos)}` |",
            f"| scale | `{fmt_tuple(pane.scale)}` |",
            f"| size | `{fmt_tuple(pane.pane_size)}` |",
            f"| material index | `{pane.material_index}` |",
            f"| material name | `{material_name}` |",
        ]
    )


def target_material_table(material: Material | None) -> str:
    if material is None:
        return "_Material not found._"
    texture_refs = ", ".join(
        f"{ref.texture_index}:{ref.texture_name} wrap={ref.wrap_s}/{ref.wrap_t}"
        for ref in material.texture_refs
    )
    if not texture_refs:
        texture_refs = "-"
    return "\n".join(
        [
            "| Field | Value |",
            "| --- | --- |",
            f"| index | `{material.index}` |",
            f"| name | `{material.name}` |",
            f"| offset | `0x{material.offset:x}` |",
            f"| size | `{material.size}` |",
            f"| flags | `{material.flags}` |",
            f"| texture refs | `{texture_refs}` |",
        ]
    )


if __name__ == "__main__":
    raise SystemExit(main())
