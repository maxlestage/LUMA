# LÜMA

**La lumière, développée en Rust.** Un éditeur photo professionnel écrit en Rust : RAW,
couleur 32 bits flottants, masques, IA locale, photothèque. **macOS et iOS/iPadOS d'abord**,
puis Windows, Linux et Android sur le même moteur.

- [Architecture](docs/architecture.md) : couches, workspace Cargo, pipeline non destructif, règles « 100 % Rust »
- [Feuille de route](docs/roadmap.md) : phases M0 → M36, Apple d'abord, jalons et risques
- [Site vitrine](site/) : animations, scènes 3D (objectif photo, Mac et iPhone), labo de retouche sur GPU

## Le site

Un site statique, sans étape de build : HTML, CSS et modules JavaScript, avec
[three.js](https://threejs.org) r170 copié dans `site/vendor/` (aucun CDN pour les scripts).

```sh
cd site
python3 -m http.server 8000   # puis http://localhost:8000
```

Les modules ES ne se chargent pas depuis `file://` : il faut un serveur, n'importe lequel.
Le dossier `site/` se dépose tel quel sur GitHub Pages, Netlify, Cloudflare Pages ou Heroku.

| Fichier | Rôle |
|---|---|
| `site/index.html` | Contenu de la page |
| `site/styles.css` | Thème, mises en page, animations CSS, versions tablette et téléphone |
| `site/js/main.js` | Point d'entrée, chargement différé des scènes 3D |
| `site/js/motion.js` | Préchargement, révélations, curseur, inclinaison 3D, sections collantes |
| `site/js/hero.js` | Objectif 3D : diaphragme à 9 lamelles, lentille irisée, flux de lumière |
| `site/js/devices.js` | MacBook qui s'ouvre et iPhone qui se retourne au défilement |
| `site/js/lab.js` | Labo WebGL : photo HDR procédurale, réglages, comparaison avant/après |
| `site/js/screen.js` | Photo procédurale et écrans de l'application dessinés au canvas |

Pensé d'abord pour Safari sur Mac, iPhone et iPad : `100svh`, zones sûres (`env(safe-area-inset-*)`),
densité de pixels plafonnée à 2, moins de particules sur téléphone, rendu à la demande pour la
scène des appareils, et respect de « Réduire les animations ». Sans WebGL, la page reste lisible.
