#!/bin/sh
# Installe LÜMA pour vous seul (sans mot de passe administrateur) : le programme dans
# ~/.local/bin, le lanceur et l'icône dans ~/.local/share. Pour tout le monde :
#   sudo ./install.sh --system   (dans /usr/local)
# Pour désinstaller : ./install.sh --remove (ou sudo ./install.sh --system --remove).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix="${XDG_DATA_HOME:-$HOME/.local/share}/.."
prefix=$(cd "$prefix" 2>/dev/null && pwd || echo "$HOME/.local")
remove=0
for arg in "$@"; do
  case "$arg" in
    --system) prefix=/usr/local ;;
    --remove) remove=1 ;;
    *) echo "usage : $0 [--system] [--remove]"; exit 2 ;;
  esac
done

if [ $remove = 1 ]; then
  rm -f "$prefix/bin/luma-studio" "$prefix/bin/luma" \
    "$prefix/share/applications/luma-studio.desktop"
  find "$prefix/share/icons/hicolor" -name luma-studio.png -delete 2>/dev/null || true
  echo "LÜMA est désinstallé (vos photos, réglages et films restent à leur place)."
else
  mkdir -p "$prefix/bin" "$prefix/share/applications"
  install -m 755 "$here/bin/luma-studio" "$here/bin/luma" "$prefix/bin/"
  install -m 644 "$here/share/applications/luma-studio.desktop" "$prefix/share/applications/"
  cp -R "$here/share/icons" "$prefix/share/"
  echo "LÜMA est installé : cherchez « LÜMA » dans vos applications,"
  echo "ou lancez « luma-studio » (et « luma » en ligne de commande)."
  case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "($prefix/bin n'est pas dans votre PATH.)" ;; esac
fi
command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" 2>/dev/null || true
