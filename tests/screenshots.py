#!/usr/bin/env python3
"""Capture the real GTK UI using synthetic data and a read-only nmcli fixture."""
import argparse
from contextlib import contextmanager
import os
from pathlib import Path
import shutil
import select
import signal
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
IMAGES = {
    "vpn-manager.png": "main.png",
    "vpn-manager-editor.png": "settings.png",
    "vpn-manager-network.png": "network.png",
    "vpn-manager-advanced.png": "advanced.png",
    "vpn-manager-modal-narrow.png": "compact.png",
    "vpn-manager-login.png": "login.png",
}


def first_line(process):
    readable, _, _ = select.select([process.stdout], [], [], 5)
    if not readable:
        raise RuntimeError("Private fixture process did not start")
    line = process.stdout.readline().strip()
    if not line:
        raise RuntimeError("Private fixture process exited")
    return line


def run_demo(argv, env, timeout):
    # Keep fixture monitors in the same isolated process group as the app/probe.
    # This also cleans them up if a crash prevents the Rust Drop handlers running.
    process = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    try:
        out, err = process.communicate(timeout=timeout)
        return subprocess.CompletedProcess(argv, process.returncode, out, err)
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


@contextmanager
def demo_environment(stage):
    processes = []
    try:
        daemon = subprocess.Popen(["dbus-daemon", "--session", "--nofork", "--print-address=1"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        processes.append(daemon)
        address = first_line(daemon)
        service = subprocess.Popen(["python3", str(ROOT / "tests/fixtures/preview_bus.py"), address], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        processes.append(service)
        if first_line(service) != "FIXTURE_READY":
            raise RuntimeError("Private fixture service did not start")
        executable = stage / "nmcli"
        shutil.copyfile(ROOT / "tests/fixtures/preview_nmcli.py", executable)
        executable.chmod(0o700)
        yield {**os.environ, "PATH": str(stage) + os.pathsep + os.environ.get("PATH", ""), "DBUS_SYSTEM_BUS_ADDRESS": address, "VPN_MANAGER_TEST_FIXTURE": "1"}
    finally:
        for process in reversed(processes):
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/vpn-manager")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/screenshots")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="vpn-manager-screenshots-") as folder, demo_environment(Path(folder)) as base_env:
        stage = Path(folder)
        env = {**base_env,
               "VPN_MANAGER_PREVIEW": str(stage / "vpn-manager.png"),
               "GTK_THEME": "Adwaita:dark", "GDK_SCALE": "1", "GDK_DPI_SCALE": "1"}
        env.pop("VPN_MANAGER_IDLE_TEST", None)
        result = run_demo([str(args.binary.resolve()), "--self-test"], env, timeout=60)
        if result.returncode or "UI_SMOKE_OK profiles=3" not in result.stdout:
            raise SystemExit("Screenshot smoke test failed:\n" + result.stdout + result.stderr)
        for source in IMAGES:
            content = (stage / source).read_bytes()
            if content[:8] != b"\x89PNG\r\n\x1a\n" or min(struct.unpack(">II", content[16:24])) < 200:
                raise SystemExit(f"Invalid screenshot: {source}")
        args.output.mkdir(parents=True, exist_ok=True)
        for source, target in IMAGES.items():
            shutil.copyfile(stage / source, args.output / target)
            print(target)
        print("SCREENSHOTS_OK: synthetic profiles; read-only fixture; no host network changes")


if __name__ == "__main__":
    main()
