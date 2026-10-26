# Fonctions

Ce que LÜMA sait faire, et ce qui vient, domaine par domaine : le développement, les
formats et la couleur, l'export, la bibliothèque. Chaque fonction marquée « fait » est
couverte par les tests.

## Principes

- **Non destructif** : une retouche est une liste de paramètres, rejouée à chaque
  rendu ; l'original n'est jamais modifié.
- **Des curseurs précis** : chacun avec son nom, sa plage, sa valeur par défaut et
  son sens ; température et teinte sur l'échelle DNG (« 5500 K, +10 »).
- **Un moteur propre** : écrit en Rust, en virgule flottante 32 bits, sur le GPU, avec
  ses propres profils de rendu (Couleur, Neutre, Vif, Monochrome).
- **Le format XMP** : réglages lus et écrits en XMP `crs:`, dans un fichier à
  côté du RAW ou en préréglages.
- **Sa propre identité** : interface, rendu et raccourcis clavier (`\`, J, Z,
  copier‑coller, annuler…) pensés pour le travail au quotidien.

Légende : ✅ fait et testé · 🟡 partiel · ⏳ à faire

## Module Développement

| Panneau | Fonction | État |
|---|---|---|
| Profil | Couleur, Neutre, Vif, Monochrome ; lecture de `crs:CameraProfile` | ✅ |
| | LUT 3D `.cube` (comme les profils créatifs), à intensité réglable, sur le GPU aussi | ✅ |
| | Profils fabricant | ⏳ |
| Base | Balance des blancs telle quelle, température et teinte (échelle DNG) | ✅ |
| | Balance automatique, préréglages (lumière du jour, nuageux, ombre, tungstène, fluorescent, flash), pipette (W) | ✅ |
| | Exposition, Contraste | ✅ |
| | Hautes lumières, Ombres (adaptatifs à l'image) | ✅ |
| | Blancs, Noirs (adaptatifs : amènent les vrais extrêmes de la photo au blanc et au noir) | ✅ |
| | Texture, Clarté, Correction du voile | ✅ |
| | Vibrance, Saturation | ✅ |
| Courbe des tonalités | Paramétrique (4 régions, 3 séparations) | ✅ |
| | Courbes à points : luminosité, rouge, vert, bleu | ✅ |
| Mélangeur de couleurs | Teinte, saturation, luminance de 8 couleurs | ✅ |
| | Mélange noir et blanc (±3 IL, `crs:GrayMixer*`) | ✅ |
| | Outil de ciblage direct (T) : glisser sur une couleur de la photo règle cette couleur en teinte, saturation ou luminance | ✅ |
| Étalonnage des couleurs | Ombres, tons moyens, hautes lumières, global ; fusion, balance | ✅ |
| Détail | Accentuation : gain, rayon, détail, masquage | ✅ |
| | Réduction du bruit de luminance et de couleur | ✅ |
| | Réduction du bruit avancée (moyennes non locales, ondelettes), sans IA | ⏳ |
| Corrections de l'objectif | Aberration chromatique latérale estimée sur la photo, distorsion, vignetage (gain, milieu), défrangeage violet et vert | ✅ |
| | Corrections d'objectif écrites dans les DNG (opcodes `WarpRectilinear` et `FixVignetteRadial`) : distorsion, aberration chromatique latérale et vignetage, appliqués automatiquement à toutes les tailles | ✅ |
| | Corrections d'objectif des ARW de Sony (distorsion, aberration chromatique latérale, vignetage, enregistrées par le boîtier), appliquées automatiquement sans bord vide | ✅ |
| | Corrections d'objectif des RAF de Fujifilm (distorsion, aberration chromatique latérale, vignetage, enregistrées par le boîtier, mode recadré compris), appliquées automatiquement | ✅ |
| | Corrections d'objectif des ORF d'Olympus et d'OM System (distorsion, aberration chromatique latérale) et des RW2 de Panasonic (distorsion), enregistrées par le boîtier, appliquées automatiquement | ✅ |
| | Case « Corrections du boîtier » : cochée par défaut, enregistrée dans le XMP (`crs:LensProfileEnable`) ; décochée, la photo revient telle que l'objectif l'a donnée, et les curseurs de distorsion et de vignetage prennent le relais | ✅ |
| | Aberration chromatique longitudinale, corrections des autres marques (Canon, Nikon, Pentax…) | ⏳ |
| Transformation | Perspective verticale et horizontale, rotation, proportions, échelle (homographie, centre fixe) | ✅ |
| | Redressement automatique, guidé | ⏳ |
| Effets | Vignetage après recadrage (gain, milieu, arrondi, contour progressif, hautes lumières) | ✅ |
| | Grain (gain, taille, rugosité), identique à toutes les tailles | ✅ |
| Étalonnage de l'appareil photo | Teinte des ombres, teinte et saturation des primaires | ✅ |
| Outils | Recadrage et redressement (angle libre), outil à l'écran avec proportions verrouillées (R) | ✅ |
| | Masques : dégradé linéaire (M), dégradé radial (Maj+M), pinceau avec gomme (K) ; inverser, intensité, affichage de la zone (O) ; exposition, contraste, hautes lumières, ombres, blancs, noirs, température, teinte, saturation, clarté | ✅ |
| | Masques restreints à une plage de luminance (minimum, maximum, fondu) ou de couleur (teinte, largeur, fondu ; gris laissés), lue sur la photo ; zone affichée (O) | ✅ |
| | Pinceau qui suit les bords : chaque touche ne peint que les tons proches de son centre, la zone s'arrête aux contours de la photo | ✅ |
| | Combinaisons de masques : ajouter, soustraire, intersection d'autres formes (chacune inversable) | ✅ |
| | Lecture des masques `crs:MaskGroupBasedCorrections` | ⏳ |
| | Correcteur, tampon, yeux rouges, suppression | ⏳ |
| | Historique, annuler et rétablir, copier‑coller des réglages | ✅ dans LÜMA Studio |
| | Copier‑coller sélectif des réglages (« Coller… », Alt+Maj+Cmd+V) : quatorze groupes au choix, recadrage et masques laissés à chaque photo par défaut | ✅ |
| | Instantanés, synchronisation sur plusieurs photos | ⏳ |
| | iPhone et iPad : aperçu réduit pendant un geste (12 fois plus rapide), mémoire surveillée, caches libérés avant que le système ne ferme l'app | ✅ |
| | Histogramme, indicateur d'écrêtage (J), avant/après (`\`), zoom 1:1 | ✅ |
| | Panorama (perspective, cylindrique, sphérique), rognage automatique réversible, DNG | ✅ |
| | Empilements : moyenne, médiane, maximum, ciel étoilé | ✅ |
| | Fusion HDR, déformation des bords du panorama | ⏳ |
| | 100 préréglages avec intensité : pellicules couleur, cinéma, noir et blanc, instantanés ; tous mesurés sur une mire à chaque test (distincts, sans image écrasée, rendus identiques sur le GPU) | ✅ |
| | Préréglages utilisateur (enregistrer, importer des `.xmp`), export à l'unité ou en dossier au format XMP | ✅ |
| | Photo modifiée signalée sous la photo et sur les vignettes (film, intensité, panneaux retouchés) | ✅ |
| | Mesure d'une vraie pellicule : un scan comparé à la même scène en numérique, alignement automatique, LUT 3D | ✅ |
| | Films empilés par masques, chacun dans sa zone et à son intensité, hautes lumières, ombres et clarté comprises | ✅ |

## Formats et couleur

| Fonction | État |
|---|---|
| RAW Bayer : Sony ARW, Canon CR3, Nikon NEF (et les autres formats RAW courants) | ✅ 3 boîtiers vérifiés en continu |
| RAW X‑Trans (Fujifilm) | ✅ dématriçage dédié |
| DNG linéaire, Apple ProRAW (avec `BaselineExposure`) | ✅ |
| Images développées : JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG ; HEIC, HEIF, AVIF (ImageIO d'Apple) ; extension inconnue lue d'après le contenu | ✅ profil « Original » : identique à l'ouverture |
| Couleur DNG : deux matrices interpolées selon la température | ✅ |
| Pipeline en virgule flottante 32 bits, espace de travail Rec. 2020 linéaire | ✅ |
| Hautes lumières saturées au capteur rendues neutres (pas de ciel rose) | ✅ |
| Orientation EXIF | ✅ |
| Profils ICC lus (versions 2 et 4) : matrice et courbes, tables lut8, lut16, A→B et B→A des imprimantes et des papiers, quatre intentions de rendu (`luma icc`) | ✅ |
| Épreuvage à l'écran : l'aperçu tel qu'il sera imprimé avec un profil d'imprimante, intention perceptuelle ou colorimétrique relative (histogramme compris) | ✅ |
| Alerte de gamut : les couleurs que l'imprimante ne saura pas rendre, en bleu sur l'aperçu (aller et retour en colorimétrique relative, écart au‑delà de ΔE 3) | ✅ |

## Export

| Fonction | État |
|---|---|
| JPEG (qualité), PNG et TIFF 8 ou 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, avec profil ICC incorporé | ✅ |
| Redimensionnement : grand côté, petit côté, largeur × hauteur, mégapixels, sans jamais agrandir (`--resize`) | ✅ |
| Fenêtre Exporter : format, qualité, profondeur, espace, taille, dossier ; export de toutes les photos du filtre | ✅ |
| Export vers plusieurs préréglages à la fois (fenêtre Exporter : cases à cocher, reprendre, enregistrer ; `luma export --preset A --preset B`) | ✅ |
| Préréglages d'export (format, qualité, profondeur, espace, taille, dossier) : Web, Pleine définition, Écran P3, Impression, et les vôtres ; `luma export-preset`, `luma develop --preset` | ✅ |
| Modèles de nom des fichiers : `{nom}`, `{date}`, `{heure}`, `{séquence}`, `{boîtier}`, `{objectif}`, `{iso}`, `{note}`, `{libellé}`…, sous‑dossiers par « / » ; dans la fenêtre Exporter, les préréglages et `luma develop --name` | ✅ |
| Accentuation de sortie : écran, papier mat, papier brillant ; faible, normale, forte ; à la taille finale (`--output-sharpen`) | ✅ |
| Filigrane texte (police de LÜMA, lissé) : neuf emplacements, taille, opacité, blanc ou noir (`--watermark`) | ✅ |
| Filigrane logo : un PNG avec sa transparence, couleurs fidèles, neuf emplacements, taille, opacité (`--logo`) | ✅ |
| Métadonnées d'export : EXIF de la prise de vue recopié (boîtier, objectif, réglages, date), auteur et copyright, mots‑clés, note et libellé en XMP, position GPS retirée par défaut (`--author`, `--keep-gps`) | ✅ |
| Métadonnées dans les TIFF aussi : EXIF, profil ICC et XMP, TIFF écrit par LÜMA (mêmes pixels, même taille) | ✅ |

## Bibliothèque et interopérabilité

| Fonction | État |
|---|---|
| Lecture des XMP (sidecar `IMG_0001.xmp`), écriture au même format | ✅ enregistrement automatique dans l'app |
| Film du dossier avec vignettes, navigation ← → | ✅ |
| Grille, notes (0 à 5), drapeaux (P, X, U), filtre par note et drapeau, dans le XMP | ✅ |
| Catalogue local (`luma catalog`) : dossiers et sous‑dossiers, date de prise de vue, boîtier, objectif, ISO lus dans l'en‑tête sans décoder l'image, mise à jour incrémentale, tri par date | ✅ |
| Vignettes instantanées (l'aperçu intégré au RAW, en 15 ms), puis rendues avec les réglages et enregistrées sans perte pour les visites suivantes | ✅ |
| Doublon de travail en pleine définition : identique à l'original, ouverture plus rapide, mémoire divisée par deux et demi ; « Vider le cache » en un clic | ✅ |
| Mots‑clés dans le XMP (`dc:subject`), lus et écrits | ✅ |
| Recherche et filtres (`luma catalog --search`) : mots sans tenir compte des accents ; note, drapeau, libellé, dates, ISO, focale, boîtier, objectif, mot‑clé ; décompte des mots‑clés, boîtiers et objectifs | ✅ |
| Collections manuelles et dynamiques (règle : note, date, boîtier, objectif, ISO, mots‑clés…), enregistrées dans le catalogue | ✅ |
| Mots‑clés dans l'app : saisie (plusieurs à la fois, séparés par une virgule), ajout et retrait sur toutes les photos choisies, liste des mots‑clés du dossier avec leur nombre, filtre de la grille | ✅ |
| Photothèque dans l'app : chaque dossier ouvert y entre (en arrière‑plan) ; toutes les photos, les collections et les dossiers, chacun avec son nombre de photos ; recherche et filtres ; vues triées par date de prise de vue | ✅ |
| Copies virtuelles : plusieurs développements d'une photo sans dupliquer le fichier (un XMP par copie), chacune avec sa note, ses mots‑clés et sa vignette, à côté de son original ; export nommé `IMG_0001-2` ; suppression sans toucher l'original | ✅ |
| Import de catalogues existants | ⏳ |

## Ce qui distingue LÜMA

- **Rien n'est envoyé nulle part** : pas de compte, pas de cloud obligatoire.
- **100 % Rust, vérifié** : `scripts/audit-rust.sh` contrôle le graphe de dépendances à
  chaque build (170 crates, aucune liaison native ni compilateur C).
- **Mesuré, pas promis** : `luma bench` et la CI publient les temps de rendu réels.
