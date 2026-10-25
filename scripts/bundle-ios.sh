#!/usr/bin/env bash
# Fabrique LÜMA.app pour iPhone et iPad. À lancer sur un Mac (Xcode installé) :
#
#   scripts/bundle-ios.sh sim      # simulateur (Apple Silicon), signé ad hoc
#   scripts/bundle-ios.sh device   # appareil : à signer avec un compte développeur
#                                  # Apple (codesign / Xcode), non signé ici
#
# Résultat : target/ios/<sim|device>/LÜMA.app et LUMA-ios-<sim|device>.zip.
set -euo pipefail
cd "$(dirname "$0")/.."

kind=${1:-sim}
case "$kind" in
  sim) target=aarch64-apple-ios-sim; platform=iPhoneSimulator ;;
  device) target=aarch64-apple-ios; platform=iPhoneOS ;;
  *) echo "usage : $0 sim|device" >&2; exit 2 ;;
esac
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
out="target/ios/$kind"
app="$out/LÜMA.app"

rustup target add "$target" >/dev/null
IPHONEOS_DEPLOYMENT_TARGET=16.0 cargo build --release -p luma-studio --target "$target"

rm -rf "$app"
mkdir -p "$app"
cp "target/$target/release/luma-studio" "$app/luma-studio"
sed -e "s/@VERSION@/$version/g" -e "s/@PLATFORM@/$platform/g" packaging/ios/Info.plist > "$app/Info.plist"

# Les icônes : dessinées par le code, aux tailles de l'iPhone et de l'iPad.
cargo run -q --release -p luma-studio --example icon -- "$out/icon.png"
for spec in 60x60@2x:120 60x60@3x:180 76x76:76 76x76@2x:152; do
  name=${spec%%:*}; px=${spec##*:}
  sips -z "$px" "$px" "$out/icon.png" --out "$app/AppIcon$name.png" >/dev/null
done

if [ "$kind" = sim ]; then
  codesign --force --sign - "$app"
fi
(cd "$out" && rm -f "LUMA-ios-$kind.zip" && ditto -c -k --keepParent "LÜMA.app" "LUMA-ios-$kind.zip")
echo "→ $app ($version, $platform)"
