# Binary-only spec: files are staged by scripts/package-release.py.
Name:           vpn-manager
Version:        %{pkg_version}
Release:        1
Summary:        Simple VPN manager for NetworkManager
License:        MIT
URL:            https://github.com/joaogabriel15/vpn-manager
Requires:       NetworkManager
Recommends:     NetworkManager-openvpn

%description
GTK interface, tray and CLI to list, import, edit and connect VPN profiles
managed by NetworkManager.

%install
cp -a %{staged_root}/. %{buildroot}/

%files
%license /usr/share/licenses/vpn-manager/LICENSE
/usr/bin/vpn-manager
/usr/bin/vpn-manager-cli
/usr/share/applications/br.com.pacheco.VpnManager.desktop
/usr/share/icons/hicolor/*/apps/vpn-manager.*
/usr/share/doc/vpn-manager
