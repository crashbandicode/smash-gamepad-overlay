#!/usr/bin/env python3
"""Compatibility wrapper for the Rust SGPO skin-pack installer."""

from __future__ import annotations

import subprocess
import sys


def main() -> int:
    command = [
        "cargo",
        "run",
        "--manifest-path",
        "tools/sgpo_installer/Cargo.toml",
        "--",
        *sys.argv[1:],
    ]
    return subprocess.call(command)


if __name__ == "__main__":
    raise SystemExit(main())
