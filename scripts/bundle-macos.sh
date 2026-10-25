#!/usr/bin/env bash
# Fabrique LÜMA.app (universelle : Apple Silicon et Intel) dans target/macos/.
# À lancer sur un Mac : ./scripts/bundle-macos.sh
#
# Signature :
# - avec un certificat « Developer ID Application » (compte Apple Developer) dans
#   MACOS_CERTIFICATE (le .p12 en base64) et MACOS_CERTIFICATE_PASSWORD : signée avec
#   le runtime renforcé, puis, avec une clé d'API App Store Connect (APPLE_API_KEY : le
#   contenu du .p8, APPLE_API_KEY_ID, APPLE_API_ISSUER), notariée par Apple et agrafée :
#   elle s'ouvre d'un double clic sur n'importe quel Mac ;
# - sinon « ad hoc » : macOS 15 et plus refusent de l'ouvrir au premier lancement ;
#   Réglages Système → Confidentialité et sécurité → « Ouvrir quand même ».
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

# L'icône : le logo de LÜMA, dessiné par le code (luma-brand), aux tailles de macOS.
iconset="$out/LUMA.iconset"
rm -rf "$iconset" && mkdir -p "$iconset"
# Le logo avec « LÜMA » ; aux plus petites tailles, l'anneau seul (le nom y serait
# illisible).
cargo run -q --release -p luma-studio --example icon -- "$out/icon.png" mac
cargo run -q --release -p luma-studio --example icon -- "$out/icon-small.png" mac-petit
for size in 16 32 128 256 512; do
  src="$out/icon.png"
  [ $size -le 32 ] && src="$out/icon-small.png"
  sips -z $size $size "$src" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) "$src" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/LUMA.icns"

sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
zip_app() { (cd "$out" && rm -f LUMA-macos.zip && ditto -c -k --keepParent "LÜMA.app" LUMA-macos.zip); }

if [ -n "${MACOS_CERTIFICATE:-}" ]; then
  # Un trousseau jetable, le temps de signer.
  keychain="$RUNNER_TEMP/luma-signature.keychain-db"
  [ -n "${RUNNER_TEMP:-}" ] || keychain="$(mktemp -d)/luma-signature.keychain-db"
  secret=$(uuidgen)
  security create-keychain -p "$secret" "$keychain"
  security set-keychain-settings -lut 3600 "$keychain"
  security unlock-keychain -p "$secret" "$keychain"
  echo "$MACOS_CERTIFICATE" | base64 --decode > "$out/certificat.p12"
  security import "$out/certificat.p12" -k "$keychain" -P "${MACOS_CERTIFICATE_PASSWORD:-}" -T /usr/bin/codesign
  rm -f "$out/certificat.p12"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$secret" "$keychain" >/dev/null
  security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
  identity=$(security find-identity -v -p codesigning "$keychain" | sed -n 's/.*"\(Developer ID Application:[^"]*\)".*/\1/p' | head -1)
  [ -n "$identity" ] || { echo "Aucun certificat « Developer ID Application » dans MACOS_CERTIFICATE" >&2; exit 1; }
  codesign --force --options runtime --timestamp --sign "$identity" "$app"
  codesign --verify --strict --verbose=2 "$app"
  echo "Signée : $identity"
  if [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_KEY_ID:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ]; then
    key="$out/AuthKey_$APPLE_API_KEY_ID.p8"
    printf '%s' "$APPLE_API_KEY" > "$key"
    zip_app
    xcrun notarytool submit "$out/LUMA-macos.zip" --key "$key" --key-id "$APPLE_API_KEY_ID" \
      --issuer "$APPLE_API_ISSUER" --wait --timeout 30m
    rm -f "$key"
    xcrun stapler staple "$app"
    spctl --assess --type execute --verbose=2 "$app"
    echo "Notariée par Apple et agrafée : elle s'ouvre d'un double clic."
  else
    echo "Pas de clé d'API App Store Connect : signée, mais non notariée." >&2
  fi
  security delete-keychain "$keychain"
else
  codesign --force --sign - "$app"
  echo "Signature ad hoc (pas de certificat Developer ID dans MACOS_CERTIFICATE)." >&2
fi

zip_app
echo "→ $app ($version) et $out/LUMA-macos.zip"
