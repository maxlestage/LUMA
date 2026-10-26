# Améliorations

Le journal des améliorations de LÜMA, une par heure : ce qui est prévu, les idées en
attente, et ce qui est fait, avec les chiffres mesurés avant et après.

Règles : moteur 100 % Rust (`scripts/audit-rust.sh`), aucune IA, `unsafe` seulement dans
`crates/luma-apple`, tout texte visible en français, anglais et espagnol, des tests pour
chaque changement, rendu identique quand on accélère.

## Feuille de route du propriétaire

Dans cet ordre (« je veux tout », puis « hyper complet » : fonctions 7 à 16) ; chaque
étape tient en une heure, en complétant l'existant plutôt qu'en le refaisant. Une heure sur trois environ : un gain
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

### 7. Tri et sélection

- [ ] 7.1 Tri en plein écran au clavier (notes, drapeaux, libellés, suivante).
- [ ] 7.2 Comparaison côte à côte puis grille de 2 à 4 photos, zoom et déplacement
  synchronisés ; loupe.
- [ ] 7.3 Empilement des rafales et des séries d'exposition (date, réglages).
- [ ] 7.4 Doublons par empreinte d'image ; notes, drapeaux et libellés en lot.

### 8. Synchronisation

- [ ] 8.1 Copier‑coller sélectif des réglages (choix des panneaux).
- [ ] 8.2 Synchroniser une sélection ; réglages automatiques par lot.
- [ ] 8.3 Réglages par défaut par boîtier et par ISO ; préréglages d'import.

### 9. Retouche locale

- [ ] 9.1 Tampon (clonage) et correcteur (réparation par diffusion), dans le XMP.
- [ ] 9.2 Taches de capteur : visualisation des poussières, retrait.
- [ ] 9.3 Yeux rouges.
- [ ] 9.4 Moiré et aberration chromatique locale au pinceau.

### 10. Outils de développement avancés

Vérifier d'abord l'existant (courbes paramétrique et par canal, étalonnage, mélange noir
et blanc, vignetage après recadrage et grain existent).

- [ ] 10.1 Mélangeur de couches ; virage partiel.
- [ ] 10.2 Guides de recadrage : tiers, nombre d'or, diagonales.
- [ ] 10.3 Redressement automatique par l'horizon (transformée de Hough).
- [ ] 10.4 Perspective automatique par les lignes de fuite.

### 11. Qualité d'image

- [ ] 11.1 Dématriçage de haute qualité (directionnel, maison, en Rust).
- [ ] 11.2 Réduction du bruit avancée (moyennes non locales ou ondelettes).
- [ ] 11.3 Accentuation par déconvolution.
- [ ] 11.4 Hautes lumières reconstruites à partir des canaux non saturés.
- [ ] 11.5 Fusion HDR alignée, sans fantômes ; empilement de mise au point.

### 12. Couleur professionnelle

- [ ] 12.1 Profil de boîtier créé depuis une mire (`luma colorchecker` jusqu'au profil).
- [ ] 12.2 Correspondance des couleurs entre deux boîtiers.
- [ ] 12.3 Vectorscope, forme d'onde, parade RVB.
- [ ] 12.4 Mesure au point (RVB, Lab), écrêtage par canal.

### 13. Métadonnées et organisation

- [ ] 13.1 IPTC/XMP complet : titre, légende, mots‑clés hiérarchiques, lieu, auteur,
  droits ; modèles de métadonnées.
- [ ] 13.2 Carte des photos géolocalisées (tuiles libres).
- [ ] 13.3 Renommage par lot. (Pas de détection des visages : ce serait de l'IA.)

### 14. Sortie

- [ ] 14.1 Impression : mise en page, marges, planche contact, profil de l'imprimante.
- [ ] 14.2 Diaporama exportable ; livre photo simple en PDF.
- [ ] 14.3 Galerie web statique ; fichiers optimisés pour le web (WebP, AVIF, JPEG XL).

### 15. Flux de travail

- [ ] 15.1 Historique illimité, instantanés nommés ; copies virtuelles comparées.
- [ ] 15.2 Sauvegarde et restauration du catalogue ; fichiers manquants ou déplacés.
- [ ] 15.3 Import depuis une carte mémoire, renommage, copie sur un second disque.
- [ ] 15.4 Raccourcis personnalisables, espaces de travail.

### 16. Vidéo de base

- [ ] 16.1 Si possible sans dépendance native : un film ou préréglage appliqué à une
  vidéo courte, export.

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
| 2026-10-26 | Vignettes instantanées : l'aperçu intégré au RAW (lecture du début du fichier, JPEG décodé réduit, orientation TIFF), puis le vrai rendu ; vignettes rendues enregistrées sans perte (QOI) dans le dossier des doublons, refaites seulement si la photo, ses réglages ou le moteur changent | Mac (CI, Metal, 5 RAW) : vignette affichée en 15 ms (médiane) au lieu de 154 ms ; vignette déjà rendue relue en 0,6 ms au lieu d'environ 460 ms (Linux), identique au pixel près |
| 2026-10-26 | Doublon de travail : un niveau à la taille exacte de l'aperçu (2048 px) quand la réduction par deux passerait dessous (photos de 2048 à 4096 px, comme celles de l'iPhone) | iPhone ProRAW 3024 × 4032 : ouverture depuis le doublon 487 → 337 ms (Linux, mesure) ; sur Mac elle prenait 1171 ms, plus que depuis le RAW (346 ms) |
