#!/usr/bin/env python3
"""Analyze a RetroSpy-style skin.xml against the SGPO skin model.

This is a PC-side converter foundation only. It reads skin metadata and writes
an analysis report; it does not generate Smash assets or runtime skin files.

Do not redistribute the RetroSpy PNG assets or any Nintendo layout files
parsed by this script unless their license and redistribution path are handled
separately. The outputs under target/ are repo-safe metadata only.

The RetroSpy skin directory is supplied either via --skin-dir on the command
line or by setting SGPO_RETROSPY_SKIN_DIR in a local .env file (see
.env.example).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import struct
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from stage_arcropolis_layout import load_dotenv


SKIN_DIR_ENV_VAR = "SGPO_RETROSPY_SKIN_DIR"
DEFAULT_REPORT_OUT = Path("target/skin-analysis/switch-pro-alt.md")
DEFAULT_MANIFEST_DIR = Path("target/skin-build/switch-pro-alt")
DEFAULT_DOTENV = Path(".env")
GENERATED_SKIN_NAME = "switch_pro_alt_builtin"
ROOT_PANE_NAME = "sgpo_root"
EXPECTED_LAYOUT_FLAVOR = (
    "switch_pro_alt_builtin generated asset panes from the future skin converter"
)

IMAGE_BUTTON_DEFAULTS = {
    "released_alpha": 70,
    "pressed_alpha": 255,
    "released_scale": 1.0,
    "pressed_scale": 1.05,
}

IMAGE_STICK_DEFAULTS = {
    "released_alpha": 255,
    "pressed_alpha": 255,
    "released_scale": 1.0,
    "pressed_scale": 1.0,
}

CONTROL_ORDER = [
    "A",
    "B",
    "X",
    "Y",
    "L",
    "R",
    "ZL",
    "ZR",
    "L3",
    "R3",
    "Plus",
    "Minus",
    "Home",
    "Capture",
    "DpadUp",
    "DpadDown",
    "DpadLeft",
    "DpadRight",
    "DpadUpLeft",
    "DpadUpRight",
    "DpadDownLeft",
    "DpadDownRight",
    "LeftStickGate",
    "LeftStickDot",
    "RightStickGate",
    "RightStickDot",
    "GcLTrigger",
    "GcRTrigger",
]

SECTION_BUTTON_MAPS = {
    "switch": {
        "a": "A",
        "b": "B",
        "x": "X",
        "y": "Y",
        "l": "L",
        "r": "R",
        "zl": "ZL",
        "zr": "ZR",
        "ls": "L3",
        "rs": "R3",
        "+": "Plus",
        "-": "Minus",
        "home": "Home",
        "capture": "Capture",
    },
    "pc360": {
        "a": "A",
        "b": "B",
        "x": "X",
        "y": "Y",
        "l": "L",
        "r": "R",
        "trig_l_d": "ZL",
        "trig_r_d": "ZR",
        "l3": "L3",
        "r3": "R3",
        "start": "Plus",
        "back": "Minus",
    },
}

GLOBAL_BUTTON_MAP = {
    "up": "DpadUp",
    "down": "DpadDown",
    "left": "DpadLeft",
    "right": "DpadRight",
}

STICK_MAP = {
    ("lstick_x", "lstick_y"): "LeftStickDot",
    ("rstick_x", "rstick_y"): "RightStickDot",
}

RUNTIME_NEUTRAL_CONTROLS = {"Home", "Capture"}
FLOAT_RE = r"[-+]?\d+(?:\.\d+)?"


@dataclass(frozen=True)
class SkinMetadata:
    name: str
    author: str
    types: tuple[str, ...]
    background_image: str
    background_width: int
    background_height: int


@dataclass(frozen=True)
class ParsedControl:
    kind: str
    section: str
    retrospy_name: str
    control_id: str | None
    image: str
    x: float
    y: float
    width: float
    height: float
    base_x: float
    base_y: float
    movement_x: float | None = None
    movement_y: float | None = None


@dataclass(frozen=True)
class BuiltInElement:
    control_id: str
    pane_name: str
    image: str
    material_name: str
    base_x: float
    base_y: float
    width: float
    height: float
    released_alpha: int
    pressed_alpha: int
    released_scale: float
    pressed_scale: float
    movement_x: float | None = None
    movement_y: float | None = None


def main() -> int:
    args = parse_args()
    load_dotenv(args.env_file)
    skin_dir = resolve_skin_dir(args.skin_dir)
    skin_xml = skin_dir / "skin.xml"
    if not skin_xml.is_file():
        raise SystemExit(
            f"missing skin.xml at {skin_xml}; check --skin-dir or {SKIN_DIR_ENV_VAR}"
        )
    metadata, controls = parse_skin(skin_xml, args.section)
    control_ids = parse_control_ids(args.input_rs)
    builtin = parse_switch_pro_alt_builtin(args.skin_rs)
    manifest = build_manifest(skin_xml, args.section, metadata, controls, builtin)
    manifest_validation_errors = validate_manifest_against_builtin(
        manifest["elements"], builtin
    )

    report = build_report(
        skin_xml=skin_xml,
        section=args.section,
        metadata=metadata,
        controls=controls,
        control_ids=control_ids,
        builtin=builtin,
        skin_rs=args.skin_rs,
        manifest=manifest,
        manifest_dir=args.manifest_dir,
        manifest_validation_errors=manifest_validation_errors,
    )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(report, encoding="utf-8")
    write_manifest_files(args.manifest_dir, manifest, manifest_validation_errors)
    if args.rust_preview:
        write_rust_preview(args.manifest_dir, manifest)

    mapped = [control for control in controls if control.control_id]
    unmapped = [control for control in controls if not control.control_id]
    print(f"Wrote {args.out}")
    print(f"Wrote {args.manifest_dir / 'skin_manifest.json'}")
    print(f"Wrote {args.manifest_dir / 'skin_manifest.md'}")
    if args.rust_preview:
        print(f"Wrote {args.manifest_dir / 'skin_manifest_preview.rs'}")
    print(
        f"Parsed {len(controls)} controls ({len(mapped)} mapped, "
        f"{len(unmapped)} unsupported/unmapped) from {skin_xml}"
    )
    print(f"Compared against {len(builtin)} switch_pro_alt_builtin elements")
    if manifest_validation_errors:
        print("Generated manifest does not match switch_pro_alt_builtin:")
        for error in manifest_validation_errors:
            print(f"  - {error}")
        return 1
    print("Generated manifest exactly matches switch_pro_alt_builtin")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Analyze a RetroSpy-style skin.xml against SGPO SkinElement data."
    )
    parser.add_argument(
        "--skin-dir",
        type=Path,
        default=None,
        help=(
            "RetroSpy skin directory containing skin.xml. If omitted, falls back "
            f"to ${SKIN_DIR_ENV_VAR} from the environment or .env file."
        ),
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=DEFAULT_DOTENV,
        help=f"Optional local env file providing {SKIN_DIR_ENV_VAR} (default: .env)",
    )
    parser.add_argument(
        "--section",
        default="switch",
        help="RetroSpy <section type=...> to analyze (default: switch)",
    )
    parser.add_argument(
        "--input-rs",
        type=Path,
        default=Path("src/input.rs"),
        help="Rust source containing ControlId (default: src/input.rs)",
    )
    parser.add_argument(
        "--skin-rs",
        type=Path,
        default=Path("src/skin.rs"),
        help="Rust source containing switch_pro_alt_builtin (default: src/skin.rs)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_REPORT_OUT,
        help=f"Markdown report path (default: {DEFAULT_REPORT_OUT})",
    )
    parser.add_argument(
        "--manifest-dir",
        type=Path,
        default=DEFAULT_MANIFEST_DIR,
        help=f"Dry-run manifest output directory (default: {DEFAULT_MANIFEST_DIR})",
    )
    parser.add_argument(
        "--rust-preview",
        action="store_true",
        help="Also write a generated Rust preview table under the manifest directory.",
    )
    return parser.parse_args()


def resolve_skin_dir(arg_value: Path | None) -> Path:
    if arg_value is not None:
        return arg_value

    env_value = os.environ.get(SKIN_DIR_ENV_VAR, "").strip()
    if env_value:
        return Path(env_value)

    raise SystemExit(
        f"--skin-dir is required (or set {SKIN_DIR_ENV_VAR} in .env). "
        "RetroSpy typically stores skins under .../RetroSpy/skins/<skin-name>."
    )


def parse_skin(skin_xml: Path, selected_section: str) -> tuple[SkinMetadata, list[ParsedControl]]:
    try:
        tree = ET.parse(skin_xml)
    except ET.ParseError as error:
        raise SystemExit(f"{skin_xml} is not a valid RetroSpy skin.xml: {error}") from error
    root = tree.getroot()

    skin_dir = skin_xml.parent
    background = root.find("background")
    background_image = background.get("image") if background is not None else ""
    background_width, background_height = png_dimensions(skin_dir / background_image)
    metadata = SkinMetadata(
        name=root.get("name", ""),
        author=root.get("author", ""),
        types=tuple(filter(None, root.get("type", "").split(";"))),
        background_image=background_image,
        background_width=background_width,
        background_height=background_height,
    )

    controls: list[ParsedControl] = []
    section = find_section(root, selected_section)
    if section is not None:
        section_map = SECTION_BUTTON_MAPS.get(selected_section, {})
        for button in section.findall("button"):
            controls.append(
                parsed_button(
                    button,
                    section=selected_section,
                    control_id=section_map.get(button.get("name", "")),
                    background_width=background_width,
                    background_height=background_height,
                )
            )

    for button in root.findall("button"):
        controls.append(
            parsed_button(
                button,
                section="global",
                control_id=GLOBAL_BUTTON_MAP.get(button.get("name", "")),
                background_width=background_width,
                background_height=background_height,
            )
        )

    for stick in root.findall("stick"):
        xname = stick.get("xname", "")
        yname = stick.get("yname", "")
        controls.append(
            parsed_stick(
                stick,
                section="global",
                control_id=STICK_MAP.get((xname, yname)),
                background_width=background_width,
                background_height=background_height,
            )
        )

    return metadata, controls


def find_section(root: ET.Element, selected_section: str) -> ET.Element | None:
    for section in root.findall("section"):
        section_types = set(filter(None, section.get("type", "").split(";")))
        if selected_section in section_types:
            return section
    return None


def parsed_button(
    element: ET.Element,
    section: str,
    control_id: str | None,
    background_width: int,
    background_height: int,
) -> ParsedControl:
    x, y, width, height = rect_attrs(element)
    base_x, base_y = centered_position(x, y, width, height, background_width, background_height)
    return ParsedControl(
        kind="button",
        section=section,
        retrospy_name=element.get("name", ""),
        control_id=control_id,
        image=element.get("image", ""),
        x=x,
        y=y,
        width=width,
        height=height,
        base_x=base_x,
        base_y=base_y,
    )


def parsed_stick(
    element: ET.Element,
    section: str,
    control_id: str | None,
    background_width: int,
    background_height: int,
) -> ParsedControl:
    x, y, width, height = rect_attrs(element)
    base_x, base_y = centered_position(x, y, width, height, background_width, background_height)
    return ParsedControl(
        kind="stick",
        section=section,
        retrospy_name=f"{element.get('xname', '')}/{element.get('yname', '')}",
        control_id=control_id,
        image=element.get("image", ""),
        x=x,
        y=y,
        width=width,
        height=height,
        base_x=base_x,
        base_y=base_y,
        movement_x=float_attr(element, "xrange"),
        movement_y=float_attr(element, "yrange"),
    )


def rect_attrs(element: ET.Element) -> tuple[float, float, float, float]:
    return (
        float_attr(element, "x"),
        float_attr(element, "y"),
        float_attr(element, "width"),
        float_attr(element, "height"),
    )


def float_attr(element: ET.Element, name: str) -> float:
    value = element.get(name)
    if value is None:
        raise ValueError(f"<{element.tag}> missing required {name}= attribute")
    return float(value)


def centered_position(
    x: float,
    y: float,
    width: float,
    height: float,
    background_width: int,
    background_height: int,
) -> tuple[float, float]:
    return (
        x + width / 2.0 - background_width / 2.0,
        background_height / 2.0 - (y + height / 2.0),
    )


def png_dimensions(path: Path) -> tuple[int, int]:
    try:
        with path.open("rb") as file:
            header = file.read(24)
    except FileNotFoundError as error:
        raise SystemExit(
            f"missing background PNG at {path}; check --skin-dir or {SKIN_DIR_ENV_VAR}"
        ) from error

    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise SystemExit(f"{path} is not a valid PNG file")

    return struct.unpack(">II", header[16:24])


def parse_control_ids(input_rs: Path) -> list[str]:
    text = input_rs.read_text(encoding="utf-8")
    match = re.search(r"enum\s+ControlId\s*\{(?P<body>.*?)^\}", text, flags=re.S | re.M)
    if not match:
        raise ValueError(f"Could not find ControlId enum in {input_rs}")

    variants: list[str] = []
    for line in match.group("body").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("#["):
            continue
        variant_match = re.match(r"([A-Za-z][A-Za-z0-9_]*)\s*,", stripped)
        if variant_match:
            variants.append(variant_match.group(1))

    return variants


_LINE_COMMENT_RE = re.compile(r"//[^\n]*")
_BLOCK_COMMENT_RE = re.compile(r"/\*.*?\*/", re.S)


def strip_rust_comments(text: str) -> str:
    """Remove `//` line comments and non-nested `/* */` block comments.

    Not string-literal aware: a `//` inside a `"..."` string would still be
    stripped. Acceptable for this project because pane-name and image-name
    strings in src/skin.rs never contain `//` or `/*`.
    """
    no_block = _BLOCK_COMMENT_RE.sub("", text)
    return _LINE_COMMENT_RE.sub("", no_block)


def parse_switch_pro_alt_builtin(skin_rs: Path) -> list[BuiltInElement]:
    # Strip comments first so the substring search for `= [` and `];` cannot
    # match characters inside an explanatory doc comment.
    text = strip_rust_comments(skin_rs.read_text(encoding="utf-8"))
    start = text.find("const SWITCH_PRO_ALT_ELEMENTS")
    if start < 0:
        raise ValueError(f"Could not find SWITCH_PRO_ALT_ELEMENTS in {skin_rs}")

    body_start = text.find("= [", start)
    body_end = text.find("];", body_start)
    if body_start < 0 or body_end < 0:
        raise ValueError(f"Could not isolate SWITCH_PRO_ALT_ELEMENTS body in {skin_rs}")
    body = text[body_start:body_end]

    elements: list[BuiltInElement] = []
    elements.extend(parse_builtin_buttons(body))
    elements.extend(parse_builtin_sticks(body))
    return elements


def parse_builtin_buttons(body: str) -> list[BuiltInElement]:
    pattern = re.compile(
        rf"""image_button\(
            \s*ControlId::(?P<control>\w+),
            \s*b"(?P<pane>[^"]+)\\0",
            \s*"(?P<image>[^"]+)",
            \s*(?P<base_x>{FLOAT_RE}),
            \s*(?P<base_y>{FLOAT_RE}),
            \s*(?P<width>{FLOAT_RE}),
            \s*(?P<height>{FLOAT_RE}),
            \s*\)""",
        flags=re.S | re.X,
    )
    return [
        BuiltInElement(
            control_id=match.group("control"),
            pane_name=match.group("pane"),
            image=match.group("image"),
            material_name=material_name(match.group("pane")),
            base_x=float(match.group("base_x")),
            base_y=float(match.group("base_y")),
            width=float(match.group("width")),
            height=float(match.group("height")),
            released_alpha=IMAGE_BUTTON_DEFAULTS["released_alpha"],
            pressed_alpha=IMAGE_BUTTON_DEFAULTS["pressed_alpha"],
            released_scale=IMAGE_BUTTON_DEFAULTS["released_scale"],
            pressed_scale=IMAGE_BUTTON_DEFAULTS["pressed_scale"],
        )
        for match in pattern.finditer(body)
    ]


def parse_builtin_sticks(body: str) -> list[BuiltInElement]:
    pattern = re.compile(
        rf"""image_stick\(
            \s*ControlId::(?P<control>\w+),
            \s*b"(?P<pane>[^"]+)\\0",
            \s*"(?P<image>[^"]+)",
            \s*(?P<base_x>{FLOAT_RE}),
            \s*(?P<base_y>{FLOAT_RE}),
            \s*(?P<width>{FLOAT_RE}),
            \s*(?P<height>{FLOAT_RE}),
            \s*(?P<movement_x>{FLOAT_RE}),
            \s*(?P<movement_y>{FLOAT_RE}),
            \s*\)""",
        flags=re.S | re.X,
    )
    return [
        BuiltInElement(
            control_id=match.group("control"),
            pane_name=match.group("pane"),
            image=match.group("image"),
            material_name=material_name(match.group("pane")),
            base_x=float(match.group("base_x")),
            base_y=float(match.group("base_y")),
            width=float(match.group("width")),
            height=float(match.group("height")),
            released_alpha=IMAGE_STICK_DEFAULTS["released_alpha"],
            pressed_alpha=IMAGE_STICK_DEFAULTS["pressed_alpha"],
            released_scale=IMAGE_STICK_DEFAULTS["released_scale"],
            pressed_scale=IMAGE_STICK_DEFAULTS["pressed_scale"],
            movement_x=float(match.group("movement_x")),
            movement_y=float(match.group("movement_y")),
        )
        for match in pattern.finditer(body)
    ]


def build_manifest(
    skin_xml: Path,
    section: str,
    metadata: SkinMetadata,
    controls: list[ParsedControl],
    builtin: list[BuiltInElement],
) -> dict:
    builtin_by_control = {element.control_id: element for element in builtin}
    mapped = sorted(
        [control for control in controls if control.control_id],
        key=lambda item: control_sort_key(item.control_id or ""),
    )

    return {
        "schema_version": 1,
        "skin_name": GENERATED_SKIN_NAME,
        "root_pane_name": ROOT_PANE_NAME,
        "expected_layout_flavor": EXPECTED_LAYOUT_FLAVOR,
        "source": {
            "skin_xml": str(skin_xml),
            "retrospy_skin_name": metadata.name,
            "retrospy_author": metadata.author,
            "retrospy_types": list(metadata.types),
            "section": section,
            "background_image": metadata.background_image,
            "background_width": metadata.background_width,
            "background_height": metadata.background_height,
        },
        "coordinate_space": {
            "origin": "background_center",
            "base_x_formula": "x + width / 2 - background_width / 2",
            "base_y_formula": "background_height / 2 - (y + height / 2)",
        },
        "defaults": {
            "image_button": IMAGE_BUTTON_DEFAULTS,
            "image_stick": IMAGE_STICK_DEFAULTS,
        },
        "elements": [
            manifest_element(control, builtin_by_control.get(control.control_id or ""))
            for control in mapped
        ],
    }


def manifest_element(
    control: ParsedControl,
    builtin_element: BuiltInElement | None,
) -> dict:
    pane_name = builtin_element.pane_name if builtin_element else suggested_pane_name(control)
    defaults = IMAGE_STICK_DEFAULTS if control.kind == "stick" else IMAGE_BUTTON_DEFAULTS
    return {
        "control_id": control.control_id,
        "pane_name": pane_name,
        "image_filename": control.image,
        "material_name": material_name(pane_name),
        "source": {
            "retrospy_kind": control.kind,
            "retrospy_section": control.section,
            "retrospy_name": control.retrospy_name,
            "x": control.x,
            "y": control.y,
        },
        "base_x": control.base_x,
        "base_y": control.base_y,
        "width": control.width,
        "height": control.height,
        "released_alpha": defaults["released_alpha"],
        "pressed_alpha": defaults["pressed_alpha"],
        "released_scale": defaults["released_scale"],
        "pressed_scale": defaults["pressed_scale"],
        "stick_movement": stick_movement_dict(control),
    }


def stick_movement_dict(control: ParsedControl) -> dict[str, float] | None:
    if control.movement_x is None or control.movement_y is None:
        return None
    return {"x": control.movement_x, "y": control.movement_y}


def write_manifest_files(
    manifest_dir: Path,
    manifest: dict,
    validation_errors: list[str],
) -> None:
    manifest_dir.mkdir(parents=True, exist_ok=True)
    json_path = manifest_dir / "skin_manifest.json"
    md_path = manifest_dir / "skin_manifest.md"

    json_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    md_path.write_text(build_manifest_markdown(manifest, validation_errors), encoding="utf-8")


def build_manifest_markdown(manifest: dict, validation_errors: list[str]) -> str:
    lines: list[str] = []
    lines.append(f"# Skin Manifest: {manifest['skin_name']}")
    lines.append("")
    lines.append("Generated by `tools/analyze_retrospy_skin.py`.")
    lines.append("")
    lines.append("## Source")
    lines.append("")
    source = manifest["source"]
    lines.append(f"- Skin XML: `{source['skin_xml']}`")
    lines.append(f"- RetroSpy skin: `{source['retrospy_skin_name']}`")
    lines.append(f"- Section: `{source['section']}`")
    lines.append(
        f"- Background: `{source['background_image']}` "
        f"({source['background_width']}x{source['background_height']})"
    )
    lines.append(f"- Root pane: `{manifest['root_pane_name']}`")
    lines.append(f"- Expected layout flavor: `{manifest['expected_layout_flavor']}`")
    lines.append("")

    lines.append("## Validation")
    lines.append("")
    if validation_errors:
        for error in validation_errors:
            lines.append(f"- Mismatch: {error}")
    else:
        lines.append("- Generated manifest exactly matches `switch_pro_alt_builtin`.")
    lines.append("")

    lines.append("## Elements")
    lines.append("")
    lines.append(
        "| ControlId | Pane | Image | Material | Base x/y | Size | Alpha released/pressed | Scale released/pressed | Stick movement |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for element in manifest["elements"]:
        stick_movement = element["stick_movement"]
        if stick_movement is None:
            movement = "-"
        else:
            movement = f"{fmt(stick_movement['x'])}, {fmt(stick_movement['y'])}"
        lines.append(
            "| "
            + " | ".join(
                [
                    code(element["control_id"]),
                    code(element["pane_name"]),
                    code(element["image_filename"]),
                    code(element["material_name"]),
                    f"{fmt(element['base_x'])}, {fmt(element['base_y'])}",
                    f"{fmt(element['width'])} x {fmt(element['height'])}",
                    f"{element['released_alpha']} / {element['pressed_alpha']}",
                    f"{fmt(element['released_scale'])} / {fmt(element['pressed_scale'])}",
                    movement,
                ]
            )
            + " |"
        )
    lines.append("")
    lines.append("## Dry-Run Boundary")
    lines.append("")
    lines.append("- This manifest is repo-safe metadata only.")
    lines.append("- It does not write modified `layout.arc` files or generated Smash assets.")
    lines.append("- PNGs and Nintendo layout assets remain external to git/release artifacts.")
    lines.append("")
    return "\n".join(lines)


def write_rust_preview(manifest_dir: Path, manifest: dict) -> None:
    lines: list[str] = []
    lines.append("// Generated dry-run preview only. Do not paste blindly into runtime code.")
    lines.append("// Source: tools/analyze_retrospy_skin.py")
    lines.append("")
    lines.append("const GENERATED_SWITCH_PRO_ALT_ELEMENTS: &[SkinElement] = &[")
    for element in manifest["elements"]:
        function = "image_stick" if element["stick_movement"] else "image_button"
        args = [
            f"ControlId::{element['control_id']}",
            f"b\"{element['pane_name']}\\0\"",
            f"\"{element['image_filename']}\"",
            f"{element['base_x']:.1f}",
            f"{element['base_y']:.1f}",
            f"{element['width']:.1f}",
            f"{element['height']:.1f}",
        ]
        if element["stick_movement"]:
            args.append(f"{element['stick_movement']['x']:.1f}")
            args.append(f"{element['stick_movement']['y']:.1f}")
        lines.append(f"    {function}({', '.join(args)}),")
    lines.append("];")
    lines.append("")
    (manifest_dir / "skin_manifest_preview.rs").write_text(
        "\n".join(lines), encoding="utf-8"
    )


def validate_manifest_against_builtin(
    manifest_elements: list[dict],
    builtin: list[BuiltInElement],
) -> list[str]:
    errors: list[str] = []
    manifest_by_control = {
        element["control_id"]: element for element in manifest_elements
    }
    builtin_by_control = {element.control_id: element for element in builtin}

    for control_id in sorted_controls(set(manifest_by_control) - set(builtin_by_control)):
        errors.append(f"{control_id}: present in manifest but missing from built-in")
    for control_id in sorted_controls(set(builtin_by_control) - set(manifest_by_control)):
        errors.append(f"{control_id}: present in built-in but missing from manifest")

    for control_id in sorted_controls(set(manifest_by_control) & set(builtin_by_control)):
        manifest_element = manifest_by_control[control_id]
        builtin_element = builtin_by_control[control_id]
        movement = manifest_element["stick_movement"]
        manifest_movement_x = None if movement is None else movement["x"]
        manifest_movement_y = None if movement is None else movement["y"]
        checks = [
            ("pane_name", diff_string(manifest_element["pane_name"], builtin_element.pane_name)),
            (
                "image_filename",
                diff_string(manifest_element["image_filename"], builtin_element.image),
            ),
            (
                "material_name",
                diff_string(manifest_element["material_name"], builtin_element.material_name),
            ),
            ("base_x", diff_float(manifest_element["base_x"], builtin_element.base_x)),
            ("base_y", diff_float(manifest_element["base_y"], builtin_element.base_y)),
            ("width", diff_float(manifest_element["width"], builtin_element.width)),
            ("height", diff_float(manifest_element["height"], builtin_element.height)),
            (
                "released_alpha",
                diff_int(manifest_element["released_alpha"], builtin_element.released_alpha),
            ),
            (
                "pressed_alpha",
                diff_int(manifest_element["pressed_alpha"], builtin_element.pressed_alpha),
            ),
            (
                "released_scale",
                diff_float(manifest_element["released_scale"], builtin_element.released_scale),
            ),
            (
                "pressed_scale",
                diff_float(manifest_element["pressed_scale"], builtin_element.pressed_scale),
            ),
            (
                "stick_movement.x",
                diff_optional_float(manifest_movement_x, builtin_element.movement_x),
            ),
            (
                "stick_movement.y",
                diff_optional_float(manifest_movement_y, builtin_element.movement_y),
            ),
        ]
        for field, diff in checks:
            if diff is None:
                continue
            parsed_text, expected_text = diff
            errors.append(
                f"{control_id}.{field}: manifest {parsed_text} != built-in {expected_text}"
            )

    return errors


def build_report(
    skin_xml: Path,
    section: str,
    metadata: SkinMetadata,
    controls: list[ParsedControl],
    control_ids: list[str],
    builtin: list[BuiltInElement],
    skin_rs: Path,
    manifest: dict,
    manifest_dir: Path,
    manifest_validation_errors: list[str],
) -> str:
    mapped = [control for control in controls if control.control_id]
    unmapped = [control for control in controls if not control.control_id]
    parsed_by_control = {control.control_id: control for control in mapped}
    builtin_by_control = {element.control_id: element for element in builtin}
    parsed_controls = set(parsed_by_control)
    builtin_controls = set(builtin_by_control)
    model_controls = set(control_ids)
    missing_from_builtin = sorted_controls(parsed_controls - builtin_controls)
    missing_from_parsed = sorted_controls(builtin_controls - parsed_controls)
    model_missing_from_parsed = sorted_controls(model_controls - parsed_controls)
    model_missing_from_builtin = sorted_controls(model_controls - builtin_controls)
    field_mismatches = compare_fields(parsed_by_control, builtin_by_control)
    runtime_neutral = sorted_controls(parsed_controls & RUNTIME_NEUTRAL_CONTROLS)

    lines: list[str] = []
    lines.append("# RetroSpy Skin Analysis: switch-pro-alt")
    lines.append("")
    lines.append("Generated by `tools/analyze_retrospy_skin.py`.")
    lines.append("")
    lines.append("## Source")
    lines.append("")
    lines.append(f"- Skin XML: `{skin_xml}`")
    lines.append(f"- Selected section: `{section}`")
    lines.append(f"- Skin name: `{metadata.name}`")
    lines.append(f"- Author: `{metadata.author}`")
    lines.append(f"- Declared types: `{';'.join(metadata.types)}`")
    lines.append(
        f"- Background: `{metadata.background_image}` "
        f"({metadata.background_width}x{metadata.background_height})"
    )
    lines.append(f"- Built-in target parsed from: `{skin_rs}`")
    lines.append(f"- Dry-run manifest JSON: `{manifest_dir / 'skin_manifest.json'}`")
    lines.append(f"- Dry-run manifest Markdown: `{manifest_dir / 'skin_manifest.md'}`")
    lines.append("")

    lines.append("## Parsed Controls")
    lines.append("")
    lines.append(
        "| Kind | Section | RetroSpy name | ControlId | Image | Source x/y | Size | Smash base x/y | Movement |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for control in controls:
        movement = movement_text(control.movement_x, control.movement_y)
        lines.append(
            "| "
            + " | ".join(
                [
                    control.kind,
                    control.section,
                    code(control.retrospy_name),
                    code(control.control_id or "UNMAPPED"),
                    code(control.image),
                    f"{fmt(control.x)}, {fmt(control.y)}",
                    f"{fmt(control.width)} x {fmt(control.height)}",
                    f"{fmt(control.base_x)}, {fmt(control.base_y)}",
                    movement,
                ]
            )
            + " |"
        )
    lines.append("")

    lines.append("## Unsupported Or Unmapped")
    lines.append("")
    if unmapped:
        lines.append("| Kind | Section | RetroSpy name | Image | Reason |")
        lines.append("| --- | --- | --- | --- | --- |")
        for control in unmapped:
            lines.append(
                "| "
                + " | ".join(
                    [
                        control.kind,
                        control.section,
                        code(control.retrospy_name),
                        code(control.image),
                        "No mapping to current `ControlId` model",
                    ]
                )
                + " |"
            )
    else:
        lines.append("- None for the selected section.")
    if runtime_neutral:
        lines.append(
            "- Runtime-neutral mapped controls: "
            + ", ".join(code(control) for control in runtime_neutral)
            + ". These exist in the model/skin data but current match input leaves them unpressed."
        )
    lines.append("")

    lines.append("## Model And Built-In Skin Comparison")
    lines.append("")
    lines.append(f"- Parsed mapped controls: {len(mapped)}")
    lines.append(f"- `switch_pro_alt_builtin` elements: {len(builtin)}")
    lines.append(f"- Current `ControlId` variants: {len(control_ids)}")
    lines.append(
        "- Parsed controls missing from built-in: "
        + comma_or_none(missing_from_builtin)
    )
    lines.append(
        "- Built-in controls missing from parsed skin: "
        + comma_or_none(missing_from_parsed)
    )
    lines.append(
        "- ControlId variants not represented by parsed skin: "
        + comma_or_none(model_missing_from_parsed)
    )
    lines.append(
        "- ControlId variants not represented by built-in target: "
        + comma_or_none(model_missing_from_builtin)
    )
    lines.append("")
    if field_mismatches:
        lines.append("### Field Mismatches")
        lines.append("")
        lines.append("| ControlId | Field | Parsed | Built-in |")
        lines.append("| --- | --- | --- | --- |")
        for control_id, field, parsed, expected in field_mismatches:
            lines.append(
                f"| {code(control_id)} | {field} | {parsed} | {expected} |"
            )
        lines.append("")
    else:
        lines.append(
            "No field mismatches: parsed image names, centered coordinates, sizes, and stick ranges match `switch_pro_alt_builtin`."
        )
        lines.append("")
    if manifest_validation_errors:
        lines.append("Generated manifest validation errors:")
        lines.append("")
        for error in manifest_validation_errors:
            lines.append(f"- {error}")
    else:
        lines.append(
            "Generated dry-run manifest exactly matches `switch_pro_alt_builtin`."
        )
    lines.append("")

    lines.append("## Smash Layout Generation Plan")
    lines.append("")
    lines.append(
        "Future converter output should generate layout panes/assets from these rows, then emit a matching plugin skin manifest. This report does not copy PNGs or write any Nintendo layout assets."
    )
    lines.append("")
    lines.append("- Root pane expected by runtime: `sgpo_root`")
    lines.append(f"- Generated manifest element count: {len(manifest['elements'])}")
    lines.append(
        "- Coordinate transform used here: `base_x = x + width / 2 - background_width / 2`; `base_y = background_height / 2 - (y + height / 2)`"
    )
    lines.append("- Suggested pane names use existing `switch_pro_alt_builtin` names when present.")
    lines.append("")
    lines.append(
        "| ControlId | Suggested pane | Image asset | Material | Base x/y | Size | Stick movement |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- |")
    for control in sorted(mapped, key=lambda item: control_sort_key(item.control_id or "")):
        builtin_element = builtin_by_control.get(control.control_id or "")
        pane = builtin_element.pane_name if builtin_element else suggested_pane_name(control)
        material = f"mat_{pane}"
        lines.append(
            "| "
            + " | ".join(
                [
                    code(control.control_id or "UNMAPPED"),
                    code(pane),
                    code(control.image),
                    code(material),
                    f"{fmt(control.base_x)}, {fmt(control.base_y)}",
                    f"{fmt(control.width)} x {fmt(control.height)}",
                    movement_text(control.movement_x, control.movement_y),
                ]
            )
            + " |"
        )
    lines.append("")

    lines.append("## Unique Image Assets Referenced")
    lines.append("")
    for image in sorted({control.image for control in mapped if control.image}):
        lines.append(f"- `{image}`")
    lines.append("")

    lines.append("## Notes For The Future Converter")
    lines.append("")
    lines.append("- Input: RetroSpy `skin.xml` plus PNG assets.")
    lines.append("- Output: generated Smash `layout.arc` replacement assets/panes plus generated skin data matching `SkinElement` semantics.")
    lines.append("- The runtime plugin should continue to update existing named panes by `ControlId`; it should not parse arbitrary PNGs at runtime.")
    lines.append("- Do not commit or release Nintendo assets, extracted layout files, modified `layout.arc`, or copied RetroSpy PNG assets unless their license and redistribution path are handled separately.")
    lines.append("")
    return "\n".join(lines)


def compare_fields(
    parsed_by_control: dict[str | None, ParsedControl],
    builtin_by_control: dict[str, BuiltInElement],
) -> list[tuple[str, str, str, str]]:
    mismatches: list[tuple[str, str, str, str]] = []
    for control_id, parsed in sorted(
        parsed_by_control.items(), key=lambda item: control_sort_key(item[0] or "")
    ):
        if control_id is None or control_id not in builtin_by_control:
            continue
        expected = builtin_by_control[control_id]
        checks = [
            ("image", diff_string(parsed.image, expected.image)),
            ("base_x", diff_float(parsed.base_x, expected.base_x)),
            ("base_y", diff_float(parsed.base_y, expected.base_y)),
            ("width", diff_float(parsed.width, expected.width)),
            ("height", diff_float(parsed.height, expected.height)),
            ("movement_x", diff_optional_float(parsed.movement_x, expected.movement_x)),
            ("movement_y", diff_optional_float(parsed.movement_y, expected.movement_y)),
        ]
        for field, diff in checks:
            if diff is None:
                continue
            parsed_text, expected_text = diff
            mismatches.append((control_id, field, code(parsed_text), code(expected_text)))
    return mismatches


def diff_string(parsed: str, expected: str) -> tuple[str, str] | None:
    if parsed == expected:
        return None
    return parsed, expected


def diff_int(parsed: int, expected: int) -> tuple[str, str] | None:
    if parsed == expected:
        return None
    return str(parsed), str(expected)


def diff_float(parsed: float, expected: float) -> tuple[str, str] | None:
    if abs(parsed - expected) <= 0.01:
        return None
    return fmt(parsed), fmt(expected)


def diff_optional_float(
    parsed: float | None, expected: float | None
) -> tuple[str, str] | None:
    if parsed is None and expected is None:
        return None
    if parsed is not None and expected is not None and abs(parsed - expected) <= 0.01:
        return None
    return (
        "none" if parsed is None else fmt(parsed),
        "none" if expected is None else fmt(expected),
    )


def sorted_controls(values: Iterable[str]) -> list[str]:
    return sorted(values, key=control_sort_key)


def control_sort_key(control_id: str) -> tuple[int, str]:
    try:
        return CONTROL_ORDER.index(control_id), control_id
    except ValueError:
        return len(CONTROL_ORDER), control_id


def suggested_pane_name(control: ParsedControl) -> str:
    suffix = re.sub(r"[^a-z0-9]+", "_", (control.control_id or control.retrospy_name).lower())
    return f"sgpo_alt_{suffix}"


def material_name(pane_name: str) -> str:
    return f"mat_{pane_name}"


def movement_text(movement_x: float | None, movement_y: float | None) -> str:
    if movement_x is None or movement_y is None:
        return "-"
    return f"{fmt(movement_x)}, {fmt(movement_y)}"


def comma_or_none(values: list[str]) -> str:
    if not values:
        return "none"
    return ", ".join(code(value) for value in values)


def code(value: str) -> str:
    return f"`{value}`"


def fmt(value: float) -> str:
    text = f"{value:.2f}"
    return text.rstrip("0").rstrip(".")


if __name__ == "__main__":
    raise SystemExit(main())
