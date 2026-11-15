#!/bin/sh
set -eu
# The runtime bind-mounts resolv.conf; NetworkManager must own a plain file to apply VPN DNS.
umount /etc/resolv.conf 2>/dev/null || true
: > /etc/resolv.conf
# VPN activation needs a managed base connection with a default route. Keep the
# runtime-assigned address; the internal lab network has no router, so the
# gateway is the unused .1 address.
address=$(ip -4 -o address show eth0 | awk '{print $4}')
host=${address%/*}
umask 077
cat > /etc/NetworkManager/system-connections/lab-uplink.nmconnection <<EOF
[connection]
id=lab-uplink
type=ethernet
interface-name=eth0

[ipv4]
method=manual
address1=$address,${host%.*}.1
dns=192.0.2.53;

[ipv6]
method=ignore
EOF
mkdir -p /run/dbus
dbus-daemon --system --fork
# --debug only keeps it in the foreground logging to stderr (there is no syslog here).
exec NetworkManager --debug --log-level=INFO
