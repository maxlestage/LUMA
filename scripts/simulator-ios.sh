#!/usr/bin/env bash
# Essaie LÜMA pour iPhone dans le simulateur d'iOS, comme un utilisateur : l'app est
# installée sur un iPhone simulé dont Photos contient quelques images, puis on touche
# « Importer des photos », on choisit une photo et on l'ajoute. Les touchers passent
# par idb (de vrais événements tactiles). Chaque étape laisse une capture d'écran,
# avec le journal de l'app, dans target/ios/simulateur.
#
# À lancer sur un Mac (Xcode, idb-companion et fb-idb installés) ; la CI le fait
# (.github/workflows/ios-simulator.yml).
set -euo pipefail
cd "$(dirname "$0")/.."

out=target/ios/simulateur
rm -rf "$out" && mkdir -p "$out"
step=0
shot() {
  step=$((step + 1))
  xcrun simctl io "$udid" screenshot "$out/$(printf %02d "$step")-$1.png" >/dev/null 2>&1 || true
  echo "→ capture $step : $1"
}

scripts/bundle-ios.sh sim
app="target/ios/sim/LÜMA.app"

# Le plus récent des iPhone « Pro » disponibles.
udid=$(xcrun simctl list devices available -j | python3 -c '
import json, sys
devices = [d for runtime, ds in json.load(sys.stdin)["devices"].items() if "iOS" in runtime for d in ds]
phones = [d for d in devices if d["name"].startswith("iPhone")]
pro = [d for d in phones if "Pro" in d["name"]] or phones
print(pro[-1]["udid"])')
echo "→ simulateur $udid"
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null
open -a Simulator --args -CurrentDeviceUDID "$udid" || true
# En français, comme l'iPhone du propriétaire.
xcrun simctl spawn "$udid" defaults write -g AppleLanguages -array fr
xcrun simctl spawn "$udid" defaults write -g AppleLocale fr_FR

# Quelques images dans Photos.
cargo run -q --release -p luma-studio --example icon -- "$out/logo.png" ios 1024
for angle in 0 90 180; do
  sips -s format jpeg -r "$angle" "$out/logo.png" --out "$out/photo-$angle.jpg" >/dev/null
done
xcrun simctl addmedia "$udid" "$out"/photo-*.jpg
rm -f "$out"/photo-*.jpg "$out/logo.png"

xcrun simctl install "$udid" "$app"
xcrun simctl launch "$udid" app.luma.studio
sleep 20
shot accueil

data=$(xcrun simctl get_app_container "$udid" app.luma.studio data)
journal="$data/Documents/LÜMA-journal.txt"
read -r x y < <(sed -n 's/^bouton importer : centre \([0-9]*\) \([0-9]*\).*/\1 \2/p' "$journal" | tail -1) || true
if [ -z "${x:-}" ]; then
  echo "le bouton « Importer des photos » n'est pas affiché" >&2
else
  echo "→ toucher « Importer des photos » ($x, $y)"
  idb ui tap --udid "$udid" "$x" "$y"
  sleep 6
  shot apres-importer

  # Dans le sélecteur : la première photo, puis « Ajouter ».
  idb ui describe-all --udid "$udid" --json > "$out/ecran.json" || true
  tap_label() {
    python3 - "$out/ecran.json" "$@" <<'PY' || return 1
import json, sys
items = json.load(open(sys.argv[1]))
wanted = [w.lower() for w in sys.argv[2:]]
for it in items:
    label = (it.get("AXLabel") or "").lower()
    if any(w in label for w in wanted):
        f = it["frame"]
        print(f'{f["x"] + f["width"] / 2:.0f} {f["y"] + f["height"] / 2:.0f}')
        sys.exit(0)
sys.exit(1)
PY
  }
  if pos=$(tap_label "photo," "photo"); then
    echo "→ toucher la première photo ($pos)"
    idb ui tap --udid "$udid" $pos
    sleep 2
    shot photo-choisie
    idb ui describe-all --udid "$udid" --json > "$out/ecran.json" || true
    if pos=$(tap_label "ajouter" "add"); then
      echo "→ toucher « Ajouter » ($pos)"
      idb ui tap --udid "$udid" $pos
    fi
  fi
  sleep 10
  shot apres-import
fi

cp "$journal" "$out/journal.txt" 2>/dev/null || true
ls "$data/Documents" > "$out/documents.txt" 2>/dev/null || true
echo "--- journal de l'app"
cat "$out/journal.txt" 2>/dev/null || true
echo "--- Documents"
cat "$out/documents.txt" 2>/dev/null || true
