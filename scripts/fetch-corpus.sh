#!/usr/bin/env bash
# Télécharge le corpus de RAW de test dans tests/raw-corpus/ (non versionné).
# Les fichiers viennent de raw.pixls.us, la base d'échantillons RAW de la communauté
# (licence indiquée pour chaque fichier sur le site, CC0 pour la plupart).
# Un boîtier par famille de capteur et de format : Sony ARW, Canon CR3, Nikon NEF,
# Fujifilm RAF (X-Trans), Apple ProRAW (DNG linéaire).
set -euo pipefail
dir="$(cd "$(dirname "$0")/.." && pwd)/tests/raw-corpus"
mkdir -p "$dir"
files=(
  "Sony/ILCE-7M3/_DSC0009.ARW"
  "Canon/EOS R5/Canon_EOS_R5_CRAW_ISO_100_nocrop_dual.CR3"
  "Nikon/Z 6/DSC_0750.NEF"
  "Fujifilm/X-T3/AFXT2720.RAF"
  "Apple/iPhone 12 Pro/IMG_1361.DNG"
)
for f in "${files[@]}"; do
  name=$(basename "$f")
  if [ -s "$dir/$name" ]; then
    echo "  déjà là   $name"
    continue
  fi
  url="https://raw.pixls.us/data/${f// /%20}"
  for attempt in 1 2 3 4; do
    if curl -sSfL -o "$dir/$name.part" "$url"; then
      mv "$dir/$name.part" "$dir/$name"
      echo "  téléchargé $name ($(du -h "$dir/$name" | cut -f1))"
      break
    fi
    echo "  nouvel essai ($attempt) pour $name"
    sleep $((attempt * 2))
  done
  [ -s "$dir/$name" ] || { echo "échec : $url" >&2; exit 1; }
done
