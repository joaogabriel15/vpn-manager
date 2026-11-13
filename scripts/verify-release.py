#!/usr/bin/env python3
"""Reject missing, extra or corrupted desktop release assets before publication."""
import hashlib
from pathlib import Path
import sys
import tomllib


def verify(folder, version):
    names = {f"vpn-manager-{version}-{target}{ext}" for target, ext in [
        ("linux-x86_64", ".tar.gz"), ("windows-x86_64", ".zip"),
        ("macos-arm64", ".zip"), ("macos-x86_64", ".zip"),
    ]}
    expected = names | {name + ".sha256" for name in names}
    if {path.name for path in folder.iterdir()} != expected:
        raise ValueError("Release assets do not match the complete desktop matrix")
    for name in names:
        with (folder / name).open("rb") as file:
            digest = hashlib.file_digest(file, "sha256").hexdigest()
        if (folder / (name + ".sha256")).read_text().strip() != f"{digest}  {name}":
            raise ValueError("Invalid checksum: " + name)


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    verify(Path(sys.argv[1]), version)
