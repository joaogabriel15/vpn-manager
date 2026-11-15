#!/usr/bin/env bash
# Build signed APT and DNF repositories from the release .deb/.rpm files.
# Usage: REPO_SIGNING_KEY=<armored private key> build-package-repos.sh <packages-dir> <site-dir>
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
incoming=$(realpath "$1")
site=$(realpath -m "$2")
base_url=${PAGES_URL:-https://joaogabriel15.github.io/vpn-manager}
base_url=${base_url%/}

GNUPGHOME=$(mktemp -d)
export GNUPGHOME
trap 'gpgconf --kill gpg-agent 2>/dev/null || true; rm -rf "$GNUPGHOME"' EXIT
printf '%s\n' "$REPO_SIGNING_KEY" | gpg --batch --quiet --import
key=$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr/ {print $10; exit}')
sign() { gpg --batch --yes --local-user "$key" "$@"; }

mkdir -p "$site/apt/pool/main" "$site/apt/dists/stable/main/binary-amd64" "$site/rpm"
cp "$incoming"/*.deb "$site/apt/pool/main/"
(
    cd "$site/apt"
    apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
    gzip -9kn dists/stable/main/binary-amd64/Packages
    apt-ftparchive \
        -o APT::FTPArchive::Release::Origin="VPN Manager" \
        -o APT::FTPArchive::Release::Label="VPN Manager" \
        -o APT::FTPArchive::Release::Suite=stable \
        -o APT::FTPArchive::Release::Codename=stable \
        -o APT::FTPArchive::Release::Architectures=amd64 \
        -o APT::FTPArchive::Release::Components=main \
        release dists/stable > dists/stable/Release
    sign --clearsign -o dists/stable/InRelease dists/stable/Release
    sign --armor --detach-sign -o dists/stable/Release.gpg dists/stable/Release
)

cp "$incoming"/*.rpm "$site/rpm/"
printf '%%_gpg_name %s\n%%__gpg %s\n' "$key" "$(command -v gpg)" > "$GNUPGHOME/rpmmacros"
rpmsign --macros="/usr/lib/rpm/macros:$GNUPGHOME/rpmmacros" --addsign "$site"/rpm/*.rpm
createrepo_c --quiet "$site/rpm"
sign --armor --detach-sign -o "$site/rpm/repodata/repomd.xml.asc" "$site/rpm/repodata/repomd.xml"

cp "$root/packaging/repo/vpn-manager.asc" "$site/vpn-manager.asc"
cat > "$site/vpn-manager.repo" <<EOF
[vpn-manager]
name=VPN Manager
baseurl=$base_url/rpm
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=$base_url/vpn-manager.asc
EOF
cat > "$site/index.html" <<EOF
<!doctype html>
<html lang="pt-BR"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>VPN Manager — repositórios</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:46rem;margin:2rem auto;padding:0 1rem}pre{background:#f2f4f7;padding:1rem;overflow-x:auto;border-radius:6px}</style>
<h1>VPN Manager</h1>
<p>Instale e receba atualizações pelo gerenciador de pacotes do seu Linux. Outras opções em <a href="https://github.com/joaogabriel15/vpn-manager#baixar-e-instalar">github.com/joaogabriel15/vpn-manager</a>.</p>
<h2>Ubuntu, Debian e Mint</h2>
<pre>curl -fsSL $base_url/vpn-manager.asc | sudo gpg --dearmor -o /usr/share/keyrings/vpn-manager.gpg
echo "deb [signed-by=/usr/share/keyrings/vpn-manager.gpg] $base_url/apt stable main" | sudo tee /etc/apt/sources.list.d/vpn-manager.list
sudo apt update &amp;&amp; sudo apt install vpn-manager</pre>
<h2>Fedora</h2>
<pre>sudo curl -fsSL -o /etc/yum.repos.d/vpn-manager.repo $base_url/vpn-manager.repo
sudo dnf install vpn-manager</pre>
<h2>openSUSE</h2>
<pre>sudo zypper addrepo $base_url/vpn-manager.repo
sudo zypper install vpn-manager</pre>
<p>Chave de assinatura: <a href="vpn-manager.asc">vpn-manager.asc</a> (D3EF 5AFE 4AFE B7F8 DAF3 D5DC C0F5 7413 B694 E98A).</p>
</html>
EOF
