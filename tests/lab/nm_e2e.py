#!/usr/bin/env python3
"""App CLI -> NetworkManager -> gateway, end to end, in disposable containers.

NetworkManager runs in a privileged container with its own system D-Bus and
network namespace, so the host's NetworkManager and routes are never touched.
All certificates, keys and names are generated per run and are synthetic.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid

LAB = Path(__file__).parent
SERVER_IMAGE = "localhost/vpn-manager-lab:local"
CLIENT_IMAGE = "localhost/vpn-manager-lab-nm:local"
PROTOCOLS = {
    # protocol: (server tunnel IPv4, IPv6, IPv4 subnet route, IPv6 subnet route)
    "openvpn": ("10.77.0.1", "fd77:77::1", "10.77.0.0/24", "fd77:77::/64"),
    "wireguard": ("10.78.0.1", "fd78:78::1", "10.78.0.0/24", "fd78:78::/64"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", default=shutil.which("docker") or shutil.which("podman"))
    parser.add_argument("--cli", type=Path, required=True, help="built vpn-manager-cli for this Linux")
    parser.add_argument("--protocols", default="openvpn,wireguard")
    args = parser.parse_args()
    protocols = args.protocols.split(",")
    runtime = args.runtime
    prefix = "vpn-manager-nm-lab-" + uuid.uuid4().hex[:10]
    network = prefix + "-net"
    client = prefix + "-client"
    containers = []

    def ctr(*argv, check=True, timeout=120, input=None):
        return subprocess.run([runtime, *argv], text=True, capture_output=True, check=check, timeout=timeout, input=input)

    def sh(container, script, check=True, timeout=60):
        return ctr("exec", container, "sh", "-c", script, check=check, timeout=timeout)

    def cli(*argv, check=True):
        return ctr("exec", client, "vpn-manager-cli", *argv, check=check, timeout=90)

    def profile(name):
        for line in cli("list").stdout.splitlines():
            fields = line.split("\t")
            if fields[3] == name:
                return fields[0], fields[1]
        raise RuntimeError(f"{name}: profile not listed by the CLI")

    def wait(description, condition, seconds=45):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if condition():
                return
            time.sleep(1)
        raise RuntimeError(f"timed out: {description}")

    def network_state():
        routes = sh(client, "ip -4 route show table main; ip -6 route show table main").stdout
        return sorted(routes.splitlines()), sh(client, "cat /etc/resolv.conf").stdout

    def restored(description, baseline):
        try:
            wait(f"{description}: routes and DNS restored", lambda: network_state() == baseline, 20)
        except RuntimeError:
            routes, resolv = network_state()
            print("missing:", sorted(set(baseline[0]) - set(routes)), "extra:", sorted(set(routes) - set(baseline[0])),
                  f"\nresolv before:\n{baseline[1]}resolv after:\n{resolv}", flush=True)
            raise

    def ip_of(container):
        return ctr("inspect", "-f", "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}", container).stdout.strip()

    for image, folder in [(SERVER_IMAGE, LAB), (CLIENT_IMAGE, LAB / "nm")]:
        subprocess.run([runtime, "build", "-q", "-t", image, "-f", str(folder / "Containerfile"), str(folder)], check=True, timeout=900)

    with tempfile.TemporaryDirectory(prefix="vpn-manager-nm-lab-") as temp:
        fixtures = Path(temp)
        fixtures.chmod(0o700)

        def write(name, text):
            (fixtures / name).write_text(text)
            (fixtures / name).chmod(0o600)

        def openssl(*argv):
            ctr("run", "--rm", "--network", "none", "-v", f"{fixtures}:/fixtures:z", "--entrypoint", "openssl", SERVER_IMAGE, *argv)

        try:
            openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "/fixtures/ca.key", "-out", "/fixtures/ca.crt", "-days", "1", "-subj", "/CN=VPN Manager synthetic NM lab CA")
            for role, usage in [("server", "serverAuth"), ("client", "clientAuth")]:
                write(role + ".ext", f"basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage={usage}\n")
                openssl("req", "-newkey", "rsa:2048", "-nodes", "-keyout", f"/fixtures/{role}.key", "-out", f"/fixtures/{role}.csr", "-subj", f"/CN=synthetic-{role}")
                openssl("x509", "-req", "-in", f"/fixtures/{role}.csr", "-CA", "/fixtures/ca.crt", "-CAkey", "/fixtures/ca.key", "-CAcreateserial", "-out", f"/fixtures/{role}.crt", "-days", "1", "-extfile", f"/fixtures/{role}.ext")
            write("openvpn-server.conf", "dev tun\nproto udp\nca /fixtures/ca.crt\ncert /fixtures/server.crt\nkey /fixtures/server.key\ndh none\n"
                  "tls-version-min 1.2\ndata-ciphers AES-256-GCM\nserver 10.77.0.0 255.255.255.0\ntopology subnet\nserver-ipv6 fd77:77::/64\n"
                  "ping 1\nping-restart 10\npush \"dhcp-option DNS 10.77.0.1\"\npush \"dhcp-option DOMAIN vpn-manager.test\"\nverb 1\n")
            keys = {}
            for role in ["server", "client"]:
                private = ctr("run", "--rm", "--network", "none", "--entrypoint", "wg", SERVER_IMAGE, "genkey").stdout.strip()
                public = ctr("run", "--rm", "-i", "--network", "none", "--entrypoint", "wg", SERVER_IMAGE, "pubkey", input=private + "\n").stdout.strip()
                keys[role] = private, public
            write("wireguard-server.conf", f"[Interface]\nPrivateKey={keys['server'][0]}\nListenPort=51820\n[Peer]\nPublicKey={keys['client'][1]}\nAllowedIPs=10.78.0.2/32,fd78:78::2/128\n")
            # Rootful runtimes create these as root, so tighten modes from inside a container.
            ctr("run", "--rm", "--network", "none", "-v", f"{fixtures}:/fixtures:z", "--entrypoint", "chmod", SERVER_IMAGE, "-R", "go-rwx", "/fixtures")

            ctr("network", "create", "--internal", network)
            containers.append(client)
            # Not --privileged: a read-only /sys makes NetworkManager use netlink without udev.
            # SYS_ADMIN/unconfined only let the entrypoint drop the runtime's resolv.conf mount.
            ctr("run", "-d", "--name", client, "--cap-add", "NET_ADMIN", "--cap-add", "NET_RAW", "--cap-add", "SYS_ADMIN",
                "--security-opt", "apparmor=unconfined", "--device", "/dev/net/tun", "--network", network, "--sysctl", "net.ipv6.conf.all.disable_ipv6=0",
                "-v", f"{fixtures}:/fixtures:ro,z", "-v", f"{args.cli.resolve()}:/usr/local/bin/vpn-manager-cli:ro,z", CLIENT_IMAGE)
            wait("NetworkManager uplink and DNS", lambda: "nameserver 192.0.2.53" in sh(client, "cat /etc/resolv.conf").stdout)
            print(sh(client, "nmcli --version; nmcli -t -f DEVICE,TYPE,STATE device").stdout, flush=True)

            for protocol in protocols:
                v4, v6, route4, route6 = PROTOCOLS[protocol]
                server = f"{prefix}-{protocol}-server"
                containers.append(server)
                ctr("run", "-d", "--name", server, "--network", network, "--cap-add", "NET_ADMIN", "--cap-add", "NET_RAW", "--device", "/dev/net/tun",
                    "--sysctl", "net.ipv6.conf.all.disable_ipv6=0", "-v", f"{fixtures}:/fixtures:z", SERVER_IMAGE, protocol + "-server")
                interface = "tun0" if protocol == "openvpn" else "wg0"
                wait(f"{protocol} server interface", lambda: sh(server, f"ip -4 addr show {interface} | grep -q {v4}", check=False).returncode == 0)
                ctr("exec", "-d", server, "dnsmasq", "--keep-in-foreground", "--no-resolv", "--no-hosts", "--bind-interfaces",
                    f"--listen-address={v4}", f"--address=/vpn-manager.test/{v4}")

                endpoint = ip_of(server)
                if protocol == "openvpn":
                    name, file = "lab-openvpn", "lab-openvpn.ovpn"
                    write(file, f"client\ndev tun\nproto udp\nremote {endpoint} 1194\nca /fixtures/ca.crt\ncert /fixtures/client.crt\nkey /fixtures/client.key\n"
                          "remote-cert-tls server\ntls-version-min 1.2\ndata-ciphers AES-256-GCM\nping 1\nping-exit 8\n")
                else:
                    # NetworkManager names the interface after the file, so keep it short.
                    name, file = "wglab", "wglab.conf"
                    write(file, f"[Interface]\nPrivateKey={keys['client'][0]}\nAddress=10.78.0.2/24, fd78:78::2/64\nDNS=10.78.0.1, vpn-manager.test\n"
                          f"[Peer]\nPublicKey={keys['server'][1]}\nEndpoint={endpoint}:51820\nAllowedIPs=10.78.0.0/24, fd78:78::/64\nPersistentKeepalive=1\n")

                cli("import", f"/fixtures/{file}")
                profile_id, label = profile(name)
                # The app must not leave imported profiles connecting on their own.
                wait(f"{protocol} idle after import", lambda: profile(name)[1] == "Desconectada")
                baseline = network_state()

                def connected_and_carrying():
                    cli("connect", profile_id)
                    wait(f"{protocol} connected state", lambda: profile(name)[1] == "Conectada")
                    wait(f"{protocol} IPv4 traffic", lambda: sh(client, f"ping -c1 -W1 {v4}", check=False).returncode == 0, 20)
                    wait(f"{protocol} IPv6 traffic", lambda: sh(client, f"ping -6 -c1 -W1 {v6}", check=False).returncode == 0, 20)
                    routes, resolv = network_state()
                    if not any(line.startswith(route4) for line in routes) or not any(line.startswith(route6) for line in routes):
                        raise RuntimeError(f"{protocol}: tunnel routes missing")
                    if f"nameserver {v4}" not in resolv or "vpn-manager.test" not in resolv:
                        raise RuntimeError(f"{protocol}: VPN DNS not applied")
                    if v4 not in sh(client, "getent ahostsv4 vpn-manager.test", check=False).stdout:
                        raise RuntimeError(f"{protocol}: name not resolved through the tunnel")

                connected_and_carrying()
                cli("disconnect", profile_id)
                wait(f"{protocol} disconnected state", lambda: profile(name)[1] == "Desconectada")
                restored(f"{protocol} disconnect", baseline)
                print(f"{protocol}: import, connect, IPv4, IPv6, DNS, routes and disconnect restore passed", flush=True)

                connected_and_carrying()
                ctr("stop", "-t", "0", server)
                if protocol == "openvpn":
                    # ping-exit makes the engine give up; NetworkManager must tear down and restore.
                    wait("openvpn leaves connected after server loss", lambda: profile(name)[1] != "Conectada")
                    restored("openvpn server loss", baseline)
                else:
                    # WireGuard has no liveness signal: traffic stops but the profile stays active.
                    wait("wireguard traffic stops after server loss", lambda: sh(client, f"ping -c1 -W1 {v4}", check=False).returncode != 0, 20)
                    if profile(name)[1] != "Conectada":
                        raise RuntimeError("wireguard: unexpected state change without liveness signal")
                    cli("disconnect", profile_id)
                    restored("wireguard manual disconnect", baseline)
                print(f"{protocol}: server loss handled and network restored", flush=True)
            print("LINUX_NM_TUNNEL_OK", flush=True)
        except (RuntimeError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            if isinstance(error, subprocess.CalledProcessError):
                print(f"command failed: {error.cmd[1:4]} rc={error.returncode}\n{error.stdout}\n{error.stderr}", flush=True)
            for name in containers:
                logs = ctr("logs", "--tail", "40", name, check=False)
                diagnostic = logs.stdout + logs.stderr
                for private, _ in locals().get("keys", {}).values():
                    diagnostic = diagnostic.replace(private, "[redacted]")
                print(f"--- {name}\n{diagnostic}", flush=True)
            raise
        finally:
            for name in reversed(containers):
                ctr("rm", "-f", name, check=False)
            ctr("network", "rm", network, check=False)


if __name__ == "__main__":
    main()
