#!/usr/bin/env python3
"""Read-only nmcli fixture for documentation screenshots. Never invokes nmcli."""
import sys
import os
from pathlib import Path
import threading

PROFILES = {
    "11111111-1111-4111-8111-111111111111": ("Escritório · OpenVPN", "vpn", "yes"),
    "22222222-2222-4222-8222-222222222222": ("Laboratório · WireGuard", "wireguard", "no"),
    "33333333-3333-4333-8333-333333333333": ("Projeto · OpenVPN", "vpn", "no"),
}
VALUES = {
    "vpn.service-type": "org.freedesktop.NetworkManager.openvpn",
    "GENERAL.STATE": "activated",
    "GENERAL.STATE,GENERAL.DBUS-PATH": "activated\n/org/freedesktop/NetworkManager/ActiveConnection/1",
    "vpn.user-name": "usuario.exemplo",
    "vpn.data": "connection-type=password, remote=vpn.example.invalid, port=1194, ca=/exemplo/ca.crt, password-flags=2",
    "ipv4.dns": "192.0.2.53",
    "ipv4.dns-search": "~example.invalid",
    "ipv4.routes": "192.0.2.0/24",
    "ipv6.dns": "2001:db8::53",
    "ipv6.routes": "2001:db8::/32",
    "ipv4.never-default": "yes",
    "ipv6.never-default": "yes",
    "ipv4.ignore-auto-dns": "yes",
    "ipv6.ignore-auto-dns": "yes",
}


SAVED_ID = "33333333-3333-4333-8333-333333333333"


def saved_login_active():
    marker = os.environ.get("VPN_MANAGER_TEST_ACTIVATION")
    return bool(marker and Path(marker).exists())


def main():
    args = sys.argv[1:]
    if args[:2] == ["--wait", "40"]:
        args = args[2:]
    if os.environ.get("VPN_MANAGER_TEST_SAVED_LOGIN") == "1":
        if args == ["connection", "down", "uuid", SAVED_ID]:
            Path(os.environ["VPN_MANAGER_TEST_ACTIVATION"]).unlink(missing_ok=True)
            return
        if args[:4] == ["connection", "up", "uuid", SAVED_ID]:
            assert len(args) == 6 and args[4] == "passwd-file"
            assert "synthetic-saved-password" not in sys.argv
            value = Path(args[5]).read_text()
            assert value == "vpn.secrets.password:synthetic-saved-password\n"
            rejected = Path(os.environ["VPN_MANAGER_TEST_ACTIVATION"] + ".reject-once")
            if rejected.exists():
                rejected.unlink()
                raise SystemExit(4)
            Path(os.environ["VPN_MANAGER_TEST_ACTIVATION"]).write_text("active")
            print("Synthetic VPN activation confirmed")
            return
    if args == ["--version"]:
        print("nmcli tool, synthetic read-only fixture")
        return
    if args == ["-t", "-f", "RUNNING", "general"]:
        print("running")
        return
    if args == ["monitor"]:
        threading.Event().wait()
        return
    if args == ["-t", "-f", "UUID,NAME,TYPE,ACTIVE", "connection", "show"]:
        # Simulate a profile removed by another client between list/show.
        print("44444444-4444-4444-8444-444444444444:Vanished synthetic VPN:vpn:no")
        for uuid, values in PROFILES.items():
            if uuid == SAVED_ID and saved_login_active():
                values = (*values[:2], "yes")
            print(":".join((uuid, *values)))
        return
    if args == ["-t", "-f", "UUID", "connection", "show", "--active"]:
        print("11111111-1111-4111-8111-111111111111")
        if saved_login_active():
            print(SAVED_ID)
        return
    if len(args) >= 6 and args[:3] == ["--escape", "no", "-g"]:
        field = args[3]
        selector = args[4:]
        if selector[:2] == ["connection", "show"]:
            selector = selector[2:]
            if selector[:1] == ["uuid"]:
                selector = selector[1:]
            if selector == ["44444444-4444-4444-8444-444444444444"]:
                raise SystemExit(10)  # nmcli: connection not found
            if len(selector) == 1 and selector[0] in PROFILES:
                uuid = selector[0]
                if field == "connection.id":
                    print(PROFILES[uuid][0])
                elif field == "connection.type":
                    print(PROFILES[uuid][1])
                elif field in VALUES:
                    print(VALUES[field])
                else:
                    raise SystemExit("Unsupported screenshot property")
                return
    # Mutations, imports, activations and real profile reads are never forwarded.
    raise SystemExit("Operation blocked by read-only screenshot fixture")


if __name__ == "__main__":
    main()
