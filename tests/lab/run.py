#!/usr/bin/env python3
"""Isolated, ephemeral engine tests. Never connects NetworkManager on the host."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", default=shutil.which("podman") or shutil.which("docker"))
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if not args.runtime:
        parser.error("Podman or Docker is required")
    runtime = args.runtime
    image = "localhost/vpn-manager-lab:local"
    prefix = "vpn-manager-lab-" + uuid.uuid4().hex[:12]
    network = prefix + "-network"
    containers = []

    def command(*argv, input=None, check=True, timeout=120):
        return subprocess.run(argv, input=input, text=True, capture_output=True, check=check, timeout=timeout)

    def ctr(*argv, **kw):
        return command(runtime, *argv, **kw)

    if not args.skip_build:
        print("Building disposable lab image…", flush=True)
        # Show package build progress; no credentials exist in the build context.
        subprocess.run([runtime, "build", "-t", image, "-f", str(Path(__file__).with_name("Containerfile")), str(Path(__file__).parent)], check=True, timeout=600)

    with tempfile.TemporaryDirectory(prefix="vpn-manager-lab-") as temp:
        fixtures = Path(temp)
        os.chmod(fixtures, 0o700)

        def write(name, data):
            path = fixtures / name
            path.write_text(data)
            path.chmod(0o600)

        def crypto(*argv):
            # Capture key generation output; never print runtime secrets.
            ctr("run", "--rm", "--network", "none", "-v", f"{fixtures}:/fixtures:z", "--entrypoint", "openssl", image, *argv)

        try:
            crypto("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "/fixtures/ca.key", "-out", "/fixtures/ca.crt", "-days", "1", "-subj", "/CN=VPN Manager synthetic lab CA")
            for role, usage in [("server", "serverAuth"), ("client", "clientAuth")]:
                write(role + ".ext", f"basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage={usage}\n")
                crypto("req", "-newkey", "rsa:2048", "-nodes", "-keyout", f"/fixtures/{role}.key", "-out", f"/fixtures/{role}.csr", "-subj", f"/CN=synthetic-{role}")
                crypto("x509", "-req", "-in", f"/fixtures/{role}.csr", "-CA", "/fixtures/ca.crt", "-CAkey", "/fixtures/ca.key", "-CAcreateserial", "-out", f"/fixtures/{role}.crt", "-days", "1", "-extfile", f"/fixtures/{role}.ext")
            for path in fixtures.iterdir():
                path.chmod(0o600)
            common = "dev tun\nproto udp\nca /fixtures/ca.crt\ntls-version-min 1.2\ndata-ciphers AES-256-GCM\nverb 1\n"
            write("openvpn-server.conf", common + "cert /fixtures/server.crt\nkey /fixtures/server.key\ndh none\nserver 10.77.0.0 255.255.255.0\ntopology subnet\nserver-ipv6 fd77:77::/64\nkeepalive 1 5\n")
            write("openvpn-client.conf", common + "client\nremote openvpn-server 1194\ncert /fixtures/client.crt\nkey /fixtures/client.key\nremote-cert-tls server\nroute-nopull\nconnect-retry 1 2\n")
            keys = {}
            for role in ["server", "client"]:
                private = ctr("run", "--rm", "--network", "none", "--entrypoint", "wg", image, "genkey").stdout.strip()
                public = ctr("run", "--rm", "-i", "--network", "none", "--entrypoint", "wg", image, "pubkey", input=private + "\n").stdout.strip()
                keys[role] = private, public
            write("wireguard-server.conf", f"[Interface]\nPrivateKey={keys['server'][0]}\nListenPort=51820\n[Peer]\nPublicKey={keys['client'][1]}\nAllowedIPs=10.78.0.2/32,fd78:78::2/128\n")
            write("wireguard-client.conf", f"[Interface]\nPrivateKey={keys['client'][0]}\n[Peer]\nPublicKey={keys['server'][1]}\nAllowedIPs=10.78.0.1/32,fd78:78::1/128\nEndpoint=wireguard-server:51820\nPersistentKeepalive=1\n")
            ctr("network", "create", "--internal", network)
            results = []
            for protocol, v4, v6, interface in [
                ("openvpn", "10.77.0.1", "fd77:77::1", "tun0"),
                ("wireguard", "10.78.0.1", "fd78:78::1", "wg0"),
            ]:
                for role in ["server", "client"]:
                    name = prefix + "-" + protocol + "-" + role
                    containers.append(name)
                    ctr("run", "-d", "--name", name, "--network", network, "--network-alias", protocol + "-" + role,
                        "--cap-add", "NET_ADMIN", "--cap-add", "NET_RAW", "--device", "/dev/net/tun", "--sysctl", "net.ipv6.conf.all.disable_ipv6=0",
                        "-v", f"{fixtures}:/fixtures:z", image, protocol + "-" + role)
                server = prefix + "-" + protocol + "-server"
                client = prefix + "-" + protocol + "-client"
                deadline = time.monotonic() + 35
                while time.monotonic() < deadline:
                    if ctr("exec", client, "ping", "-c", "1", "-W", "1", v4, check=False).returncode == 0:
                        break
                    time.sleep(0.5)
                else:
                    raise RuntimeError(f"{protocol}: tunnel did not carry IPv4 within 35 seconds")
                ctr("exec", client, "ping", "-6", "-c", "1", "-W", "2", v6)
                ctr("exec", "-d", server, "dnsmasq", "--keep-in-foreground", "--no-resolv", "--no-hosts", "--bind-interfaces", f"--interface={interface}", f"--listen-address={v4}", f"--address=/vpn-manager.test/{v4}")
                ctr("exec", client, "sh", "-c", f"printf 'nameserver {v4}\\noptions timeout:1 attempts:2\\n' > /etc/resolv.conf")
                for attempt in range(10):
                    dns = ctr("exec", client, "getent", "ahostsv4", "vpn-manager.test", check=False)
                    if dns.returncode == 0 and v4 in dns.stdout:
                        break
                    time.sleep(0.2)
                else:
                    raise RuntimeError(f"{protocol}: DNS lookup through tunnel failed")
                if protocol == "openvpn":
                    ctr("stop", "-t", "3", client)
                else:
                    ctr("exec", client, "ip", "link", "delete", "wg0")
                    if ctr("exec", client, "ping", "-c", "1", "-W", "1", v4, check=False).returncode == 0:
                        raise RuntimeError("wireguard: tunnel address remained reachable after teardown")
                version = ctr("exec", server, "sh", "-c", "openvpn --version | head -1" if protocol == "openvpn" else "wg --version").stdout.strip()
                results.append({"protocol": protocol, "engine": version, "ipv4": "passed", "ipv6": "passed", "dns": "passed", "teardown": "passed"})
                print(f"{protocol}: IPv4, IPv6, DNS and teardown passed", flush=True)
            print(json.dumps({"scope": "isolated engines; not NetworkManager or vendor certification", "results": results}, indent=2), flush=True)
        except (RuntimeError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
            # Only synthetic lab engine logs. Never print generated configurations.
            for name in containers:
                logs = ctr("logs", "--tail", "15", name, check=False)
                diagnostic = logs.stdout + logs.stderr
                for private, public in locals().get("keys", {}).values():
                    diagnostic = diagnostic.replace(private, "[redacted]").replace(public, "[synthetic public key]")
                print(f"{name}:\n{diagnostic}", flush=True)
            raise
        finally:
            for name in reversed(containers):
                ctr("rm", "-f", name, check=False)
            ctr("network", "rm", network, check=False)


if __name__ == "__main__":
    try:
        main()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        # Do not dump command output: it could contain secret material.
        raise SystemExit(f"Lab command failed ({type(error).__name__}); runtime resources were cleaned up.")
