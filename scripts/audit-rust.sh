#!/usr/bin/env bash
# La règle « 100 % Rust », vérifiée sur le graphe de dépendances réel (y compris les
# dépendances de construction) :
#
# 1. Le moteur (crates/luma-*) : aucune exception. Pas de crate de liaison vers une
#    bibliothèque native (-sys), pas de compilateur C (cc, cmake), pas de bindgen.
#    Seul luma-gpu, qui parle au pilote graphique (Metal, Vulkan), suit la règle 2.
# 2. L'application (apps/studio) et luma-gpu, sur macOS (la cible prioritaire) et sur Linux : seules
#    les liaisons vers des composants du système d'exploitation sont admises, une par
#    une, chacune justifiée ci‑dessous. Aucune ne compile de C.
set -euo pipefail
cd "$(dirname "$0")/.."

banned='^([A-Za-z0-9_-]+-sys|cc|cmake|bindgen|nasm-rs) v'

# Liaisons vers le système, admises pour l'application seulement :
system_bindings=(
  core-foundation-sys  # macOS : CoreFoundation d'Apple
  objc-sys             # macOS : le runtime Objective‑C d'Apple (fenêtres, Metal)
  linux-raw-sys        # Linux : définitions d'appels système, en Rust pur
  renderdoc-sys        # Linux : déclarations de l'API de capture RenderDoc (rien n'est compilé)
)
allowed="^($(IFS='|'; echo "${system_bindings[*]}")) v"

tree() { cargo tree "$@" -e normal,build --prefix none --format '{p}' | sort -u; }

status=0
engine=$(cargo metadata --no-deps --format-version 1 | grep -o '"name":"luma-[a-z]*"' | cut -d'"' -f4 | grep -v -e luma-studio -e luma-gpu | sed 's/^/-p /' | tr '\n' ' ')
# shellcheck disable=SC2086
found=$(tree $engine | grep -E "$banned" || true)
if [ -n "$found" ]; then
  echo "Moteur : dépendances interdites :"; echo "$found"; status=1
else
  # shellcheck disable=SC2086
  echo "Moteur : 100 % Rust, $(tree $engine | grep -c ' v') crates, aucune liaison native."
fi

for target in aarch64-apple-darwin x86_64-unknown-linux-gnu; do
  found=$(tree --workspace --target "$target" | grep -E "$banned" | grep -vE "$allowed" || true)
  if [ -n "$found" ]; then
    echo "Application ($target) : dépendances interdites :"; echo "$found"; status=1
  else
    used=$(tree --workspace --target "$target" | { grep -E "$allowed" || true; } | cut -d' ' -f1 | tr '\n' ' ')
    echo "Application ($target) : $(tree --workspace --target "$target" | grep -c ' v') crates, liaisons système seulement : ${used:-aucune}"
  fi
done
exit $status
