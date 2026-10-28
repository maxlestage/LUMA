#!/usr/bin/env bash
# Fabrique LÜMA pour Linux (x86_64 ou aarch64, selon la machine) dans target/linux/ :
# - LUMA-linux-<arch>.tar.gz : l'app, la ligne de commande, le lanceur, les icônes et
#   install.sh (installation sans mot de passe dans ~/.local) ;
# - LUMA-linux-<arch Debian>.deb : le même, pour Debian et Ubuntu (si dpkg-deb est
#   là). Des noms sans numéro de version : les liens du site ne changent pas.
# À lancer sur Linux : ./scripts/bundle-linux.sh
# Pour que l'app tourne sur des systèmes plus anciens, la compiler sur le plus ancien
# visé (la CI : Ubuntu 22.04, glibc 2.35).
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
out=target/linux
name="LUMA-linux-$arch"
root="$out/$name"

cargo build --release -p luma-studio -p luma-cli
rm -rf "$root"
mkdir -p "$root/bin" "$root/share/applications"
install -m 755 target/release/luma-studio target/release/luma "$root/bin/"
strip "$root/bin/luma-studio" "$root/bin/luma" 2>/dev/null || true
install -m 644 packaging/linux/luma-studio.desktop "$root/share/applications/"
install -m 755 packaging/linux/install.sh "$root/"

# Les icônes : le logo de LÜMA, dessiné par le code (luma-brand), aux tailles du thème
# d'icônes ; aux plus petites, l'anneau seul (le nom y serait illisible).
cargo build -q --release -p luma-studio --example icon
for size in 16 24 32 48 64 128 256 512 1024; do
  dir="$root/share/icons/hicolor/${size}x${size}/apps"
  mkdir -p "$dir"
  kind=mac
  [ $size -le 32 ] && kind=mac-petit
  target/release/examples/icon "$dir/luma-studio.png" "$kind" "$size"
done

cat > "$root/LISEZMOI.txt" <<TXT
LÜMA $version pour Linux ($arch)

Installer (pour vous seul, sans mot de passe) :  ./install.sh
Installer pour tout le monde :                  sudo ./install.sh --system
Désinstaller :                                  ./install.sh --remove
Ou sans installer :                             ./bin/luma-studio

Il faut un pilote graphique Vulkan (Mesa sur Intel et AMD, le pilote NVIDIA) et la
bibliothèque du clavier ; sur Debian et Ubuntu (déjà là sur un bureau installé) :
  sudo apt install mesa-vulkan-drivers libxkbcommon-x11-0
Sur Fedora : sudo dnf install mesa-vulkan-drivers libxkbcommon-x11
Les fenêtres pour choisir un fichier passent par le portail du bureau
(xdg-desktop-portal, présent sur GNOME, KDE et les autres bureaux récents).

Un mois d'essai, complet, puis LÜMA Premium : https://maxlestage.github.io/LUMA/
TXT

tar -C "$out" -czf "$out/$name.tar.gz" "$name"
echo "$out/$name.tar.gz"

# Le paquet Debian : les mêmes fichiers dans /usr.
if command -v dpkg-deb >/dev/null; then
  deb_arch=$(dpkg --print-architecture)
  pkg="$out/deb"
  rm -rf "$pkg"
  mkdir -p "$pkg/DEBIAN" "$pkg/usr"
  cp -R "$root/bin" "$root/share" "$pkg/usr/"
  size=$(du -sk "$pkg/usr" | cut -f1)
  cat > "$pkg/DEBIAN/control" <<CTRL
Package: luma
Version: $version
Architecture: $deb_arch
Maintainer: Maxime Lestage <maxlestage@icloud.com>
Installed-Size: $size
Depends: libc6 (>= 2.35), libxkbcommon0, libxkbcommon-x11-0, mesa-vulkan-drivers | vulkan-icd
Recommends: xdg-desktop-portal
Section: graphics
Priority: optional
Homepage: https://maxlestage.github.io/LUMA/
Description: LÜMA, développement photo RAW
 Développez vos photos RAW : 275 films, masques, fusions (HDR, panoramas,
 empilements), export JPEG, PNG et TIFF 16 bits. Le même moteur et le même rendu
 que sur Mac, iPhone et iPad. Comprend la ligne de commande « luma ».
CTRL
  dpkg-deb --root-owner-group --build "$pkg" "$out/LUMA-linux-$deb_arch.deb" >/dev/null
  echo "$out/LUMA-linux-$deb_arch.deb"
fi
