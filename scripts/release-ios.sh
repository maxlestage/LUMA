#!/usr/bin/env bash
# Publie LÜMA pour iPhone et iPad sur TestFlight (puis l'App Store), depuis la CI macOS :
# aucun Mac à soi n'est nécessaire. Tout passe par l'API App Store Connect :
#
#   1. l'identifiant de l'app (app.luma.studio), créé s'il manque ;
#   2. un certificat de distribution neuf (sa clé est générée ici et ne sort pas de la
#      machine) et un profil App Store ;
#   3. l'app compilée (scripts/bundle-ios.sh device), son icône App Store (catalogue
#      d'images), signée, mise en .ipa et envoyée ;
#   4. le certificat et le profil retirés à la fin (il n'en faut pas d'autre).
#
# Environnement : ASC_KEY_ID, ASC_ISSUER_ID, ASC_KEY (contenu du .p8), APPLE_TEAM_ID,
# BUILD_NUMBER (numéro de build, croissant), LUMA_UPLOAD=0 pour ne pas envoyer.
set -euo pipefail
cd "$(dirname "$0")/.."

: "${ASC_KEY_ID:?clé API App Store Connect manquante (ASC_KEY_ID)}"
: "${ASC_ISSUER_ID:?ASC_ISSUER_ID manquant}"
: "${ASC_KEY:?ASC_KEY manquant (contenu du fichier .p8)}"
: "${APPLE_TEAM_ID:?APPLE_TEAM_ID manquant}"
# Les secrets tels qu'on les a collés : sans espaces ni retours à la ligne autour.
ASC_KEY_ID=$(printf '%s' "$ASC_KEY_ID" | tr -d ' \r\n')
ASC_ISSUER_ID=$(printf '%s' "$ASC_ISSUER_ID" | tr -d ' \r\n')
export ASC_KEY_ID ASC_ISSUER_ID
BUILD_NUMBER=${BUILD_NUMBER:-$(date +%s)}
BUNDLE=app.luma.studio
asc() { python3 scripts/asc.py "$@"; }

work=$(mktemp -d)
cert_id=""
profile_id=""
keychain="$work/luma.keychain-db"
cleanup() {
  [ -n "$profile_id" ] && asc delete profiles "$profile_id" || true
  [ -n "$cert_id" ] && asc delete certificates "$cert_id" || true
  security delete-keychain "$keychain" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

echo "→ Identifiant de l'app"
bundle_res=$(asc bundle-id "$BUNDLE" LUMA)

echo "→ Certificat de distribution et profil"
# LibreSSL du système : son PKCS#12 est celui que le trousseau de macOS sait lire.
/usr/bin/openssl genrsa -out "$work/key.pem" 2048 2>/dev/null
/usr/bin/openssl req -new -key "$work/key.pem" -out "$work/demande.csr" \
  -subj "/CN=LUMA Distribution/O=Maxime Lestage/C=FR"
cert_id=$(asc certificate "$work/demande.csr" "$work/distribution.cer")
profile_id=$(asc profile "$bundle_res" "$cert_id" "$work/profil.mobileprovision")
/usr/bin/openssl x509 -inform DER -in "$work/distribution.cer" -out "$work/distribution.pem"
pass=$(/usr/bin/openssl rand -hex 16)
/usr/bin/openssl pkcs12 -export -inkey "$work/key.pem" -in "$work/distribution.pem" \
  -out "$work/distribution.p12" -passout "pass:$pass"

security create-keychain -p "$pass" "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p "$pass" "$keychain"
curl -fsSL https://www.apple.com/certificateauthority/AppleWWDRCAG3.cer -o "$work/wwdr.cer"
security import "$work/wwdr.cer" -k "$keychain" -T /usr/bin/codesign >/dev/null
security import "$work/distribution.p12" -k "$keychain" -P "$pass" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple: -s -k "$pass" "$keychain" >/dev/null
security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
identity=$(security find-identity -v -p codesigning "$keychain" | awk '/Apple Distribution|iPhone Distribution/ {print $2; exit}')
[ -n "$identity" ] || { echo "certificat de distribution introuvable dans le trousseau" >&2; exit 1; }

echo "→ LÜMA pour iPhone et iPad"
scripts/bundle-ios.sh device
app="target/ios/device/LÜMA.app"
plist="$app/Info.plist"
pb() { /usr/libexec/PlistBuddy "$@"; }

# Le numéro de build, et ce que Xcode écrit d'ordinaire dans l'Info.plist.
sdk_version=$(xcrun --sdk iphoneos --show-sdk-version)
sdk_build=$(xcrun --sdk iphoneos --show-sdk-build-version)
xcode_version=$(xcodebuild -version | awk 'NR==1 {print $2}')
xcode_build=$(xcodebuild -version | awk 'NR==2 {print $3}')
xcode_number=$(printf '%s' "$xcode_version" | awk -F. '{printf "%d%d%d0", $1, $2, $3}')
pb -c "Set :CFBundleVersion $BUILD_NUMBER" "$plist"
for kv in "CFBundleInfoDictionaryVersion:6.0" "DTPlatformName:iphoneos" \
  "DTPlatformVersion:$sdk_version" "DTSDKName:iphoneos$sdk_version" "DTSDKBuild:$sdk_build" \
  "DTPlatformBuild:$sdk_build" "DTXcode:$xcode_number" "DTXcodeBuild:$xcode_build" \
  "DTCompiler:com.apple.compilers.llvm.clang.1_0" "BuildMachineOSBuild:$(sw_vers -buildVersion)"; do
  pb -c "Delete :${kv%%:*}" "$plist" 2>/dev/null || true
  pb -c "Add :${kv%%:*} string ${kv#*:}" "$plist"
done

# L'icône App Store (1024 px, opaque) dans un catalogue d'images compilé (Assets.car).
icons="$work/LUMA.xcassets/AppIcon.appiconset"
mkdir -p "$icons"
cargo run -q --release -p luma-studio --example icon -- "$work/icon-rgba.png" ios 1024
sips -s format jpeg "$work/icon-rgba.png" --out "$work/icon.jpg" >/dev/null
sips -s format png "$work/icon.jpg" --out "$icons/icon-1024.png" >/dev/null
cat > "$icons/Contents.json" <<'JSON'
{"images":[{"filename":"icon-1024.png","idiom":"universal","platform":"ios","size":"1024x1024"}],
 "info":{"author":"xcode","version":1}}
JSON
echo '{"info":{"author":"xcode","version":1}}' > "$work/LUMA.xcassets/Contents.json"
xcrun actool "$work/LUMA.xcassets" --compile "$app" --platform iphoneos \
  --minimum-deployment-target 16.0 --app-icon AppIcon --target-device iphone --target-device ipad \
  --output-partial-info-plist "$work/icones.plist" >/dev/null
pb -c "Delete :CFBundleIcons" "$plist" 2>/dev/null || true
pb -c "Delete :CFBundleIcons~ipad" "$plist" 2>/dev/null || true
pb -c "Merge $work/icones.plist" "$plist"

echo "→ Signature"
cp "$work/profil.mobileprovision" "$app/embedded.mobileprovision"
security cms -D -i "$work/profil.mobileprovision" > "$work/profil.plist"
pb -x -c "Print :Entitlements" "$work/profil.plist" > "$work/droits.plist"
codesign --force --sign "$identity" --keychain "$keychain" \
  --entitlements "$work/droits.plist" --generate-entitlement-der "$app"
codesign --verify --deep --strict "$app"

echo "→ LUMA.ipa"
out="target/ios/app-store"
rm -rf "$out" && mkdir -p "$out/Payload"
cp -R "$app" "$out/Payload/"
(cd "$out" && ditto -c -k --sequesterRsrc --keepParent Payload LUMA.ipa)

if [ "${LUMA_UPLOAD:-1}" = 1 ]; then
  # La fiche de l'app (créée à la main dans App Store Connect) n'est vérifiée qu'ici :
  # tout le reste est déjà construit et signé.
  asc app "$BUNDLE" >/dev/null
  echo "→ Envoi à App Store Connect (build $BUILD_NUMBER)"
  mkdir -p ~/.appstoreconnect/private_keys
  printf '%s\n' "$ASC_KEY" > ~/.appstoreconnect/private_keys/"AuthKey_$ASC_KEY_ID.p8"
  xcrun altool --upload-app -f "$out/LUMA.ipa" -t ios --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
fi
echo "→ $out/LUMA.ipa (build $BUILD_NUMBER)"
