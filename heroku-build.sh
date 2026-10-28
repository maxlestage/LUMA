#!/usr/bin/env bash
# Construit le serveur et les îlots (WebAssembly), comme le Dockerfile : lancé sur
# Heroku par RustConfig une fois Rust installé par le buildpack emk/rust. Marche aussi à
# la main, depuis le dépôt : ./heroku-build.sh
# Le Procfile démarre site/target/release/luma-site.
set -euo pipefail

# Paquets et ses paquets sont des dépôts privés : Cargo les récupère par git, qui
# demande ses identifiants GitHub à un assistant lisant la variable REPOS_TOKEN, un jeton
# GitHub qui peut les lire (le buildpack transmet les variables dans ENV_DIR). Le jeton
# n'apparaît dans aucune URL.
if [ -z "${REPOS_TOKEN:-}" ] && [ -n "${ENV_DIR:-}" ] && [ -f "$ENV_DIR/REPOS_TOKEN" ]; then
  REPOS_TOKEN=$(<"$ENV_DIR/REPOS_TOKEN")
fi
if [ -n "${REPOS_TOKEN:-}" ]; then
  export REPOS_TOKEN CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=credential.https://github.com.helper \
    GIT_CONFIG_VALUE_0='!f() { test "$1" = get && echo username=x-access-token && echo "password=$REPOS_TOKEN"; }; f'
else
  echo "       pas de variable REPOS_TOKEN : les dépendances git privées ne pourront pas être récupérées"
fi
cd "$(dirname "$0")/site"
# Sur Heroku, Cargo compile dans le cache du buildpack (CARGO_TARGET_DIR).
target="${CARGO_TARGET_DIR:-target}"

echo "-----> Construction du serveur"
cargo build --release --bin luma-site
mkdir -p target/release
if [ "$target" != target ]; then
  cp "$target/release/luma-site" target/release/
fi
echo "       target/release/luma-site prêt"

echo "-----> Construction des îlots (WebAssembly)"
rustup target add wasm32-unknown-unknown
cargo build --lib --target wasm32-unknown-unknown --profile wasm-release

# wasm-bindgen doit être la version de site/Cargo.lock : un binaire prêt, ou compilé
# s'il ne peut pas être téléchargé.
version=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')
bin="$target/wasm-bindgen-$version"
if [ ! -x "$bin/wasm-bindgen" ]; then
  mkdir -p "$bin"
  curl -sSfL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${version}/wasm-bindgen-${version}-x86_64-unknown-linux-musl.tar.gz" \
    | tar -xz --strip-components=1 -C "$bin" \
    || cargo install wasm-bindgen-cli --version "$version" --locked --root "$bin" --bin wasm-bindgen
  [ -x "$bin/wasm-bindgen" ] || mv "$bin/bin/wasm-bindgen" "$bin/"
fi
"$bin/wasm-bindgen" --target web --no-typescript --out-dir pkg \
  "$target/wasm32-unknown-unknown/wasm-release/luma_site.wasm"
echo "       îlots prêts dans site/pkg"

# Les apps à télécharger (boutons « Télécharger pour Mac », « pour Linux » et « pour
# Windows ») : le dépôt est privé, donc ses Releases aussi ; les fichiers de la dernière
# Release sont récupérés avec le même jeton (qui doit pouvoir lire le dépôt LUMA) et
# servis par le site sous /download/.
echo "-----> Les apps à télécharger (dernière Release)"
repo="${LUMA_REPO:-maxlestage/LUMA}"
mkdir -p public/download
# Avec un jeton si le dépôt est privé ; sans, si les Releases sont publiques.
auth=()
[ -n "${REPOS_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $REPOS_TOKEN")
api="https://api.github.com/repos/$repo/releases/latest"
release=$(curl -sSf "${auth[@]}" -H "Accept: application/vnd.github+json" "$api") || release=""
# Windows : ses fichiers sont facultatifs ; sans eux, le site affiche « Bientôt ».
for name in LUMA-macos.zip LUMA-linux-x86_64.tar.gz LUMA-linux-amd64.deb \
            LUMA-windows-x86_64-setup.exe LUMA-windows-x86_64.zip; do
  # L'identifiant de l'asset : la ligne "url" de l'asset précède son nom.
  asset=$(printf '%s' "$release" | tr ',' '\n' | grep -E '"(url|name)"' \
    | awk -v n="$name" '
        match($0, /"url": *"[^"]*"/) { url = substr($0, RSTART, RLENGTH); sub(/^"url": *"/, "", url); sub(/"$/, "", url) }
        $0 ~ "\"name\": *\"" n "\"" { print url; exit }') || asset=""
  if [ -n "$asset" ] && curl -sSfL "${auth[@]}" -H "Accept: application/octet-stream" \
      -o "public/download/$name" "$asset"; then
    echo "       public/download/$name ($(du -h "public/download/$name" | cut -f1))"
  elif [[ "$name" == LUMA-windows-* ]]; then
    echo "       $name absent de la dernière Release : Windows reste « Bientôt » sur le site"
  else
    echo "       $name absent de la dernière Release (ou dépôt privé sans REPOS_TOKEN) : son bouton mènera à une page introuvable"
  fi
done
