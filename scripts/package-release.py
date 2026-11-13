#!/usr/bin/env python3
"""Build desktop release archives from native binaries; never download engines."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-gnu": "linux-x86_64",
    "x86_64-pc-windows-msvc": "windows-x86_64",
    "aarch64-apple-darwin": "macos-arm64",
    "x86_64-apple-darwin": "macos-x86_64",
}


def version_from_manifest():
    return tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]


def copy_executable(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(0o755)


def notices(destination):
    """Include actual license texts as well as SPDX metadata for dependencies."""
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT
    ))
    folder = destination / "licenses"
    shutil.copytree(ROOT / "packaging/licenses", folder)
    entries = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["source"] is None:
            continue
        label = f'{package["name"]}-{package["version"]}'
        entries.append(f'{label}: {package["license"] or "SEE LICENSE FILE"}\n{package.get("repository") or ""}\n')
        source = Path(package["manifest_path"]).parent
        files = [p for p in source.rglob("*") if p.is_file() and (
            p.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "AUTHORS"))
            or "LICENSE" in p.name.upper()
            or p.name in {"OFL.txt", "UFL.txt", "Hack-Regular.txt"}
        )]
        if not files and (source / "README.md").is_file():
            files.append(source / "README.md")
        if package.get("license_file"):
            files.append(source / package["license_file"])
        for path in set(files):
            if path.is_file():
                relative = path.relative_to(source) if path.is_relative_to(source) else Path(path.name)
                target = folder / label / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
    (destination / "THIRD-PARTY.txt").write_text("Dependency inventory (includes target-specific and build dependencies).\n\n" + "\n".join(entries), encoding="utf-8")


def build_archive(target, version, binaries, output, include_notices=True):
    if target not in TARGETS:
        raise ValueError("Unsupported release target")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("Invalid release version")
    name = f"vpn-manager-{version}-{TARGETS[target]}"
    windows = "windows" in target
    macos = "apple" in target
    extension = ".zip" if windows or macos else ".tar.gz"
    archive = output / (name + extension)
    if archive.exists():
        raise FileExistsError(archive)
    suffix = ".exe" if windows else ""
    cli = "vpn-manager-native-cli" if windows or macos else "vpn-manager-cli"
    for binary in ["vpn-manager", cli]:
        if not (binaries / (binary + suffix)).is_file():
            raise FileNotFoundError(binaries / (binary + suffix))
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="vpn-manager-package-") as temp:
        staging = Path(temp) / name
        staging.mkdir()
        (staging / "README.md").write_text((ROOT / "docs/DESKTOP-RELEASES.md").read_text(), encoding="utf-8")
        if include_notices:
            notices(staging)
        if macos:
            app = staging / "VPN Manager.app"
            executable = app / "Contents/MacOS/vpn-manager"
            copy_executable(binaries / "vpn-manager", executable)
            copy_executable(binaries / cli, executable.with_name("vpn-manager-cli"))
            resources = app / "Contents/Resources"
            resources.mkdir(parents=True)
            # Move notices into the app so they remain available after installation.
            for item in ["licenses", "THIRD-PARTY.txt"]:
                if (staging / item).exists():
                    shutil.move(staging / item, resources / item)
            iconset = Path(temp) / "VpnManager.iconset"
            iconset.mkdir()
            for size in [16, 32, 128, 256, 512]:
                subprocess.run(["sips", "-z", str(size), str(size), str(ROOT / "packaging/icons/vpn-manager-256.png"), "--out", str(iconset / f"icon_{size}x{size}.png")], check=True, stdout=subprocess.DEVNULL)
            subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "VpnManager.icns")], check=True)
            with (app / "Contents/Info.plist").open("wb") as file:
                plistlib.dump({
                    "CFBundleIdentifier": "br.com.pacheco.VpnManager",
                    "CFBundleName": "VPN Manager", "CFBundleDisplayName": "VPN Manager",
                    "CFBundleExecutable": "vpn-manager", "CFBundlePackageType": "APPL",
                    "CFBundleShortVersionString": version.split("-")[0],
                    "CFBundleVersion": version.split("-")[0],
                    "CFBundleIconFile": "VpnManager.icns", "LSMinimumSystemVersion": "13.0",
                    "NSHighResolutionCapable": True,
                }, file)
            subprocess.run(["codesign", "--force", "--sign", "-", str(executable.with_name("vpn-manager-cli"))], check=True)
            subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
        elif windows:
            copy_executable(binaries / "vpn-manager.exe", staging / "vpn-manager.exe")
            copy_executable(binaries / (cli + suffix), staging / "vpn-manager-cli.exe")
        else:
            shutil.copy2(ROOT / "docs/LINUX-VPN-SUPPORT.md", staging / "LINUX-VPN-SUPPORT.md")
            copy_executable(binaries / "vpn-manager", staging / "bin/vpn-manager")
            copy_executable(binaries / cli, staging / "bin/vpn-manager-cli")
            copy_executable(ROOT / "scripts/install-release.sh", staging / "install.sh")
            shutil.copytree(ROOT / "packaging/icons", staging / "icons")
            shutil.copy2(ROOT / "packaging/vpn-manager.svg", staging / "vpn-manager.svg")
        if extension == ".zip":
            with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as file:
                for path in sorted(staging.rglob("*")):
                    if path.is_file():
                        file.write(path, path.relative_to(staging.parent))
        else:
            with tarfile.open(archive, "x:gz") as file:
                file.add(staging, arcname=name)
    with archive.open("rb") as file:
        digest = hashlib.file_digest(file, "sha256").hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--version", default=version_from_manifest())
    parser.add_argument("--binary-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if args.version != version_from_manifest():
        parser.error("Release tag/version must match Cargo.toml")
    print(build_archive(args.target, args.version, args.binary_dir, args.output))


if __name__ == "__main__":
    main()
