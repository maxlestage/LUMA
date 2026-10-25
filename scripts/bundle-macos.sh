#!/usr/bin/env bash
# Fabrique LÜMA.app (universelle : Apple Silicon et Intel) dans target/macos/.
# À lancer sur un Mac : ./scripts/bundle-macos.sh
#
# L'app est signée « ad hoc » (sans compte développeur Apple) : au premier lancement
# d'une copie téléchargée, macOS demande de l'ouvrir par clic droit → Ouvrir.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
out=target/macos
app="$out/LÜMA.app"

rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release -p luma-studio --target "$target"
done

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/luma-studio" \
  target/aarch64-apple-darwin/release/luma-studio \
  target/x86_64-apple-darwin/release/luma-studio

# L'icône : dessinée par le code, déclinée aux tailles de macOS.
iconset="$out/LUMA.iconset"
rm -rf "$iconset" && mkdir -p "$iconset"
cargo run -q --release -p luma-studio --example icon -- "$out/icon.png"
for size in 16 32 128 256 512; do
  sips -z $size $size "$out/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$out/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/LUMA.icns"

sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
codesign --force --sign - "$app"

(cd "$out" && rm -f LUMA-macos.zip && ditto -c -k --keepParent "LÜMA.app" LUMA-macos.zip)
echo "→ $app ($version) et $out/LUMA-macos.zip"
