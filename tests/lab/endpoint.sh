#!/bin/sh
set -eu
mode=$1
case "$mode" in
  openvpn-server|openvpn-client)
    exec openvpn --config "/fixtures/$mode.conf"
    ;;
  wireguard-server|wireguard-client)
    if ! ip link add wg0 type wireguard 2>/dev/null; then
      wireguard-go wg0 >/tmp/wireguard-go.log 2>&1
    fi
    wg setconf wg0 "/fixtures/$mode.conf"
    if [ "$mode" = wireguard-server ]; then
      ip address add 10.78.0.1/24 dev wg0
      ip -6 address add fd78:78::1/64 dev wg0
    else
      ip address add 10.78.0.2/24 dev wg0
      ip -6 address add fd78:78::2/64 dev wg0
    fi
    ip link set wg0 up
    exec sleep infinity
    ;;
  *) exec "$@" ;;
esac
