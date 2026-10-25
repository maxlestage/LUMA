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

# L'app à télécharger (bouton « Télécharger pour Mac ») : le dépôt est privé, donc ses
# Releases aussi ; LUMA-macos.zip de la dernière Release est récupéré avec le même jeton
# (qui doit pouvoir lire le dépôt LUMA) et servi par le site sous /download/.
echo "-----> L'app à télécharger (dernière Release)"
repo="${LUMA_REPO:-maxlestage/LUMA}"
mkdir -p public/download
if [ -n "${REPOS_TOKEN:-}" ]; then
  api="https://api.github.com/repos/$repo/releases/latest"
  # L'identifiant de l'asset LUMA-macos.zip : la ligne "url" de l'asset précède son nom.
  asset=$(curl -sSf -H "Authorization: Bearer $REPOS_TOKEN" -H "Accept: application/vnd.github+json" "$api" \
    | tr ',' '\n' | grep -E '"(url|name)"' \
    | awk -F'"' '/"url"/ { url = $4 } /"name": *"LUMA-macos.zip"|"name":"LUMA-macos.zip"/ { print url; exit }') || asset=""
  if [ -n "$asset" ] && curl -sSfL -H "Authorization: Bearer $REPOS_TOKEN" -H "Accept: application/octet-stream" \
      -o public/download/LUMA-macos.zip "$asset"; then
    echo "       public/download/LUMA-macos.zip ($(du -h public/download/LUMA-macos.zip | cut -f1))"
  else
    echo "       pas de Release lisible : le bouton de téléchargement mènera à une page introuvable"
  fi
else
  echo "       pas de REPOS_TOKEN : pas de téléchargement"
fi
