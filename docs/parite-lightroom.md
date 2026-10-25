# LÜMA face à Lightroom Classic : la parité, fonction par fonction

LÜMA reprend le **flux de travail** de Lightroom Classic : les mêmes modules, les mêmes
panneaux, les mêmes curseurs avec les mêmes noms, plages et valeurs par défaut, et la
lecture des fichiers XMP de Lightroom. Un photographe y retrouve ses repères et ses
retouches existantes.

## Ce que « à l'identique » veut dire, et ce qu'il ne peut pas vouloir dire

| Repris tel quel | Volontairement différent |
|---|---|
| Organisation en modules (Bibliothèque, Développement, Export…) et en panneaux | L'apparence : LÜMA a sa propre identité visuelle, pas celle d'Adobe |
| Noms, plages, valeurs par défaut et sens de chaque curseur | Le moteur de rendu : celui d'Adobe est secret. LÜMA a le sien, écrit en Rust ; une même valeur a le même sens et la même amplitude, mais ne donne pas exactement les mêmes pixels |
| Réglages échangés au format XMP `crs:` (lecture et écriture) | Les profils d'Adobe (« Adobe Couleur »…), protégés : LÜMA propose ses équivalents (Couleur, Neutre, Vif, Monochrome) |
| Température et teinte sur l'échelle DNG (le même « 5500 K, +10 ») | Les marques et logos |
| Raccourcis clavier (`\`, J, Z, copier‑coller, annuler…) | |

Légende : ✅ fait et testé · 🟡 partiel · ⏳ à faire

## Module Développement

| Panneau | Fonction | État |
|---|---|---|
| Profil | Couleur, Neutre, Vif, Monochrome ; lecture de `crs:CameraProfile` | ✅ |
| | LUT 3D `.cube` (comme les profils créatifs), à intensité réglable, sur le GPU aussi | ✅ |
| | Profils fabricant (« Camera Standard »…) | ⏳ |
| Base | Balance des blancs telle quelle, température et teinte (échelle DNG) | ✅ |
| | Balance automatique, préréglages (lumière du jour, nuageux, ombre, tungstène, fluorescent, flash), pipette (W) | ✅ |
| | Exposition, Contraste | ✅ |
| | Hautes lumières, Ombres (adaptatifs à l'image, comme le processus 2012) | ✅ |
| | Blancs, Noirs | 🟡 fixes, pas encore adaptatifs à l'image |
| | Texture, Clarté, Correction du voile | ✅ |
| | Vibrance, Saturation | ✅ |
| Courbe des tonalités | Paramétrique (4 régions, 3 séparations) | ✅ |
| | Courbes à points : luminosité, rouge, vert, bleu | ✅ |
| Mélangeur de couleurs | Teinte, saturation, luminance de 8 couleurs | ✅ |
| | Mélange noir et blanc (±3 IL, `crs:GrayMixer*`) | ✅ |
| | Outil de ciblage direct | ⏳ |
| Étalonnage des couleurs | Ombres, tons moyens, hautes lumières, global ; fusion, balance | ✅ |
| Détail | Accentuation : gain, rayon, détail, masquage | ✅ |
| | Réduction du bruit de luminance et de couleur | ✅ |
| | Débruitage par IA, super‑résolution | ⏳ |
| Corrections de l'objectif | Aberration chromatique latérale estimée sur la photo, distorsion, vignetage (gain, milieu), défrangeage violet et vert ; noms XMP de Lightroom | ✅ |
| | Profils d'objectifs (base de données), aberration longitudinale | ⏳ |
| Transformation | Perspective verticale et horizontale, rotation, proportions, échelle (homographie, centre fixe) | ✅ |
| | Upright automatique, guidé | ⏳ |
| Effets | Vignetage après recadrage (gain, milieu, arrondi, contour progressif, hautes lumières) | ✅ |
| | Grain (gain, taille, rugosité), identique à toutes les tailles | ✅ |
| Étalonnage de l'appareil photo | Teinte des ombres, teinte et saturation des primaires | ✅ |
| Outils | Recadrage et redressement (angle libre), outil à l'écran avec proportions verrouillées (R) | ✅ |
| | Masques : dégradé linéaire (M), dégradé radial (Maj+M), pinceau avec gomme (K) ; inverser, intensité, affichage de la zone (O) ; exposition, contraste, hautes lumières, ombres, blancs, noirs, température, teinte, saturation, clarté | ✅ |
| | Masques par IA (sujet, ciel), plage de luminance ou de couleur ; lecture des masques des XMP de Lightroom | ⏳ |
| | Correcteur, tampon, yeux rouges, suppression | ⏳ |
| | Historique, annuler et rétablir, copier‑coller des réglages | ✅ dans LÜMA Studio |
| | Instantanés, synchronisation sur plusieurs photos | ⏳ |
| | Histogramme, indicateur d'écrêtage (J), avant/après (`\`), zoom 1:1 | ✅ |
| | Panorama (perspective, cylindrique, sphérique), rognage automatique réversible, DNG | ✅ |
| | Empilements : moyenne, médiane, maximum, ciel étoilé (au‑delà de Lightroom) | ✅ |
| | Fusion HDR, déformation des bords du panorama | ⏳ |
| | 100 préréglages avec intensité : pellicules couleur, cinéma, noir et blanc, instantanés ; tous mesurés sur une mire à chaque test (distincts, sans image écrasée, rendus identiques sur le GPU) | ✅ |
| | Préréglages utilisateur (enregistrer, importer des `.xmp` de Lightroom), export à l'unité ou en dossier au format Lightroom | ✅ |
| | Photo modifiée signalée sous la photo et sur les vignettes (film, intensité, panneaux retouchés) | ✅ |
| | Mesure d'une vraie pellicule : un scan comparé à la même scène en numérique, alignement automatique, LUT 3D (au‑delà de Lightroom) | ✅ |
| | Films empilés par masques, chacun dans sa zone et à son intensité, hautes lumières, ombres et clarté comprises (au‑delà de Lightroom) | ✅ |

## Formats et couleur

| Fonction | État |
|---|---|
| RAW Bayer : Sony ARW, Canon CR3, Nikon NEF (et les autres formats lus par rawler) | ✅ 3 boîtiers vérifiés en continu |
| RAW X‑Trans (Fujifilm) | ✅ dématriçage dédié |
| DNG linéaire, Apple ProRAW (avec `BaselineExposure`) | ✅ |
| Images développées : JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG (Rust pur) ; HEIC, HEIF, AVIF (ImageIO d'Apple) ; extension inconnue lue d'après le contenu | ✅ profil « Original » : identique à l'ouverture |
| Couleur DNG : deux matrices interpolées selon la température | ✅ |
| Pipeline en virgule flottante 32 bits, espace de travail Rec. 2020 linéaire | ✅ |
| Hautes lumières saturées au capteur rendues neutres (pas de ciel rose) | ✅ |
| Orientation EXIF | ✅ |

## Export

| Fonction | État |
|---|---|
| JPEG (qualité), PNG et TIFF 8 ou 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, avec profil ICC incorporé | ✅ |
| Redimensionnement (côté long) | ✅ |
| Fenêtre Exporter : format, qualité, profondeur, espace, taille, dossier ; export de toutes les photos du filtre | ✅ |
| Netteté de sortie, filigrane, copie des métadonnées EXIF, préréglages d'export | ⏳ |

## Bibliothèque et interopérabilité

| Fonction | État |
|---|---|
| Lecture des XMP de Lightroom (sidecar `IMG_0001.xmp`), écriture au même format | ✅ enregistrement automatique dans l'app |
| Film du dossier avec vignettes, navigation ← → | ✅ |
| Grille, notes (0 à 5), drapeaux (P, X, U), filtre par note et drapeau, dans le XMP | ✅ |
| Import, catalogue, collections, mots‑clés, recherche | ⏳ |
| Import d'un catalogue Lightroom (`.lrcat`) | ⏳ |

## Ce qui est déjà mieux, ou le sera

- **Rien n'est envoyé nulle part** : pas de compte, pas de cloud obligatoire.
- **100 % Rust, vérifié** : `scripts/audit-rust.sh` contrôle le graphe de dépendances à
  chaque build (170 crates, aucune liaison native ni compilateur C).
- **Mesuré, pas promis** : `luma bench` et la CI publient les temps de rendu réels.
