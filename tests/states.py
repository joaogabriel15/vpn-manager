#!/usr/bin/env python3
"""Exercise the production adapter against synthetic nmcli and a private D-Bus."""
from pathlib import Path
import tempfile
from screenshots import demo_environment, run_demo

with tempfile.TemporaryDirectory(prefix="vpn-manager-states-") as folder, demo_environment(Path(folder)) as env:
    result = run_demo(["cargo", "run", "--locked", "-p", "vpn-manager-networkmanager", "--example", "observe_states"], env, timeout=120)
    if result.returncode or "VPN_STATES_OK" not in result.stdout:
        raise SystemExit(result.stdout + result.stderr)
    print(result.stdout.strip())
