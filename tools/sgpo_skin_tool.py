#!/usr/bin/env python3
"""Build and install SGPO skin layouts from user-owned RetroSpy skins.

This is a PC-side convenience wrapper. It does not bundle Nintendo layout
assets or RetroSpy PNGs into the repo or the NRO. It reads local inputs,
generates repo-ignored manifests/layout output, backs up installed files, and
stages the NRO plus ARCropolis layout replacement to a configured SD root.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from stage_arcropolis_layout import load_dotenv


DEFAULT_BASE_UNPACKED = Path("local-assets/modified/info_melee/unpacked")
DEFAULT_OUTPUT_DIR = Path("local-assets/generated/sgpo-skins")
DEFAULT_NRO = Path("target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro")
DEFAULT_SWITCH_PRO_ALT_DIR = Path("/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt")
DEFAULT_GAMECUBE_TRON_DIR = Path("/mnt/c/Program Files/RetroSpy/skins/gamecube-tron")
ARCPOLIS_LAYOUT_PATH = Path("ui/layout/info/info_melee/info_melee/layout.arc")
MOD_FOLDER_NAME = "smash-gamepad-overlay"


@dataclass(frozen=True)
class SkinPreset:
    key: str
    skin_name: str
    section: str
    pane_prefix: str
    expected_layout_flavor: str
    skin_dir: Path
    manifest_dir: Path
    report: Path
    validate_builtin: bool


def presets(args: argparse.Namespace) -> list[SkinPreset]:
    return [
        SkinPreset(
            key="switch-pro-alt",
            skin_name="switch_pro_alt_builtin",
            section="switch",
            pane_prefix="sgpo_alt",
            expected_layout_flavor=(
                "switch_pro_alt_builtin generated asset panes from the future skin converter"
            ),
            skin_dir=args.switch_pro_alt_dir,
            manifest_dir=Path("target/skin-build/switch-pro-alt"),
            report=Path("target/skin-analysis/switch-pro-alt.md"),
            validate_builtin=True,
        ),
        SkinPreset(
            key="gamecube-tron",
            skin_name="gamecube_tron_builtin",
            section="gamecube",
            pane_prefix="sgpo_gct",
            expected_layout_flavor=(
                "gamecube_tron_builtin generated asset panes from the future skin converter"
            ),
            skin_dir=args.gamecube_tron_dir,
            manifest_dir=Path("target/skin-build/gamecube-tron"),
            report=Path("target/skin-analysis/gamecube-tron.md"),
            validate_builtin=False,
        ),
    ]


def main() -> int:
    args = parse_args()
    load_dotenv(args.env_file)

    sd_root = resolve_sd_root(args.sd_root)
    toolbox_cli = args.toolbox_cli or os.environ.get("TOOLBOX_CLI")
    selected_presets = [
        preset for preset in presets(args) if preset.key in set(args.include_skin)
    ]
    if not selected_presets:
        raise SystemExit("no skins selected")

    require_path(args.base_unpacked, "base unpacked info_melee layout directory")
    require_path(args.nro, "built SGPO NRO")
    for preset in selected_presets:
        require_path(preset.skin_dir / "skin.xml", f"{preset.key} RetroSpy skin.xml")

    for preset in selected_presets:
        run_analyzer(preset)

    backup_installed_files(sd_root, args.nro)
    run_builder(args, sd_root, toolbox_cli, selected_presets)
    stage_nro(sd_root, args.nro)
    print(f"installed active skin '{args.active_skin}' to {sd_root}")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate and install SGPO layout/NRO from local RetroSpy skins."
    )
    parser.add_argument("--env-file", type=Path, default=Path(".env"))
    parser.add_argument("--sd-root", type=Path, help="Switch/emulator SD root")
    parser.add_argument("--base-unpacked", type=Path, default=DEFAULT_BASE_UNPACKED)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--nro", type=Path, default=DEFAULT_NRO)
    parser.add_argument("--toolbox-cli", help="path to toolbox-cli binary")
    parser.add_argument("--active-skin", default="auto")
    parser.add_argument(
        "--include-skin",
        action="append",
        choices=["switch-pro-alt", "gamecube-tron"],
        default=None,
        help="skin preset to include; may be repeated",
    )
    parser.add_argument(
        "--switch-pro-alt-dir",
        type=Path,
        default=DEFAULT_SWITCH_PRO_ALT_DIR,
    )
    parser.add_argument(
        "--gamecube-tron-dir",
        type=Path,
        default=DEFAULT_GAMECUBE_TRON_DIR,
    )
    parser.add_argument("--quality", default="fast", choices=["ultra-fast", "fast", "basic", "slow"])
    parser.add_argument("--align", help="BNTX texture alignment, e.g. 0x200 or 0x1000")
    parser.add_argument("--force", action="store_true", help="replace local generated output")
    args = parser.parse_args()
    if args.include_skin is None:
        args.include_skin = ["switch-pro-alt", "gamecube-tron"]
    return args


def resolve_sd_root(arg: Path | None) -> Path:
    if arg is not None:
        return arg
    mods_dir = os.environ.get("SGPO_EMU_MODS_DIR")
    if mods_dir:
        return Path(mods_dir).parent.parent
    raise SystemExit("--sd-root is required unless SGPO_EMU_MODS_DIR is set in .env")


def run_analyzer(preset: SkinPreset) -> None:
    command = [
        sys.executable,
        "tools/analyze_retrospy_skin.py",
        "--skin-dir",
        str(preset.skin_dir),
        "--section",
        preset.section,
        "--skin-name",
        preset.skin_name,
        "--pane-prefix",
        preset.pane_prefix,
        "--expected-layout-flavor",
        preset.expected_layout_flavor,
        "--out",
        str(preset.report),
        "--manifest-dir",
        str(preset.manifest_dir),
    ]
    if not preset.validate_builtin:
        command.append("--skip-built-in-validation")
    run(command)


def run_builder(
    args: argparse.Namespace,
    sd_root: Path,
    toolbox_cli: str | None,
    selected_presets: list[SkinPreset],
) -> None:
    command = [
        sys.executable,
        "tools/build_skin_layout.py",
        "--base-unpacked",
        str(args.base_unpacked),
        "--output-dir",
        str(args.output_dir),
        "--sd-root",
        str(sd_root),
        "--write-config",
        "--active-skin",
        args.active_skin,
        "--quality",
        args.quality,
    ]
    if args.force:
        command.append("--force")
    if toolbox_cli:
        command.extend(["--toolbox-cli", toolbox_cli])
    if args.align:
        command.extend(["--align", args.align])
    for preset in selected_presets:
        command.extend(
            [
                "--skin",
                f"{preset.manifest_dir / 'skin_manifest.json'}::{preset.skin_dir}",
            ]
        )
    run(command)


def stage_nro(sd_root: Path, nro: Path) -> None:
    plugin_dir = (
        sd_root
        / "atmosphere"
        / "contents"
        / "01006A800016E000"
        / "romfs"
        / "skyline"
        / "plugins"
    )
    plugin_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(nro, plugin_dir / nro.name)
    print(f"staged NRO -> {plugin_dir / nro.name}")


def backup_installed_files(sd_root: Path, nro: Path) -> None:
    timestamp = time.strftime("%Y%m%d-%H%M%S")
    paths = [
        (
            sd_root
            / "ultimate"
            / "mods"
            / MOD_FOLDER_NAME
            / ARCPOLIS_LAYOUT_PATH
        ),
        sd_root / "ultimate" / "mods" / MOD_FOLDER_NAME / "config.json",
        (
            sd_root
            / "atmosphere"
            / "contents"
            / "01006A800016E000"
            / "romfs"
            / "skyline"
            / "plugins"
            / nro.name
        ),
    ]
    for path in paths:
        if not path.exists():
            continue
        backup = path.with_name(f"{path.name}.bak.{timestamp}")
        shutil.copy2(path, backup)
        print(f"backed up {path} -> {backup}")


def require_path(path: Path, label: str) -> None:
    if not path.exists():
        raise SystemExit(f"{label} does not exist: {path}")


def run(command: list[str]) -> None:
    print("+ " + " ".join(quote_arg(part) for part in command))
    subprocess.run(command, check=True)


def quote_arg(value: str | Path) -> str:
    text = str(value)
    if not text or any(ch.isspace() for ch in text):
        return repr(text)
    return text


if __name__ == "__main__":
    raise SystemExit(main())
