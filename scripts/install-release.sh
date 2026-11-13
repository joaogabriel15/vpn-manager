#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
command -v nmcli >/dev/null || { echo 'Instale NetworkManager e o plugin VPN do sistema.' >&2; exit 1; }
install -Dm755 bin/vpn-manager "$HOME/.local/bin/vpn-manager"
install -Dm755 bin/vpn-manager-cli "$HOME/.local/bin/vpn-manager-cli"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
install -Dm644 vpn-manager.svg "$data_dir/icons/hicolor/scalable/apps/vpn-manager.svg"
for image in icons/vpn-manager-*.png; do
    size="${image##*-}"
    size="${size%.png}"
    install -Dm644 "$image" "$data_dir/icons/hicolor/${size}x${size}/apps/vpn-manager.png"
done
python3 - <<'PY'
from pathlib import Path
import os
home = Path.home()
data = Path(os.environ.get('XDG_DATA_HOME', home / '.local/share'))
app = str(home / '.local/bin/vpn-manager')
for char in ['\\', '"', '`', '$']:
    app = app.replace(char, '\\' + char)
app = app.replace('%', '%%')
launcher = data / 'applications/br.com.pacheco.VpnManager.desktop'
launcher.parent.mkdir(parents=True, exist_ok=True)
launcher.write_text(f'''[Desktop Entry]
Type=Application
Name=VPN Manager
Exec="{app}"
Icon=vpn-manager
Terminal=false
Categories=Network;
StartupWMClass=br.com.pacheco.VpnManager
''')
launcher.chmod(0o755)
PY
if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$data_dir/applications"
fi
echo 'VPN Manager instalado em ~/.local/bin.'
