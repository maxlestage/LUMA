# Améliorations

Le journal des améliorations de LÜMA, une par heure : ce qui est prévu, les idées en
attente, et ce qui est fait, avec les chiffres mesurés avant et après.

Règles : moteur 100 % Rust (`scripts/audit-rust.sh`), aucune IA, `unsafe` seulement dans
`crates/luma-apple`, tout texte visible en français, anglais et espagnol, des tests pour
chaque changement, rendu identique quand on accélère.

## Feuille de route du propriétaire

Dans cet ordre ; chaque étape tient en une heure. Une heure sur trois environ : un gain
mesuré de vitesse ou de légèreté (ouverture, aperçu, vignettes, export, mémoire, taille
du binaire, démarrage, place des doublons de travail), mesuré avec
`cargo run --release -p luma-gpu --example measure -- tests/raw-corpus/*`.

### 1. Photothèque professionnelle

- [x] 1.1 Crate `luma-library` : catalogue local (index en Rust pur dans le dossier de
  LÜMA), en‑têtes lus sans décoder (date EXIF, boîtier, objectif, ISO, ouverture,
  vitesse, focale), note, drapeau, libellé, mots‑clés du XMP ; mise à jour
  incrémentale ; tri par date de prise de vue ; `luma catalog`.
- [ ] 1.2 Requêtes : recherche texte (nom, boîtier, objectif, mots‑clés) et filtres
  (note, drapeau, libellé, plages de date, d'ISO, de focale) dans `luma-library`, testés.
- [ ] 1.3 Collections manuelles et dynamiques (règles : note, date, boîtier, objectif,
  ISO), enregistrées dans le catalogue ; `luma catalog --collection`.
- [ ] 1.4 Mots‑clés dans l'app : saisie, ajout à plusieurs photos, liste des mots‑clés
  du catalogue avec leur nombre.
- [ ] 1.5 L'app ouvre le catalogue : panneau Photothèque (dossiers, collections),
  grille triée par date, recherche et filtres.
- [ ] 1.6 Copies virtuelles : plusieurs jeux de réglages pour un même fichier, dans le
  XMP, visibles dans la grille et le film.
- [ ] 1.7 Index plus léger : format binaire compact, chargement de 100 000 photos mesuré.

### 2. Export professionnel

- [ ] 2.1 Préréglages d'export enregistrés (format, qualité, espace, taille, dossier),
  dans le moteur (`luma-render::export`) et la ligne de commande.
- [ ] 2.2 Modèles de nom : `{date}`, `{nom}`, `{séquence}`, `{boîtier}`, `{note}`…
- [ ] 2.3 Redimensionnement : grand côté, petit côté, largeur × hauteur, mégapixels.
- [ ] 2.4 Accentuation de sortie : écran, papier mat, papier brillant ; faible, normale,
  forte.
- [ ] 2.5 Filigrane texte (police embarquée en Rust) puis logo (PNG), position, taille,
  opacité.
- [ ] 2.6 Métadonnées : EXIF d'origine recopié, auteur, copyright, IPTC, mots‑clés,
  retrait du GPS.
- [ ] 2.7 Export par lot vers plusieurs préréglages à la fois, dans l'app et en ligne de
  commande.

### 3. Corrections d'objectif automatiques

- [ ] 3.1 DNG : opcodes `WarpRectilinear` et `FixVignetteRadial` (OpcodeList1‑3) lus et
  appliqués.
- [ ] 3.2 Sony : distorsion, vignetage et aberration chromatique des makernotes ARW.
- [ ] 3.3 Fujifilm : données de correction du RAF.
- [ ] 3.4 Olympus/OM et Panasonic : distorsion et vignetage des makernotes.
- [ ] 3.5 Application automatique à l'ouverture, case « Corrections du boîtier »,
  réglage manuel en secours ; XMP.

### 4. Masques avancés, sans IA

- [ ] 4.1 Plage de luminance (bornes, adoucissement), sur le GPU aussi.
- [ ] 4.2 Plage de couleur (pipette, tolérance en OKLab).
- [ ] 4.3 Pinceau qui suit les bords (filtre guidé).
- [ ] 4.4 Combinaisons : ajouter, soustraire, intersection.
- [ ] 4.5 Affichage du masque en surimpression (couleur, opacité) dans l'app.

### 5. Épreuvage et impression

- [ ] 5.1 Lecture des profils ICC de sortie (matrices et tables LUT), en Rust pur.
- [ ] 5.2 Épreuvage à l'écran : intention perceptuelle et colorimétrique relative.
- [ ] 5.3 Alerte de gamut.
- [ ] 5.4 Simulation du papier et de l'encre noire.

### 6. Prise de vue en connexion

- [ ] 6.1 Dossier surveillé : les nouvelles photos sont importées et affichées.
- [ ] 6.2 Mac : connexion directe aux boîtiers par le framework du système, dans
  `crates/luma-apple` seulement.

## Idées

- Les textes de présentation parlent encore d'« IA locale » (README, architecture,
  feuille de route) : à retirer, LÜMA n'utilise aucune IA.
- Les images développées (JPEG, HEIC…) : lire leur EXIF pour la date de prise de vue,
  le boîtier et l'objectif (aujourd'hui, date du fichier).
- Doublons dans le catalogue (même date, même boîtier, même taille).
- Vignettes du catalogue en cache sur disque, partagées entre la grille et le film.
- La suppression d'une branche distante par `git push --delete` est refusée par le
  proxy de cette session : les branches `amelioration/*` restent sur le dépôt.

## Fait

| Date | Quoi | Mesures |
|---|---|---|
| 2026-10-26 | 1.1 Catalogue local (`luma-library`, `luma catalog`) ; mots‑clés XMP `dc:subject` lus et écrits ; entités XML (`&amp;`…) bien relues dans les listes XMP | Corpus de 5 RAW : indexé en 94 ms (en‑têtes seuls, 4 cœurs) contre environ 300 ms pour décoder un seul RAW ; mise à jour sans changement : moins de 1 ms ; index de 1,6 Ko |
