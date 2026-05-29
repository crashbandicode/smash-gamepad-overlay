"""Synthetic tests for tools/diff_one_pane_proof.py.

The fixtures are tiny hand-built BFLYT-like files and fake BNTX blobs. They do
not contain Nintendo data.
"""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent.parent
SCRIPT = REPO_ROOT / "tools" / "diff_one_pane_proof.py"

spec = importlib.util.spec_from_file_location("diff_one_pane_proof", SCRIPT)
assert spec is not None and spec.loader is not None
diff = importlib.util.module_from_spec(spec)
sys.modules["diff_one_pane_proof"] = diff
spec.loader.exec_module(diff)


def section(tag: bytes, payload: bytes) -> bytes:
    return tag + (len(payload) + 8).to_bytes(4, "little") + payload


def fixed_name(name: str, size: int) -> bytes:
    encoded = name.encode("ascii")
    if len(encoded) >= size:
        raise ValueError(name)
    return encoded.ljust(size, b"\0")


def txl1(names: list[str]) -> bytes:
    table_start = 0x0C
    string_start_rel = len(names) * 4
    strings = bytearray()
    offsets = []
    for name in names:
        offsets.append(string_start_rel + len(strings))
        strings.extend(name.encode("ascii") + b"\0")
    while len(strings) % 4:
        strings.append(0)
    payload = (
        len(names).to_bytes(4, "little")
        + b"".join(offset.to_bytes(4, "little") for offset in offsets)
        + strings
    )
    assert table_start + offsets[0] == 0x0C + len(names) * 4
    return section(b"txl1", payload)


def material(name: str, texture_index: int = 0) -> bytes:
    data = bytearray(0x68)
    data[0:0x1C] = fixed_name(name, 0x1C)
    data[0x1C:0x20] = (0x19).to_bytes(4, "little")
    data[0x2C:0x2E] = texture_index.to_bytes(2, "little")
    data[0x2E] = 6
    data[0x2F] = 6
    return bytes(data)


def mat1(materials: list[bytes]) -> bytes:
    table_size = len(materials) * 4
    offsets = []
    cursor = 0x0C + table_size
    for mat in materials:
        offsets.append(cursor)
        cursor += len(mat)
    payload = (
        len(materials).to_bytes(2, "little")
        + b"\0\0"
        + b"".join(offset.to_bytes(4, "little") for offset in offsets)
        + b"".join(materials)
    )
    return section(b"mat1", payload)


def pan1(name: str) -> bytes:
    data = bytearray(0x54)
    data[4 : 4 + 0x18] = fixed_name(name, 0x18)
    return section(b"pan1", bytes(data))


def pic1(name: str, material_index: int) -> bytes:
    data = bytearray(0x60)
    data[2] = 0
    data[4 : 4 + 0x18] = fixed_name(name, 0x18)
    data[0x24:0x30] = float_triplet(431.5, 137.5, 0.0)
    data[0x3C:0x44] = float_pair(1.0, 1.0)
    data[0x44:0x4C] = float_pair(99.0, 100.0)
    data[0x5C:0x5E] = material_index.to_bytes(2, "little")
    return section(b"pic1", bytes(data))


def float_pair(a: float, b: float) -> bytes:
    import struct

    return struct.pack("<ff", a, b)


def float_triplet(a: float, b: float, c: float) -> bytes:
    import struct

    return struct.pack("<fff", a, b, c)


def tiny_bflyt(include_proof_pane: bool) -> bytes:
    sections = [
        txl1(["com_white32^s"] + (["tex_sgpo_alt_face_a"] if include_proof_pane else [])),
        mat1(
            [material("base_mat", 0)]
            + ([material("mat_sgpo_alt_face_a", 1)] if include_proof_pane else [])
        ),
        pan1("RootPane"),
        section(b"pas1", b""),
        pan1("sgpo_root"),
        section(b"pas1", b""),
    ]
    if include_proof_pane:
        sections.append(pic1("sgpo_alt_face_a", 1))
    sections.extend([section(b"pae1", b""), section(b"pae1", b"")])

    body = b"".join(sections)
    header_size = 0x14
    file_size = header_size + len(body)
    header = (
        b"FLYT"
        + b"\xff\xfe"
        + header_size.to_bytes(2, "little")
        + (9).to_bytes(4, "little")
        + file_size.to_bytes(4, "little")
        + len(sections).to_bytes(2, "little")
        + b"\0\0"
    )
    return header + body


class DiffOnePaneProofTest(unittest.TestCase):
    def test_parses_table_relative_txl1_offsets_and_material_binding(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "proof.bflyt"
            path.write_bytes(tiny_bflyt(include_proof_pane=True))

            summary = diff.parse_bflyt(path)

            self.assertEqual(
                summary.texture_refs,
                ("com_white32^s", "tex_sgpo_alt_face_a"),
            )
            self.assertTrue(
                diff.proof_contains_expected_binding(
                    summary,
                    "sgpo_alt_face_a",
                    "mat_sgpo_alt_face_a",
                    "tex_sgpo_alt_face_a",
                )
            )

    def test_report_lists_expected_one_pane_deltas(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original_bflyt = root / "original.bflyt"
            proof_bflyt = root / "proof.bflyt"
            original_bntx = root / "original.bntx"
            proof_bntx = root / "proof.bntx"
            original_bflyt.write_bytes(tiny_bflyt(include_proof_pane=False))
            proof_bflyt.write_bytes(tiny_bflyt(include_proof_pane=True))
            original_bntx.write_bytes(b"BNTX\0fake_original\0")
            proof_bntx.write_bytes(b"BNTX\0tex_sgpo_alt_face_a\0")

            report = diff.build_report(
                original=diff.parse_bflyt(original_bflyt),
                proof=diff.parse_bflyt(proof_bflyt),
                original_bntx=original_bntx,
                proof_bntx=proof_bntx,
                pane_name="sgpo_alt_face_a",
                material_name="mat_sgpo_alt_face_a",
                texture_name="tex_sgpo_alt_face_a",
            )

            self.assertIn("Pane `sgpo_alt_face_a` present: `yes`", report)
            self.assertIn("Material references texture `tex_sgpo_alt_face_a`: `yes`", report)
            self.assertIn("- `tex_sgpo_alt_face_a`", report)
            self.assertIn("- `mat_sgpo_alt_face_a`", report)
            self.assertIn("- `sgpo_alt_face_a`", report)


if __name__ == "__main__":
    unittest.main()
