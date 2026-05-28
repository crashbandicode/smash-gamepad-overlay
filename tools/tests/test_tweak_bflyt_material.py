#!/usr/bin/env python3
"""Synthetic tests for tools/tweak_bflyt_material.py.

The fixture is a tiny hand-built BFLYT-like file with valid pic1 and mat1
sections. It contains no Nintendo data.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent.parent
SCRIPT = REPO_ROOT / "tools" / "tweak_bflyt_material.py"
SLOT_SIZE = 0x1C


def material_slot(name: str) -> bytes:
    encoded = name.encode("ascii")
    if len(encoded) >= SLOT_SIZE:
        raise ValueError(name)
    return encoded.ljust(SLOT_SIZE, b"\0")


def tiny_bflyt() -> bytes:
    header_size = 0x14
    pic1 = bytearray(b"pic1" + (0x68).to_bytes(4, "little") + b"\0" * (0x68 - 8))
    pic1[0x0C : 0x0C + len(b"fixture_pane")] = b"fixture_pane"
    pic1[0x64:0x66] = (0).to_bytes(2, "little")

    material_count = 2
    offsets = [0x0C + 4 * material_count, 0x0C + 4 * material_count + SLOT_SIZE]
    mat1_payload = (
        material_count.to_bytes(2, "little")
        + b"\0\0"
        + b"".join(offset.to_bytes(4, "little") for offset in offsets)
        + material_slot("old_material")
        + material_slot("other_material")
    )
    mat1 = b"mat1" + (8 + len(mat1_payload)).to_bytes(4, "little") + mat1_payload
    file_size = header_size + len(pic1) + len(mat1)
    header = (
        b"FLYT"
        + b"\xff\xfe"
        + header_size.to_bytes(2, "little")
        + (9).to_bytes(4, "little")
        + file_size.to_bytes(4, "little")
        + (2).to_bytes(2, "little")
        + b"\0\0"
    )
    return header + pic1 + mat1


def run_tool(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise AssertionError(
            f"command failed with {result.returncode}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


class RenameBflytMaterialTest(unittest.TestCase):
    def test_lists_materials_with_offsets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            result = run_tool("--input", str(path), "--list")

            self.assertIn("old_material", result.stdout)
            self.assertIn("other_material", result.stdout)
            self.assertIn("mat1 @ 0x7c", result.stdout)

    def test_dry_run_does_not_write(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            original = tiny_bflyt()
            path.write_bytes(original)

            result = run_tool(
                "--input",
                str(path),
                "--material-old-name",
                "old_material",
                "--material-new-name",
                "mat_sgpo_alt_face_a",
                "--dry-run",
            )

            self.assertIn("dry run", result.stdout)
            self.assertEqual(path.read_bytes(), original)

    def test_renames_one_material_in_place(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            run_tool(
                "--input",
                str(path),
                "--material-old-name",
                "old_material",
                "--material-new-name",
                "mat_sgpo_alt_face_a",
            )

            result = run_tool("--input", str(path), "--list")
            self.assertIn("mat_sgpo_alt_face_a", result.stdout)
            self.assertNotIn("old_material", result.stdout)
            self.assertIn("other_material", result.stdout)

    def test_rejects_overlong_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            result = run_tool(
                "--input",
                str(path),
                "--material-old-name",
                "old_material",
                "--material-new-name",
                "x" * SLOT_SIZE,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("too long", result.stderr)


class TweakBflytPaneMaterialTest(unittest.TestCase):
    def test_lists_pane_material_bindings(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            result = run_tool("--input", str(path), "--list-panes")

            self.assertIn("fixture_pane", result.stdout)
            self.assertIn("material_idx=0", result.stdout)
            self.assertIn("old_material", result.stdout)

    def test_verifies_expected_pane_binding(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            result = run_tool(
                "--input",
                str(path),
                "--verify-pane",
                "fixture_pane",
                "--expected-material",
                "old_material",
            )

            self.assertIn("OK", result.stdout)

    def test_rejects_mismatched_pane_binding(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            result = run_tool(
                "--input",
                str(path),
                "--verify-pane",
                "fixture_pane",
                "--expected-material",
                "other_material",
                check=False,
            )

            self.assertEqual(result.returncode, 1)
            self.assertIn("FAIL", result.stderr)

    def test_rebinds_pane_material(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "fixture.bflyt"
            path.write_bytes(tiny_bflyt())

            run_tool(
                "--input",
                str(path),
                "--set-pane-material",
                "fixture_pane",
                "--target-material",
                "other_material",
            )

            result = run_tool(
                "--input",
                str(path),
                "--verify-pane",
                "fixture_pane",
                "--expected-material",
                "other_material",
            )
            self.assertIn("OK", result.stdout)


if __name__ == "__main__":
    unittest.main()
