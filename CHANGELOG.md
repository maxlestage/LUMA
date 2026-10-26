# Nouveautés

## 0.3.0 — à venir

La photothèque professionnelle, étape par étape.

- **Plus rapide** : la réduction du bruit de couleur (active par défaut) trois fois
  plus rapide, sans changer un seul pixel ; export en pleine définition 25 à 30 %
  plus court, aperçu avec réglages lourds environ 25 % plus vif.

- **Catalogue local** : `luma catalog` ajoute des dossiers (sous‑dossiers compris) à la
  photothèque de LÜMA ; date de prise de vue, boîtier, objectif, ISO, ouverture,
  vitesse et focale sont lus dans l'en‑tête des RAW, sans décoder l'image (5 RAW en
  94 ms), puis seuls les fichiers modifiés sont relus (moins d'une milliseconde).
  Photos rangées par date de prise de vue.
- **Mots‑clés** : lus et écrits dans le XMP (`dc:subject`), comme le font tous les
  logiciels photo.
- **Recherche et filtres** : `luma catalog --search "mer note:3 date:2024-06 iso:..800
  boîtier:sony"` ; des mots (nom, boîtier, objectif, libellé, mots‑clés, sans tenir
  compte des accents) et des filtres (note, drapeau, libellé, dates, ISO, focale,
  boîtier, objectif, mot‑clé). 100 000 photos fouillées en 4 à 25 ms.
  `--keywords` compte les mots‑clés, boîtiers et objectifs.
- **Collections** manuelles (photos choisies, de n'importe quel dossier) et
  dynamiques (une règle écrite comme une recherche, « note:4 date:2024 boîtier:sony »,
  qui suit le catalogue) : `luma catalog --smart`, `--add`, `--collections`,
  `--collection`.
- **Mots‑clés dans l'app** (Bibliothèque) : on les tape (plusieurs à la fois, séparés
  par une virgule), ils vont à toutes les photos choisies (⌘‑clic, Maj‑clic) ; une
  croix en retire un. La liste des mots‑clés du dossier, avec leur nombre, filtre la
  grille d'un clic (majuscules et accents mis à part).
- **Catalogue léger** : un format binaire compact, où chaque dossier, boîtier,
  objectif et mot‑clé n'est écrit qu'une fois. Pour 100 000 photos : 7,4 Mo au lieu de
  33,6, relu en 68 ms au lieu de 166, enregistré en 121 ms au lieu de 332. L'ancien
  catalogue est relu et converti tout seul.
- **Préréglages d'export** : format, qualité, profondeur, espace, taille et dossier
  enregistrés sous un nom ; quatre de base (Web, Pleine définition, Écran P3,
  Impression) et les vôtres, partagés par l'app et la ligne de commande
  (`luma export-preset`, `luma develop --preset Web`).
- **Noms des fichiers exportés** : un modèle, `{date}_{séquence}` ou
  `{année}/{mois}/{boîtier}_{nom}`, avec la date de prise de vue, le boîtier,
  l'objectif, l'ISO, la note, le libellé et le rang dans l'export ; dans la fenêtre
  Exporter, les préréglages d'export et `luma develop --name`. Deux photos du même nom
  ne s'écrasent pas (`-2`, `-3`…).
- **Dimensions d'export** : grand côté, petit côté, largeur × hauteur (l'image tient
  dans le cadre) ou mégapixels, sans jamais agrandir ; dans la fenêtre Exporter, les
  préréglages et `luma develop --resize short:1080`.
- **Accentuation de sortie** : pour l'écran, le papier mat ou le papier brillant,
  faible, normale ou forte, appliquée à l'image à sa taille finale (sur papier, le
  rayon suit la taille du tirage) ; les aplats restent lisses.

## 0.2.0 — octobre 2026

Toutes les images, trois langues, et la boutique.

- **Toutes les images** : JPEG, PNG, TIFF, WebP, GIF, BMP, OpenEXR, HDR, JPEG XL, PSD,
  JPEG 2000, SVG… et HEIC, HEIF, AVIF de l'iPhone (Mac, iPhone et iPad) ; profil
  « Original » : l'image s'ouvre telle qu'elle est, tous les outils s'y appliquent.
- **Trois langues** : français, anglais, espagnol (app et site).
- **LÜMA Premium par Stripe** : la clé de licence s'affiche aussitôt après le paiement,
  vérifiée par l'app sans réseau, renouvelée chaque jour.
- **App Mac prête pour la signature Developer ID** et la notarisation d'Apple.

## 0.1.0 — octobre 2026

La première version à télécharger : 14 jours d'essai, puis LÜMA Premium.

- **Développement RAW** : un module Développement complet, panneau par
  panneau, moteur 32 bits flottants, finition sur le GPU (Metal), réglages en XMP.
- **735 boîtiers** reconnus, dont 714 à couleur étalonnée ; tout DNG, dont l'iPhone
  ProRAW.
- **100 films** vérifiés par la machine (pellicules couleur, cinéma, noir et blanc,
  instantanés) ; **vos vraies pellicules mesurées** sur un scan, en LUT 3D ; import de
  LUT `.cube`.
- **Masques** linéaire, radial et pinceau ; **films empilés** par masques, complets.
- **Empilements** (bruit, médiane, filé, ciel étoilé) et **panoramas**, en DNG.
- **Préréglages** : les vôtres, importés ou exportés en préréglages
  XMP.
- **Mac** (Apple Silicon et Intel), **iPhone et iPad** (en préparation pour l'App
  Store), ligne de commande `luma`.
