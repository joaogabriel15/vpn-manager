#!/usr/bin/env python3
"""Synthetic VPN state service; run only on the private screenshot/test bus."""
import sys
from gi.repository import Gio, GLib

PATH = "/org/freedesktop/NetworkManager/ActiveConnection/1"
INTERFACE = "org.freedesktop.NetworkManager.VPN.Connection"
XML = """<node>
  <interface name="org.freedesktop.NetworkManager.VPN.Connection">
    <property name="VpnState" type="u" access="read"/>
    <signal name="VpnStateChanged"><arg type="u"/><arg type="u"/></signal>
  </interface>
  <interface name="org.example.VpnManager.Fixture">
    <method name="SetState"><arg type="u" direction="in"/></method>
  </interface>
</node>"""

state = 5
bus = Gio.DBusConnection.new_for_address_sync(
    sys.argv[1], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
    None, None,
)
node = Gio.DBusNodeInfo.new_for_xml(XML)


def get_property(connection, sender, path, interface, name):
    return GLib.Variant("u", state) if name == "VpnState" else None


def control(connection, sender, path, interface, name, parameters, invocation):
    global state
    state, = parameters.unpack()
    invocation.return_value(GLib.Variant("()", ()))
    connection.emit_signal(None, PATH, INTERFACE, "VpnStateChanged", GLib.Variant("(uu)", (state, 0)))


bus.register_object(PATH, node.interfaces[0], None, get_property, None)
bus.register_object(PATH, node.interfaces[1], control, None, None)
bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
              GLib.Variant("(su)", ("org.freedesktop.NetworkManager", 0)), None, Gio.DBusCallFlags.NONE, 5000, None)
print("FIXTURE_READY", flush=True)
GLib.MainLoop().run()
