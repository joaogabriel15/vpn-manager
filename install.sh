#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
command -v nmcli >/dev/null || { echo 'Instale NetworkManager primeiro.' >&2; exit 1; }
if [[ ! -x target/release/vpn-manager ]]; then
  cargo build --release --locked
fi
install -Dm755 target/release/vpn-manager "$HOME/.local/bin/vpn-manager"
install -Dm644 packaging/vpn-manager.svg "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/vpn-manager.svg"
for size in 16 22 24 32 48 64 128 256; do
  install -Dm644 "packaging/icons/vpn-manager-$size.png" "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/${size}x${size}/apps/vpn-manager.png"
done
python3 - <<'PY'
from pathlib import Path
import os, subprocess
home = Path.home()
app = home / '.local/bin/vpn-manager'
escaped = str(app).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
data = Path(os.environ.get('XDG_DATA_HOME', home / '.local/share'))
icon = data / 'icons/hicolor/256x256/apps/vpn-manager.png'
content = f'''[Desktop Entry]
Version=1.0
Type=Application
Name=VPN Manager
Comment=Gerencie suas VPNs com facilidade
Exec="{escaped}"
Icon={icon}
Terminal=false
Categories=Network;
StartupNotify=true
StartupWMClass=br.com.pacheco.VpnManager
'''
data = Path(os.environ.get('XDG_DATA_HOME', home / '.local/share'))
launcher = data / 'applications/br.com.pacheco.VpnManager.desktop'
launcher.parent.mkdir(parents=True, exist_ok=True)
launcher.write_text(content)
launcher.chmod(0o755)
# Keep old launchers working while the canonical filename matches the Wayland app ID.
legacy = launcher.with_name('vpn-manager.desktop')
legacy.write_text(content + 'NoDisplay=true\n')
legacy.chmod(0o755)
try:
    desktop = Path(subprocess.check_output(['xdg-user-dir', 'DESKTOP'], text=True).strip())
    if desktop != home:
        desktop.mkdir(parents=True, exist_ok=True)
        shortcut = desktop / 'vpn-manager.desktop'
        shortcut.write_text(content)
        shortcut.chmod(0o755)
except (FileNotFoundError, subprocess.CalledProcessError):
    pass
PY
if command -v update-desktop-database >/dev/null; then
  update-desktop-database "${XDG_DATA_HOME:-$HOME/.local/share}/applications"
fi
if command -v gtk-update-icon-cache >/dev/null; then
  gtk-update-icon-cache -f -t "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor" >/dev/null 2>&1 || true
fi
if command -v kbuildsycoca6 >/dev/null; then
  kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi
if command -v qdbus6 >/dev/null; then
  qdbus6 org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell.evaluateScript "$(cat packaging/migrate-kde-launcher.js)" >/dev/null 2>&1 || true
fi
printf '%s\n' 'VPN Manager instalado. Abra pelo menu de aplicativos.'
