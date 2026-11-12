#!/usr/bin/env python3
"""Measure only the GUI process after its read-only smoke test, on Linux."""
import json
import os
from pathlib import Path
import select
import subprocess
import time

binary = Path(__file__).resolve().parents[1] / "target/release/vpn-manager"
process = subprocess.Popen([str(binary), "--self-test"], env={**os.environ, "VPN_MANAGER_IDLE_TEST": "1"}, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        readable, _, _ = select.select([process.stdout], [], [], 1)
        if readable:
            line = process.stdout.readline()
            if line.strip() == "UI_IDLE_READY":
                break
            if not line:
                raise RuntimeError("GUI exited before idle measurement")
    else:
        raise RuntimeError("GUI did not finish smoke test within 45 seconds")

    def sample():
        # The process name may contain spaces; fields after ')' begin at state.
        fields = Path(f"/proc/{process.pid}/stat").read_text().rsplit(")", 1)[1].split()
        ticks = int(fields[11]) + int(fields[12])
        status = Path(f"/proc/{process.pid}/status").read_text().splitlines()
        rss = int(next(v.split()[1] for v in status if v.startswith("VmRSS:")))
        return ticks, rss

    start = time.monotonic()
    before, _ = sample()
    time.sleep(10)
    after, rss = sample()
    elapsed = time.monotonic() - start
    cpu_seconds = (after - before) / os.sysconf("SC_CLK_TCK")
    _, _ = process.communicate(timeout=25)
    if process.returncode:
        raise RuntimeError("GUI smoke test failed")
    print(json.dumps({"scope": "GUI process only; no tunnel engine", "duration_seconds": round(elapsed, 2), "cpu_seconds": cpu_seconds, "cpu_percent_one_core": round(cpu_seconds / elapsed * 100, 3), "rss_kib": rss}, indent=2))
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
