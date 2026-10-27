# Nouveautés

## 0.3.0 — à venir

La photothèque professionnelle, étape par étape.

- **Plus rapide** : la réduction du bruit de couleur (active par défaut) trois fois
  plus rapide, sans changer un seul pixel ; export en pleine définition 25 à 30 %
  plus court, aperçu avec réglages lourds environ 25 % plus vif. Puis les flous :
  deux tampons au lieu de six images neuves, l'accentuation en pleine définition
  presque deux fois plus rapide (NEF 0,64 → 0,36 s), sans changer un pixel. Puis
  l'écriture des JPEG, sur tous les cœurs : 2,3 à 2,6 fois plus rapide, fichiers
  identiques octet pour octet ; un export pleine définition de 24 Mpx en 1,8 s au lieu
  de 2,4. Puis l'étape de rendu, calculée huit pixels à la fois : 1,8 à 1,9 fois plus
  rapide (NEF 160 → 85 ms), mêmes fichiers octet pour octet.

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
- **Case « Corrections du boîtier »** (Géométrie) : cochée par défaut ; décochée, la
  photo revient telle que l'objectif l'a donnée, et les curseurs de distorsion et de
  vignetage prennent le relais. Enregistrée dans le XMP.
- **Corrections d'objectif des OM System, Olympus et Panasonic** : la distorsion (et,
  pour Olympus et OM System, l'aberration chromatique latérale) que le boîtier
  enregistre dans chaque ORF ou RW2 est appliquée automatiquement. Le corpus de test
  compte désormais 7 vrais RAW, dont un OM‑5 Mark II et un Panasonic G9.
- **Corrections d'objectif des Fujifilm** : celles que le boîtier enregistre dans chaque
  RAF (distorsion, aberration chromatique latérale, vignetage), mode recadré compris,
  sont appliquées automatiquement ; par exemple le barillet du XF 35 mm F2.
- **Corrections d'objectif des Sony** : la distorsion, l'aberration chromatique
  latérale et le vignetage que le boîtier enregistre dans chaque ARW sont appliqués
  automatiquement, avec juste ce qu'il faut d'agrandissement pour ne laisser aucun bord
  vide.
- **Corrections d'objectif des DNG** : la distorsion, l'aberration chromatique
  latérale et le vignetage que le boîtier ou le logiciel a écrits dans un DNG (opcodes
  `WarpRectilinear` et `FixVignetteRadial`) sont appliqués automatiquement, à l'aperçu
  comme à l'export.
- **Photothèque plus rapide sur les disques externes** : pour ranger une photo, LÜMA
  ne lit plus que le début du fichier (0,5 à 2 Mo au lieu de 25 à 56 Mo). Cinq RAW du
  corpus : 34 Mo lus au lieu de 189 ; un NEF se range en 1,5 ms au lieu de 17 à 23,
  un RAF en 0,6 ms au lieu de 22 à 25 (lectures à froid).
- **Copies virtuelles** : plusieurs développements d'une même photo, sans dupliquer le
  fichier ; chaque copie a ses réglages, sa note, ses mots‑clés et sa vignette, à côté
  de son original, et s'exporte sous le nom `IMG_0001-2`. La supprimer ne touche jamais
  l'original.
- **La photothèque dans l'app** (Bibliothèque) : chaque dossier ouvert y entre, en
  arrière‑plan ; toutes les photos, les collections et les dossiers, chacun avec son
  nombre de photos, et une recherche (« mer note:3 date:2024 ») ; ces vues sont triées
  par date de prise de vue, et le film, la grille et les flèches les suivent.
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
- **Filigrane texte** : « © votre nom » dessiné avec la police de LÜMA, lissé au
  pixel près, à l'un des neuf emplacements, taille, opacité, blanc ou noir ; dans la
  fenêtre Exporter, les préréglages et `luma develop --watermark`.
- **Filigrane logo** : votre logo en PNG, avec sa transparence et ses vraies couleurs,
  à l'un des neuf emplacements, taille et opacité ; seul ou avec le texte ; dans la
  fenêtre Exporter, les préréglages et `luma develop --logo signature.png`.
- **Métadonnées d'export** : l'EXIF de la prise de vue (boîtier, objectif, réglages,
  date) recopié dans les JPEG et PNG, votre nom et votre copyright, les mots‑clés, la
  note et le libellé ; la position GPS est retirée par défaut, pour qu'une photo
  partagée ne dise pas où vous habitez (`--keep-gps` la garde). Les TIFF les reçoivent
  aussi (EXIF, XMP), avec exactement les mêmes pixels qu'avant.
- **Export vers plusieurs préréglages à la fois** : cochez « Web » et « Impression »
  dans la fenêtre Exporter, chaque photo part vers les deux en une fois ; enregistrez
  vos réglages comme préréglage ; `luma export *.ARW --preset Web --preset Impression`.
- **Masques par plage** : la zone d'un masque (dégradé, pinceau, image entière) peut
  ne garder que certains tons (plage de luminance : minimum, maximum, fondu) ou
  certaines couleurs (plage de couleur : teinte, largeur, fondu ; les gris sont
  laissés). Mesurées en OKLab sur la photo, avant les retouches des masques ; la zone
  s'affiche en rouge (O).
- **Pinceau qui suit les bords** : cochez « Suivre les bords » et le pinceau ne peint
  que les tons proches du centre de chaque touche ; la zone s'arrête au contour d'un
  ciel, d'un mur, d'un visage, sans déborder.
- **Combinaisons de masques** : « Combiner » ajoute, soustrait ou garde l'intersection
  d'une autre forme (dégradé linéaire, radial, pinceau, chacune inversable) ; un
  radial dont on retire un autre devient un anneau, un pinceau soustrait épargne un
  visage sous un dégradé.
- **Zone du masque à votre goût** : la zone affichée (O) se montre en rouge, vert,
  bleu, blanc ou blanc sur noir, avec une opacité de 10 à 100 %.
- **Correcteur et tampon** (Q) : un clic sur une poussière ou un détail gênant, et
  LÜMA choisit lui-même la partie de la photo qui le remplace. Le correcteur en garde
  la texture et prend la lumière du bord de la zone ; le tampon en fait une copie
  exacte. Zone et source se déplacent à la souris ; taille, contour progressif,
  opacité. Enregistrés dans le XMP.
- **Mélangeur de couches** (outil Couleur) : chaque couche rouge, verte ou bleue faite
  d'un mélange des trois couches de la photo, de −200 à +200 % ; enregistré dans le
  XMP et dans les préréglages, sans ralentir le rendu.
- **Moiré et frange au pinceau** : deux curseurs de plus dans chaque masque. « Moiré »
  adoucit les couleurs parasites d'un tissu ou d'une grille, « Frange » retire les
  liserés colorés le long des contours ; la luminosité ne bouge pas.
- **Yeux rouges** : un cercle sur chaque œil, et la pupille rouge redevient noire ; la
  peau autour et le reflet blanc de l'œil ne changent pas.
- **Taches de capteur** : « Voir les taches » fait ressortir les poussières du capteur
  en points sombres ; « Retirer les poussières » les trouve dans les zones unies (un
  ciel, un mur) et pose un correcteur sur chacune, en laissant les textures, les
  fils et le flou des arrière-plans.
- **Profils ICC lus** (étape vers l'épreuvage à l'écran) : profils des écrans
  (matrice et courbes) et des imprimantes et papiers (tables lut8, lut16, A→B et
  B→A, versions 2 et 4), dans les deux sens et pour les quatre intentions de rendu,
  en Rust pur ; un profil tronqué ou faux est refusé sans dommage. `luma icc`
  décrit un profil : classe, espace, intentions, blanc du support, blanc, noir et
  primaires du périphérique.
- **Plus rapide, plus léger** : les corrections d'objectif enregistrées dans les RAW
  (Sony, Fujifilm, OM System, Panasonic) sans copie de l'image, 17 à 35 % plus
  vite en pleine définition (ARW 602 → 408 ms, RW2 523 → 339 ms), pixels identiques
  au bit près ; les doublons de travail 5 à 9 % plus légers et relus jusqu'à 17 %
  plus vite (CR3 331 → 274 ms), toujours sans perte.
- **Épreuvage à l'écran** (sous l'histogramme) : choisissez le profil ICC d'une
  imprimante et d'un papier, et l'aperçu montre la photo telle qu'elle sortira, en
  intention perceptuelle ou colorimétrique relative ; l'histogramme suit. Une vue,
  pas un réglage : rien n'est écrit dans la photo.
- **iPhone plus simple** : une flèche Annuler dans la barre du haut et un menu « ••• »
  pour le reste (rétablir, copier, coller, réinitialiser, langue) ; un bouton « Avant »
  qui dit ce qu'il montre ; chaque outil nommé sous son icône ; dans la feuille des
  outils, les curseurs d'abord.
- **LÜMA Premium dans l'App Store** (iPhone et iPad) : les deux abonnements à leur
  prix App Store, « S'abonner » et « Restaurer les achats » ; chaque achat et
  renouvellement ouvre Premium. Sur iPhone et iPad, seul l'App Store vend
  l'abonnement ; sur Mac, seulement le site (Stripe).
- **Alerte de gamut** (dans l'épreuvage) : les couleurs que l'imprimante ne saura
  pas rendre passent en bleu sur l'aperçu, avec ou sans la simulation de
  l'impression.
- **Papier et encre noire** (dans l'épreuvage) : « Couleur du papier » montre le ton
  et la luminosité du papier, « Encre noire » le vrai noir de l'encre, plus clair
  que celui de l'écran ; sans elles, le blanc du papier et le noir de l'encre sont
  ramenés au blanc et au noir de l'écran.
- **Dossier surveillé** (Bibliothèque) : les photos qui arrivent dans un dossier
  (boîtier relié, carte copiée) entrent dans la photothèque et le film dès qu'elles
  sont complètes, et la dernière s'ouvre (ou s'ajoute seulement, au choix) ;
  `luma watch dossier` fait de même en ligne de commande.
- **Boîtier relié, sur Mac** (Dossier surveillé) : « Relier un boîtier » ouvre le
  boîtier branché (USB, ou réseau s'il le permet) par le cadre du système ; chaque
  photo prise arrive dans le dossier surveillé et s'ouvre, celles déjà sur la carte
  restent où elles sont ; « Déclencher » pour les boîtiers qui se déclenchent à
  distance.
- **Tri en plein écran** (F) : la photo seule, notée au clavier (0 à 5), avec ses
  drapeaux (P, X, U) et ses libellés de couleur (6 à 9 : rouge, jaune, vert, bleu ;
  violet à la souris) ; la photo suivante vient d'elle‑même (A pour couper), Échap
  pour sortir. Les libellés s'écrivent dans le XMP sous les noms du format, que
  lisent les autres logiciels, et se cherchent en français, anglais ou espagnol.
- **Comparaison** (C) : 2 à 4 photos côte à côte ou en grille (celles choisies, sinon
  la photo ouverte et la suivante), chacune développée avec ses réglages ; le zoom
  et le déplacement sont communs à toutes, la loupe (L) agrandit le même endroit de
  chacune, un clic ouvre la photo.
- **iPhone plus simple et plus sobre** : l'accueil ne se redessine plus deux fois par
  seconde pour rien (seulement pendant un import depuis Photos) ; la barre du haut ne
  montre que ce qui sert : sans photo, ni « Après » ni export ; l'épreuvage pour l'impression n'encombre plus l'iPhone et l'iPad.
- **Rafales et séries d'exposition empilées** : les photos d'un même boîtier à moins
  de deux secondes d'écart se replient derrière la première (« Rafale ×5 »,
  « Série ×3 ») ; S les déplie, « Fusionner la série » ouvre l'outil Fusion avec les
  photos d'une série d'exposition.

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
