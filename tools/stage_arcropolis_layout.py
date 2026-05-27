#!/usr/bin/env python3
"""Stage the generated SGPO layout as an ARCropolis data-replacement mod."""

from __future__ import annotations

import argparse
import os
import shutil
from pathlib import Path

from patch_info_melee_layout import PANE_SPECS, ROOT_PANE_NAME


DEFAULT_LAYOUT = Path("local-assets/modified/info_melee/layout.arc")
DEFAULT_DEST = Path("target/arcropolis/smash-gamepad-overlay")
DEFAULT_NRO = Path("target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro")
ARCPOLIS_LAYOUT_PATH = Path("ui/layout/info/info_melee/info_melee/layout.arc")
MOD_FOLDER_NAME = "smash-gamepad-overlay"
TRUE_VALUES = {"1", "true", "yes", "on"}
REQUIRED_MARKERS = [
    ROOT_PANE_NAME.encode("ascii"),
    *(name.encode("ascii") for name, _pos, _size in PANE_SPECS),
]


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


def env_flag(name: str) -> bool:
    return os.environ.get(name, "").strip().lower() in TRUE_VALUES


def validate_layout(path: Path) -> None:
    data = path.read_bytes()
    missing = [marker.decode("ascii") for marker in REQUIRED_MARKERS if marker not in data]
    if missing:
        missing_text = ", ".join(missing)
        raise SystemExit(f"{path} is missing expected SGPO pane marker(s): {missing_text}")


def copy_layout(layout: Path, mod_root: Path) -> Path:
    staged_layout = mod_root / ARCPOLIS_LAYOUT_PATH
    staged_layout.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(layout, staged_layout)
    return staged_layout


def copy_nro(nro: Path, plugin_dir: Path) -> Path:
    if not nro.exists():
        raise SystemExit(f"NRO file does not exist: {nro}")

    plugin_dir.mkdir(parents=True, exist_ok=True)
    dest = plugin_dir / nro.name
    shutil.copy2(nro, dest)
    return dest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--layout",
        type=Path,
        default=DEFAULT_LAYOUT,
        help="generated SGPO info_melee layout.arc",
    )
    parser.add_argument(
        "--dest",
        type=Path,
        default=DEFAULT_DEST,
        help="ARCropolis mod folder to create or update",
    )
    parser.add_argument(
        "--nro",
        type=Path,
        default=DEFAULT_NRO,
        help="release NRO to copy when emulator deployment is enabled",
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=Path(".env"),
        help="optional local env file with SGPO_EMU_* deployment paths",
    )
    parser.add_argument(
        "--emu",
        action="store_true",
        help="copy layout and NRO to emulator paths configured by .env/environment",
    )
    parser.add_argument(
        "--no-emu",
        action="store_true",
        help="skip emulator deployment even if SGPO_DEPLOY_EMU=1",
    )
    parser.add_argument(
        "--skip-nro",
        action="store_true",
        help="copy only the ARCropolis layout when emulator deployment is enabled",
    )
    args = parser.parse_args()

    load_dotenv(args.env_file)

    if not args.layout.exists():
        raise SystemExit(f"layout file does not exist: {args.layout}")

    validate_layout(args.layout)

    staged_layout = copy_layout(args.layout, args.dest)

    print(f"staged {args.layout} -> {staged_layout}")
    print(f"copy this folder to SD/emulator ARCropolis mods: {args.dest}")
    print("expected game path inside the mod:")
    print(f"  {ARCPOLIS_LAYOUT_PATH}")

    emu_enabled = (args.emu or env_flag("SGPO_DEPLOY_EMU")) and not args.no_emu
    if not emu_enabled:
        return

    emu_mods_dir = os.environ.get("SGPO_EMU_MODS_DIR")
    emu_plugin_dir = os.environ.get("SGPO_EMU_PLUGIN_DIR")
    if not emu_mods_dir:
        raise SystemExit("SGPO_EMU_MODS_DIR must be set when emulator deployment is enabled")

    emu_mod_root = Path(emu_mods_dir) / MOD_FOLDER_NAME
    emu_layout = copy_layout(args.layout, emu_mod_root)
    print(f"staged emulator layout -> {emu_layout}")

    if args.skip_nro:
        return

    if not emu_plugin_dir:
        raise SystemExit("SGPO_EMU_PLUGIN_DIR must be set when emulator deployment is enabled")

    emu_nro = copy_nro(args.nro, Path(emu_plugin_dir))
    print(f"staged emulator NRO -> {emu_nro}")


if __name__ == "__main__":
    main()
