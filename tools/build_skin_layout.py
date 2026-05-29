#!/usr/bin/env python3
"""Build and optionally stage a PNG-backed SGPO skin layout with toolbox-cli."""

from __future__ import annotations

import argparse
import binascii
import json
import os
import shutil
import subprocess
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path


DEFAULT_BASE_UNPACKED = Path("local-assets/modified/info_melee/unpacked")
DEFAULT_OUTPUT_DIR = Path("local-assets/generated/switch-pro-alt")
DEFAULT_MANIFEST = Path("target/skin-build/switch-pro-alt/skin_manifest.json")
DEFAULT_SKIN_DIR = Path("/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt")
DEFAULT_TOOLBOX_CLI = Path("/tmp/toolbox-cli-review/target/release/toolbox-cli")
TOOLBOX_CLI_CANDIDATES = (
    DEFAULT_TOOLBOX_CLI,
    Path("/home/intpa/Toolbox-Cli/target/release/toolbox-cli"),
    Path("/home/intpa/Toolbox-Cli/target/debug/toolbox-cli"),
)
ARCPOLIS_LAYOUT_PATH = Path("ui/layout/info/info_melee/info_melee/layout.arc")
MOD_FOLDER_NAME = "smash-gamepad-overlay"
ACTIVE_SKIN_NAME = "switch_pro_alt_builtin"
ROOT_BFLYT = Path("blyt/info_melee.bflyt")
PLAYER_PARTS_BFLYTS = (
    Path("blyt/info_melee_lct_player_00.bflyt"),
    Path("blyt/info_melee_lct_player_01.bflyt"),
)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


@dataclass(frozen=True)
class SkinSpec:
    manifest: Path
    skin_dir: Path


def load_dotenv(path: Path) -> None:
    if not path.exists():
        return

    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue

        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key:
            os.environ.setdefault(key, value)


def resolve_toolbox_cli(arg: str | None) -> str:
    if arg:
        return arg
    if os.environ.get("TOOLBOX_CLI"):
        return os.environ["TOOLBOX_CLI"]
    for candidate in TOOLBOX_CLI_CANDIDATES:
        if candidate.exists():
            return str(candidate)
    return "toolbox-cli"


def run(command: list[str]) -> None:
    print("+ " + " ".join(quote_arg(part) for part in command))
    subprocess.run(command, check=True)


def quote_arg(value: str) -> str:
    if not value or any(ch.isspace() for ch in value):
        return repr(value)
    return value


def require_path(path: Path, label: str) -> None:
    if not path.exists():
        raise SystemExit(f"{label} does not exist: {path}")


def copy_layout(layout: Path, mod_root: Path) -> Path:
    staged_layout = mod_root / ARCPOLIS_LAYOUT_PATH
    staged_layout.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(layout, staged_layout)
    return staged_layout


def prepare_skin_assets(
    source_skin_dir: Path,
    output_dir: Path,
    skin_name: str,
    elements: list[dict],
) -> Path:
    prepared_skin_dir = output_dir / "skin-assets" / skin_name
    if prepared_skin_dir.exists():
        make_tree_writable(prepared_skin_dir)
        shutil.rmtree(prepared_skin_dir)
    shutil.copytree(source_skin_dir, prepared_skin_dir)
    make_tree_writable(prepared_skin_dir)

    for element in elements:
        if element["control_id"] == "SkinBackground" and skin_name == "switch_pro_alt_builtin":
            chroma_key_green_to_alpha(prepared_skin_dir / element["image_filename"])

    return prepared_skin_dir


def make_tree_writable(path: Path) -> None:
    for item in path.rglob("*"):
        item.chmod(item.stat().st_mode | 0o200)
    path.chmod(path.stat().st_mode | 0o700)


def chroma_key_green_to_alpha(path: Path) -> None:
    width, height, rows = read_rgba_png(path)
    changed = False

    for row in rows:
        for index in range(0, len(row), 4):
            red, green, blue, _alpha = row[index : index + 4]
            if is_chroma_green(red, green, blue):
                row[index + 3] = 0
                changed = True

    if changed:
        bleed_transparent_pixel_rgb(rows, width, height, passes=6)
        path.chmod(path.stat().st_mode | 0o200)
        write_rgba_png(path, width, height, rows)


def is_chroma_green(red: int, green: int, blue: int) -> bool:
    return green >= 110 and green - max(red, blue) >= 35


def bleed_transparent_pixel_rgb(
    rows: list[bytearray],
    width: int,
    height: int,
    passes: int,
) -> None:
    for _pass_index in range(passes):
        updates: list[tuple[int, int, tuple[int, int, int]]] = []
        for y in range(height):
            row = rows[y]
            for x in range(width):
                index = x * 4
                if row[index + 3] != 0:
                    continue
                colors = neighboring_opaque_colors(rows, width, height, x, y)
                if not colors:
                    continue
                red = sum(color[0] for color in colors) // len(colors)
                green = sum(color[1] for color in colors) // len(colors)
                blue = sum(color[2] for color in colors) // len(colors)
                updates.append((y, index, (red, green, blue)))

        if not updates:
            break

        for y, index, (red, green, blue) in updates:
            rows[y][index] = red
            rows[y][index + 1] = green
            rows[y][index + 2] = blue

    for row in rows:
        for index in range(0, len(row), 4):
            if row[index + 3] == 0 and is_chroma_green(
                row[index], row[index + 1], row[index + 2]
            ):
                row[index] = 0
                row[index + 1] = 0
                row[index + 2] = 0


def neighboring_opaque_colors(
    rows: list[bytearray],
    width: int,
    height: int,
    x: int,
    y: int,
) -> list[tuple[int, int, int]]:
    colors: list[tuple[int, int, int]] = []
    for neighbor_y in range(max(0, y - 1), min(height, y + 2)):
        for neighbor_x in range(max(0, x - 1), min(width, x + 2)):
            if neighbor_x == x and neighbor_y == y:
                continue
            index = neighbor_x * 4
            row = rows[neighbor_y]
            if row[index + 3] == 0:
                continue
            colors.append((row[index], row[index + 1], row[index + 2]))
    return colors


def read_rgba_png(path: Path) -> tuple[int, int, list[bytearray]]:
    data = path.read_bytes()
    if not data.startswith(PNG_SIGNATURE):
        raise SystemExit(f"{path} is not a PNG")

    offset = len(PNG_SIGNATURE)
    width = height = 0
    compressed = bytearray()
    bit_depth = color_type = interlace = None

    while offset < len(data):
        chunk_len = struct.unpack(">I", data[offset : offset + 4])[0]
        chunk_type = data[offset + 4 : offset + 8]
        chunk_data = data[offset + 8 : offset + 8 + chunk_len]
        offset += 12 + chunk_len

        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type, compression, filter_method, interlace = (
                struct.unpack(">IIBBBBB", chunk_data)
            )
            if compression != 0 or filter_method != 0:
                raise SystemExit(f"{path} uses unsupported PNG compression/filter method")
        elif chunk_type == b"IDAT":
            compressed.extend(chunk_data)
        elif chunk_type == b"IEND":
            break

    if bit_depth != 8 or color_type != 6 or interlace != 0:
        raise SystemExit(f"{path} must be 8-bit non-interlaced RGBA PNG")

    raw = zlib.decompress(bytes(compressed))
    stride = width * 4
    rows: list[bytearray] = []
    previous = bytearray(stride)
    offset = 0

    for _row_index in range(height):
        filter_type = raw[offset]
        offset += 1
        row = bytearray(raw[offset : offset + stride])
        offset += stride
        unfilter_png_row(row, previous, filter_type, 4)
        rows.append(row)
        previous = row

    return width, height, rows


def unfilter_png_row(row: bytearray, previous: bytearray, filter_type: int, bpp: int) -> None:
    if filter_type == 0:
        return
    if filter_type == 1:
        for index in range(len(row)):
            left = row[index - bpp] if index >= bpp else 0
            row[index] = (row[index] + left) & 0xFF
        return
    if filter_type == 2:
        for index in range(len(row)):
            row[index] = (row[index] + previous[index]) & 0xFF
        return
    if filter_type == 3:
        for index in range(len(row)):
            left = row[index - bpp] if index >= bpp else 0
            row[index] = (row[index] + ((left + previous[index]) // 2)) & 0xFF
        return
    if filter_type == 4:
        for index in range(len(row)):
            left = row[index - bpp] if index >= bpp else 0
            up = previous[index]
            up_left = previous[index - bpp] if index >= bpp else 0
            row[index] = (row[index] + paeth(left, up, up_left)) & 0xFF
        return

    raise SystemExit(f"unsupported PNG filter type {filter_type}")


def paeth(left: int, up: int, up_left: int) -> int:
    estimate = left + up - up_left
    left_distance = abs(estimate - left)
    up_distance = abs(estimate - up)
    up_left_distance = abs(estimate - up_left)
    if left_distance <= up_distance and left_distance <= up_left_distance:
        return left
    if up_distance <= up_left_distance:
        return up
    return up_left


def write_rgba_png(path: Path, width: int, height: int, rows: list[bytearray]) -> None:
    raw = bytearray()
    for row in rows:
        raw.append(0)
        raw.extend(row)

    output = bytearray(PNG_SIGNATURE)
    output.extend(png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
    output.extend(png_chunk(b"IDAT", zlib.compress(bytes(raw), level=9)))
    output.extend(png_chunk(b"IEND", b""))
    path.write_bytes(output)


def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    crc = binascii.crc32(chunk_type)
    crc = binascii.crc32(data, crc) & 0xFFFF_FFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def write_skin_config(mod_root: Path, active_skin: str) -> Path:
    config_path = mod_root / "config.json"
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(json.dumps({"active_skin": active_skin}, indent=2) + "\n")
    return config_path


def load_manifest(path: Path) -> dict:
    return json.loads(path.read_text())


def parse_skin_spec(value: str) -> SkinSpec:
    if "::" not in value:
        raise SystemExit(
            "--skin values must use MANIFEST::RETROSPY_SKIN_DIR, "
            f"got: {value}"
        )
    manifest, skin_dir = value.split("::", 1)
    return SkinSpec(Path(manifest), Path(skin_dir))


def default_skin_specs(args: argparse.Namespace) -> list[SkinSpec]:
    if args.skin:
        return [parse_skin_spec(value) for value in args.skin]
    return [SkinSpec(args.manifest, args.skin_dir)]


def texture_name_for_pane(pane_name: str) -> str:
    return f"tex_{pane_name}"


def hide_generated_panes(toolbox_cli: str, bflyt: Path, elements: list[dict]) -> None:
    for element in elements:
        run(
            [
                toolbox_cli,
                "pane-set",
                "--input",
                str(bflyt),
                "--pane",
                element["pane_name"],
                "--alpha",
                "0",
            ]
        )


def add_manifest_panes_to_bflyt(
    toolbox_cli: str,
    bflyt: Path,
    elements: list[dict],
    pane_template: str,
    material_template: str,
) -> None:
    for element in elements:
        pane_name = element["pane_name"]
        material_name = element["material_name"]
        texture_name = texture_name_for_pane(pane_name)

        run(
            [
                toolbox_cli,
                "bflyt-add-texture-ref",
                "--input",
                str(bflyt),
                "--name",
                texture_name,
            ]
        )
        run(
            [
                toolbox_cli,
                "bflyt-add-material",
                "--input",
                str(bflyt),
                "--template",
                material_template,
                "--name",
                material_name,
                "--bind-texture",
                texture_name,
            ]
        )
        run(
            [
                toolbox_cli,
                "pane-clone",
                "--input",
                str(bflyt),
                "--template",
                pane_template,
                "--name",
                pane_name,
                "--parent",
                "sgpo_root",
                "--translate-x",
                str(element["base_x"]),
                "--translate-y",
                str(element["base_y"]),
                "--width",
                str(element["width"]),
                "--height",
                str(element["height"]),
                "--alpha",
                "0",
                "--visible",
                "true",
                "--bind-material",
                material_name,
            ]
        )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate a PNG-backed SGPO layout.arc from one or more RetroSpy skins."
    )
    parser.add_argument("--base-unpacked", type=Path, default=DEFAULT_BASE_UNPACKED)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--skin-dir", type=Path, default=DEFAULT_SKIN_DIR)
    parser.add_argument(
        "--skin",
        action="append",
        help=(
            "Additional/explicit skin input as MANIFEST::RETROSPY_SKIN_DIR. "
            "When provided, --manifest/--skin-dir are ignored."
        ),
    )
    parser.add_argument("--toolbox-cli", help="path to toolbox-cli binary")
    parser.add_argument("--quality", default="fast", choices=["ultra-fast", "fast", "basic", "slow"])
    parser.add_argument("--align", default=None, help="BNTX texture alignment, e.g. 0x200 or 0x1000")
    parser.add_argument(
        "--no-srgb",
        action="store_true",
        help="Import PNGs as linear BC7 instead of BC7 sRGB.",
    )
    parser.add_argument(
        "--pane-template",
        default="sgpo_pro_a_marker",
        help="existing pic1 pane to clone for generated PNG panes",
    )
    parser.add_argument(
        "--material-template",
        default="set_rep_stock_01",
        help="existing material to clone for generated PNG materials",
    )
    parser.add_argument("--env-file", type=Path, default=Path(".env"))
    parser.add_argument("--sd-root", type=Path, help="SD root to stage layout/config into")
    parser.add_argument(
        "--stage-target",
        type=Path,
        default=Path("target/arcropolis/smash-gamepad-overlay-generated"),
        help="local ARCropolis mod folder to write",
    )
    parser.add_argument("--no-stage-target", action="store_true")
    parser.add_argument("--write-config", action="store_true")
    parser.add_argument("--active-skin", default=ACTIVE_SKIN_NAME)
    parser.add_argument("--force", action="store_true", help="replace existing output directory")
    args = parser.parse_args()

    load_dotenv(args.env_file)

    toolbox_cli = resolve_toolbox_cli(args.toolbox_cli)
    base_unpacked = args.base_unpacked
    output_dir = args.output_dir
    output_unpacked = output_dir / "unpacked"
    output_layout = output_dir / "layout.arc"

    require_path(base_unpacked / ROOT_BFLYT, "base root BFLYT")
    for bflyt in PLAYER_PARTS_BFLYTS:
        require_path(base_unpacked / bflyt, "base player-parts BFLYT")
    require_path(base_unpacked / "timg" / "__Combined.bntx", "base BNTX")
    skin_specs = default_skin_specs(args)
    for spec in skin_specs:
        require_path(spec.manifest, "skin manifest")
        require_path(spec.skin_dir / "skin.xml", "RetroSpy skin.xml")

    if output_unpacked.exists():
        if not args.force:
            raise SystemExit(f"output already exists: {output_unpacked} (pass --force to replace)")
        shutil.rmtree(output_unpacked)
    output_dir.mkdir(parents=True, exist_ok=True)
    shutil.copytree(base_unpacked, output_unpacked)

    for spec in skin_specs:
        manifest = load_manifest(spec.manifest)
        skin_name = manifest["skin_name"]
        elements = manifest["elements"]
        print(f"building skin '{skin_name}' from {spec.manifest}")
        prepared_skin_dir = prepare_skin_assets(
            spec.skin_dir,
            output_dir,
            skin_name,
            elements,
        )

        apply_command = [
            toolbox_cli,
            "layout-apply-manifest",
            "--layout-dir",
            str(output_unpacked),
            "--manifest",
            str(spec.manifest),
            "--skin-dir",
            str(prepared_skin_dir),
            "--bflyt",
            str(ROOT_BFLYT),
            "--pane-template",
            args.pane_template,
            "--material-template",
            args.material_template,
            "--quality",
            args.quality,
        ]
        if not args.no_srgb:
            apply_command.append("--srgb")
        if args.align:
            apply_command.extend(["--align", args.align])
        run(apply_command)
        hide_generated_panes(toolbox_cli, output_unpacked / ROOT_BFLYT, elements)

        for bflyt in PLAYER_PARTS_BFLYTS:
            add_manifest_panes_to_bflyt(
                toolbox_cli,
                output_unpacked / bflyt,
                elements,
                args.pane_template,
                args.material_template,
            )

        for bflyt in (ROOT_BFLYT, *PLAYER_PARTS_BFLYTS):
            run([
                toolbox_cli,
                "layout-validate-manifest",
                "--layout-dir",
                str(output_unpacked),
                "--manifest",
                str(spec.manifest),
                "--bflyt",
                str(bflyt),
            ])

    if output_layout.exists():
        output_layout.unlink()
    run([toolbox_cli, "sarc-pack", "--input", str(output_unpacked), "--out", str(output_layout)])

    if not args.no_stage_target:
        staged_layout = copy_layout(output_layout, args.stage_target)
        print(f"staged local ARCropolis layout -> {staged_layout}")

    if args.sd_root:
        sd_mod_root = args.sd_root / "ultimate" / "mods" / MOD_FOLDER_NAME
        sd_layout = copy_layout(output_layout, sd_mod_root)
        print(f"staged SD layout -> {sd_layout}")
        if args.write_config:
            config_path = write_skin_config(sd_mod_root, args.active_skin)
            print(f"wrote SD skin config -> {config_path}")

    print(f"built layout -> {output_layout}")


if __name__ == "__main__":
    main()
