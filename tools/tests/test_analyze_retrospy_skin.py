#!/usr/bin/env python3
"""Fixture-based tests for tools/analyze_retrospy_skin.py parsers.

These tests pin the regex contracts that translate src/input.rs and src/skin.rs
into Python data. They are the safety net that catches accidental drift in the
parser (formatting changes, signature reordering, builder renames) before the
analyzer becomes a layout asset generator.

Run from the repo root:

    python tools/tests/test_analyze_retrospy_skin.py

or via the standard unittest CLI:

    python -m unittest tools.tests.test_analyze_retrospy_skin
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
TOOLS_DIR = HERE.parent
REPO_ROOT = TOOLS_DIR.parent
FIXTURE_DIR = HERE / "fixtures"
REAL_INPUT_RS = REPO_ROOT / "src" / "input.rs"
REAL_SKIN_RS = REPO_ROOT / "src" / "skin.rs"
REAL_SWITCH_PRO_ALT_COUNT = 21

# Allow `python tools/tests/test_analyze_retrospy_skin.py` to import the parser
# module without requiring PYTHONPATH adjustments.
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from analyze_retrospy_skin import (  # noqa: E402  (sys.path tweak must come first)
    BuiltInElement,
    IMAGE_BUTTON_DEFAULTS,
    IMAGE_STICK_DEFAULTS,
    material_name,
    parse_builtin_buttons,
    parse_builtin_sticks,
    parse_control_ids,
    parse_switch_pro_alt_builtin,
    strip_rust_comments,
)


INPUT_FIXTURE = FIXTURE_DIR / "input_fixture.rs"
SKIN_FIXTURE = FIXTURE_DIR / "skin_fixture.rs"


class ParseControlIdsTest(unittest.TestCase):
    def test_extracts_expected_variants_in_order(self) -> None:
        variants = parse_control_ids(INPUT_FIXTURE)
        self.assertEqual(
            variants,
            [
                "A",
                "B",
                "X",
                "Y",
                "ZL",
                "ZR",
                "L3",
                "R3",
                "LeftStickGate",
                "LeftStickDot",
            ],
        )

    def test_skips_comments_attributes_and_blank_lines(self) -> None:
        text = INPUT_FIXTURE.read_text(encoding="utf-8")
        self.assertIn("// First face button.", text)
        self.assertIn("#[allow(dead_code)]", text)
        variants = parse_control_ids(INPUT_FIXTURE)
        # Ensure the parser did not turn skipped lines into bogus variants.
        self.assertNotIn("allow", variants)
        self.assertNotIn("First", variants)
        self.assertEqual(len(variants), 10)


class ParseSwitchProAltBuiltinTest(unittest.TestCase):
    def test_parses_buttons_and_stick_into_builtin_elements(self) -> None:
        elements = parse_switch_pro_alt_builtin(SKIN_FIXTURE)
        self.assertEqual(len(elements), 3)

        by_control = {element.control_id: element for element in elements}
        self.assertEqual(set(by_control), {"A", "B", "LeftStickDot"})

        face_a = by_control["A"]
        self.assertEqual(
            face_a,
            BuiltInElement(
                control_id="A",
                pane_name="sgpo_fixture_face_a",
                image="face_A.png",
                material_name=material_name("sgpo_fixture_face_a"),
                base_x=10.0,
                base_y=20.0,
                width=50.0,
                height=60.0,
                released_alpha=IMAGE_BUTTON_DEFAULTS["released_alpha"],
                pressed_alpha=IMAGE_BUTTON_DEFAULTS["pressed_alpha"],
                released_scale=IMAGE_BUTTON_DEFAULTS["released_scale"],
                pressed_scale=IMAGE_BUTTON_DEFAULTS["pressed_scale"],
            ),
        )

        face_b = by_control["B"]
        self.assertEqual(face_b.base_x, -10.5)
        self.assertEqual(face_b.base_y, -20.5)
        self.assertEqual(face_b.image, "face_B.png")
        self.assertIsNone(face_b.movement_x)
        self.assertIsNone(face_b.movement_y)

        left_stick = by_control["LeftStickDot"]
        self.assertEqual(left_stick.image, "stick_Left.png")
        self.assertEqual(left_stick.base_x, 100.0)
        self.assertEqual(left_stick.base_y, 200.0)
        self.assertEqual(left_stick.width, 80.0)
        self.assertEqual(left_stick.height, 80.0)
        self.assertEqual(left_stick.movement_x, 15.5)
        self.assertEqual(left_stick.movement_y, 25.5)
        self.assertEqual(left_stick.released_alpha, IMAGE_STICK_DEFAULTS["released_alpha"])
        self.assertEqual(left_stick.pressed_alpha, IMAGE_STICK_DEFAULTS["pressed_alpha"])
        self.assertEqual(left_stick.released_scale, IMAGE_STICK_DEFAULTS["released_scale"])
        self.assertEqual(left_stick.pressed_scale, IMAGE_STICK_DEFAULTS["pressed_scale"])

    def test_button_regex_does_not_match_stick_signature(self) -> None:
        # parse_builtin_buttons must skip the image_stick(...) entry because the
        # 7-argument button regex cannot match the 9-argument stick signature.
        body = SKIN_FIXTURE.read_text(encoding="utf-8")
        buttons = parse_builtin_buttons(body)
        self.assertEqual([button.control_id for button in buttons], ["A", "B"])

    def test_stick_regex_does_not_match_button_signature(self) -> None:
        body = SKIN_FIXTURE.read_text(encoding="utf-8")
        sticks = parse_builtin_sticks(body)
        self.assertEqual([stick.control_id for stick in sticks], ["LeftStickDot"])

    def test_missing_constant_raises_clearly(self) -> None:
        empty = HERE / "_empty_skin_fixture.rs"
        empty.write_text(
            "// no SWITCH_PRO_ALT_ELEMENTS block here\n",
            encoding="utf-8",
        )
        try:
            with self.assertRaises(ValueError) as captured:
                parse_switch_pro_alt_builtin(empty)
            self.assertIn("SWITCH_PRO_ALT_ELEMENTS", str(captured.exception))
        finally:
            empty.unlink(missing_ok=True)


class StripRustCommentsTest(unittest.TestCase):
    def test_removes_line_comments(self) -> None:
        text = "let x = 1; // trailing\nlet y = 2;\n// full line\nlet z = 3;"
        self.assertEqual(
            strip_rust_comments(text),
            "let x = 1; \nlet y = 2;\n\nlet z = 3;",
        )

    def test_removes_block_comments(self) -> None:
        text = "let a = 1; /* inline */ let b = 2;\n/* multi\nline */\nlet c = 3;"
        self.assertEqual(
            strip_rust_comments(text),
            "let a = 1;  let b = 2;\n\nlet c = 3;",
        )

    def test_strips_bracketing_chars_inside_comment(self) -> None:
        text = (
            "// the body looks like `= [ ... ];` but it is inside a comment\n"
            "const SWITCH_PRO_ALT_ELEMENTS: [SkinElement; 0] = [];\n"
        )
        stripped = strip_rust_comments(text)
        # Comment is gone, so the substring search will land on the real const.
        self.assertNotIn("// the body looks like", stripped)
        self.assertIn("const SWITCH_PRO_ALT_ELEMENTS", stripped)


class RealSourceParseTest(unittest.TestCase):
    """Backstop that runs the parser against the actual src/*.rs sources.

    Fixture tests can pass even when the regex contract has drifted from the
    real code (e.g. if a builder is renamed). These tests pin the live
    expectation: parse_switch_pro_alt_builtin must return exactly 21 elements
    and parse_control_ids must return at least LOGICAL_CONTROL_COUNT variants
    derived from src/input.rs.
    """

    def test_switch_pro_alt_builtin_round_trips_full_skin(self) -> None:
        elements = parse_switch_pro_alt_builtin(REAL_SKIN_RS)
        self.assertEqual(
            len(elements),
            REAL_SWITCH_PRO_ALT_COUNT,
            f"parse_switch_pro_alt_builtin returned {len(elements)} elements; "
            f"expected {REAL_SWITCH_PRO_ALT_COUNT}. The image_static/image_button/image_stick "
            "regex is likely out of sync with src/skin.rs.",
        )

    def test_control_ids_match_logical_control_count(self) -> None:
        variants = parse_control_ids(REAL_INPUT_RS)
        rust_count = _logical_control_count_from_input_rs(REAL_INPUT_RS)
        self.assertEqual(
            len(variants),
            rust_count,
            "parse_control_ids variant count drifted from LOGICAL_CONTROL_COUNT "
            f"in {REAL_INPUT_RS}",
        )


def _logical_control_count_from_input_rs(input_rs: Path) -> int:
    """Read LOGICAL_CONTROL_COUNT directly from src/input.rs without importing Rust."""
    import re

    text = input_rs.read_text(encoding="utf-8")
    match = re.search(
        r"pub\(crate\)\s+const\s+LOGICAL_CONTROL_COUNT\s*:\s*usize\s*=\s*(\d+)\s*;",
        text,
    )
    if not match:
        raise AssertionError(f"Could not find LOGICAL_CONTROL_COUNT in {input_rs}")
    return int(match.group(1))


if __name__ == "__main__":
    unittest.main(verbosity=2)
