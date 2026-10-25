#!/usr/bin/env bash
# La règle « 100 % Rust » du moteur, vérifiée sur le graphe de dépendances réel :
# aucune crate de liaison vers une bibliothèque native (-sys), aucun compilateur C
# (cc, cmake) ni générateur de liaisons (bindgen) dans les dépendances, y compris
# celles de construction.
set -euo pipefail
cd "$(dirname "$0")/.."
tree=$(cargo tree --workspace -e normal,build --prefix none --format '{p}' | sort -u)
banned=$(echo "$tree" | grep -E '^([A-Za-z0-9_-]+-sys|cc|cmake|bindgen|pkg-config|nasm-rs) v' || true)
count=$(echo "$tree" | grep -c ' v' || true)
if [ -n "$banned" ]; then
  echo "Dépendances interdites :"
  echo "$banned"
  exit 1
fi
echo "100 % Rust : $count crates dans le graphe, aucune liaison native ni compilateur C."
