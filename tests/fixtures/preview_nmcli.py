#!/usr/bin/env python3
"""Read-only nmcli fixture for documentation screenshots. Never invokes nmcli."""
import sys
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


def main():
    args = sys.argv[1:]
    if args[:2] == ["--wait", "40"]:
        args = args[2:]
    if args == ["monitor"]:
        threading.Event().wait()
        return
    if args == ["-t", "-f", "UUID,NAME,TYPE,ACTIVE", "connection", "show"]:
        for uuid, values in PROFILES.items():
            print(":".join((uuid, *values)))
        return
    if args == ["-t", "-f", "UUID", "connection", "show", "--active"]:
        print("11111111-1111-4111-8111-111111111111")
        return
    if len(args) >= 6 and args[:3] == ["--escape", "no", "-g"]:
        field = args[3]
        selector = args[4:]
        if selector[:2] == ["connection", "show"]:
            selector = selector[2:]
            if selector[:1] == ["uuid"]:
                selector = selector[1:]
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
