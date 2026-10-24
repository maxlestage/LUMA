# LÜMA

**La lumière, développée en Rust.** Un éditeur photo professionnel écrit en Rust : RAW,
couleur 32 bits flottants, masques, IA locale, photothèque. **macOS et iOS/iPadOS d'abord**,
puis Windows, Linux et Android sur le même moteur.

- [Architecture](docs/architecture.md) : couches, workspace Cargo, pipeline non destructif, règles « 100 % Rust »
- [Feuille de route](docs/roadmap.md) : phases M0 → M36, Apple d'abord, jalons et risques
- [Site vitrine](site/) : animations, scènes 3D (objectif photo, Mac et iPhone), labo de retouche sur GPU

## Le site

Le site est une app [Paquets](https://github.com/maxlestage/Paquets) : du Rust de bout en bout.

| Partie | Où | En quoi |
|---|---|---|
| Serveur, pages, SEO, CSP, PWA, export statique | `site/src/server/` | Rust (Paquets, natif) |
| Photo du pipeline, générée en SVG | `site/src/server/photo.rs` | Rust |
| Îlots : labo (réglages, préréglages) et filtre des crates | `site/src/islands.rs` | Rust, rendu serveur puis hydraté en WebAssembly |
| Animations de page : préchargement, révélations, curseur, inclinaison, compteurs, manifeste, pipeline 3D, feuille de route, terminal | `site/src/client/motion.rs` | Rust → WebAssembly |
| Rendu du labo (WebGL) | `site/src/client/lab.rs`, `lab.frag` | Rust → WebAssembly, shader GLSL |
| Textes et données | `site/src/content.rs` | Rust, partagé serveur/navigateur |
| Scènes 3D : objectif photo, MacBook et iPhone | `site/public/js/` | three.js r170, embarqué (aucun CDN) |
| Styles | `site/public/site.css` | CSS |

La page est du HTML fini rendu par le serveur : elle se lit sans JavaScript, avec ses
balises SEO, et sa Content-Security-Policy n'autorise que son propre code.

### Construire et lancer

Tout se lance depuis `site/` :

```sh
# 1. Le bundle du navigateur (îlots + animations) → pkg/
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
cargo build --lib --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir pkg \
  target/wasm32-unknown-unknown/wasm-release/luma_site.wasm

# 2. Le serveur, sur http://localhost:3000 (PORT sur Heroku)
cargo run --release

# Ou le site en fichiers statiques (GitHub Pages…), liens sous BASE_PATH
BASE_PATH=/LUMA cargo run --release -- export dist
```

Paquets et ses paquets sont des dépôts git privés : il faut un accès GitHub qui peut les lire.

### Vérifier

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings
cargo test
```

### Déployer

- **Heroku, buildpack** : `app.json`, `RustConfig`, `heroku-build.sh` et `Procfile`. Ajouter la
  variable `REPOS_TOKEN` (un jeton GitHub qui lit les dépôts privés).
- **Docker** : `docker build --build-arg REPOS_TOKEN=… -t luma .`, ou `heroku.yml` sur la stack container.

### Apple d'abord

Pensé d'abord pour Safari sur Mac, iPhone et iPad : installable sur l'écran d'accueil (PWA,
icônes, `apple-touch-icon`), `100svh`, zones sûres (`env(safe-area-inset-*)`), densité de
pixels plafonnée à 2, moins de particules sur téléphone, rendu à la demande pour la scène
des appareils, et respect de « Réduire les animations ».
