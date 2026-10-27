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
  rapide (NEF 160 → 85 ms), mêmes fichiers octet pour octet. Puis la couleur (profil
  et Saturation), de même : trois fois plus rapide (NEF 147 → 48 ms). Puis le codage
  des JPEG, encore 13 à 15 % plus court (24 Mpx : 200 → 175 ms), mêmes octets. Puis
  les corrections d'objectif enregistrées par le boîtier (Sony, Fujifilm, Olympus,
  Panasonic), huit pixels à la fois : 30 % plus rapides (ARW 24 Mpx : 385 → 270 ms),
  mêmes fichiers octet pour octet. Puis la Vibrance, de même : l'étape couleur 2,7 fois
  plus rapide quand elle est réglée (NEF 372 → 136 ms), mêmes octets.
  Puis les exports TIFF et PNG en 16 bits : l'image
  écrite directement en octets, sur tous les cœurs, sans seconde copie : 20 à 28 % plus
  courts (24 Mpx : 372 → 299 ms) et 144 Mo de mémoire en moins, mêmes octets.
  Puis la réduction du bruit par ondelettes, sans image neuve à chaque échelle : 1,8 fois
  plus rapide (24 Mpx : 1,17 → 0,65 s), et l'accentuation par déconvolution, huit pixels
  à la fois : 28 % plus courte (0,83 → 0,60 s), mêmes pixels.
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
- **Dématriçage directionnel** : le vert de chaque pixel suit les contours au lieu de
  les croiser ; les branches, cheveux et textures fines n'ont plus de pointillés ni
  d'effet « fermeture éclair » (4 dB de mieux sur nos essais). Les doublons de travail
  sont refaits une fois.
- **Fluidité** (Retouche) : Pousser, Gonfler et Dégonfler au pinceau, avec sa taille et sa
  force ; pour redresser une ligne, affiner une forme ou corriger un pli. Chaque coup
  est gardé dans le XMP et s'annule. Sans IA.
- **Sélectionner le ciel** (Masques, Masquage) : un clic, et le ciel devient un
  masque, nuages compris, bords des arbres et des toits suivis ; un lac qui le reflète
  reste à part. Sans IA : seulement les couleurs et la texture de la photo.
- **Supprimer** (Retouche, à côté du correcteur) : un cercle sur ce qui gêne, un fil, un
  passant, un déchet, et LÜMA le remplace par des morceaux de la photo qui l'entourent,
  texture et grain compris. Sans IA : seulement la photo elle-même.
- **Bords du panorama déformés** (Fusion, Panorama, « Déformer les bords », ou
  `luma merge --pano … --warp`) : au lieu de rogner les bords irréguliers d'un
  panorama, LÜMA les étire doucement pour remplir tout le rectangle ; on garde toute la
  scène.
- **Bords vides rognés** (Géométrie, Transformation) : après une perspective, une
  rotation ou une correction de distorsion, un clic recadre au plus grand cadre sans
  coin noir, aux proportions de la photo.
- **Redressement guidé** (Géométrie, Transformation) : glissez le long de deux à
  quatre arêtes qui doivent être droites (un mur, un poteau, l'horizon) ; LÜMA règle
  aussitôt la perspective et la rotation. Une seule ligne ne fait que tourner la photo.
- **Couleurs accordées entre deux boîtiers** : la même scène photographiée par vos deux
  boîtiers, même cadrée un peu autrement ; choisissez les deux photos, puis « Accorder
  sur la photo choisie » (Mélangeur de couches), ou `luma match-colors`. Le second
  boîtier prend les couleurs du premier ; enregistrée en préréglage, la correction se
  pose sur toutes ses photos.
- **Profil de boîtier** : photographiez une charte ColorChecker, recadrez-la au plus
  juste, puis « Profil depuis une charte » (Mélangeur de couches) ou
  `luma colorchecker charte.ARW -o profil.xmp` : LÜMA calcule la correction qui rend
  ses 24 couleurs exactes (de 3,8 à 0,02 d'écart sur nos essais), le blanc restant
  neutre. Enregistrée en préréglage, elle se pose sur toutes les photos de ce boîtier.
- **Empilement de mise au point** (Fusion, « Mise au point », ou
  `luma merge --stack focus`) : des poses dont la mise au point avance, en macro ou en
  paysage, deviennent une seule photo nette du premier plan au fond. Chaque pose est
  alignée sur sa voisine, et LÜMA prend à chacune ce qu'elle a de plus net.
- **Fusion HDR** (Fusion, « HDR », ou `luma merge --stack hdr`) : des poses de
  bracketing, même à main levée, deviennent un seul DNG où les ombres et le ciel
  gardent leur détail. LÜMA mesure lui-même l'écart d'exposition entre les poses, et ce
  qui a bougé de l'une à l'autre (feuillage, passants) ne laisse pas de fantôme.
- **Hautes lumières reconstruites** : la case « Reconstruire les hautes lumières »
  (Lumière, sous la Tonalité) rend à un canal saturé par le capteur la couleur de ses
  voisins ; un ciel brûlé redevient orangé, une serviette blanche retrouve sa texture.
  Un pixel où tout sature reste blanc. Aussi en ligne de commande
  (`--set highlights.reconstruction=on`).
- **Accentuation par déconvolution** : la case « Déconvolution » de l'Accentuation
  défait le flou de l'objectif au lieu d'en creuser les contours ; du vrai détail
  revient, sans halo (1,6 dB de mieux que le masque flou sur nos essais). Visible à
  100 % et à l'export ; aussi en ligne de commande (`--set sharpen.deconvolution=on`).
- **Réduction du bruit par ondelettes** : la case « Ondelettes » du Détail lisse la
  luminance échelle par échelle, chacune selon le bruit mesuré sur la photo ; à force
  égale, plus de texture et de contours gardés que le filtre habituel (3,7 dB de mieux
  sur nos essais), pour environ deux fois son temps. Sans IA.
- **Verticales automatiques** (Maj+V, ou le bouton de la Transformation) : un
  bâtiment photographié en visant vers le haut retrouve des murs droits ; LÜMA
  mesure où ses verticales se rejoignent et règle la Verticale en conséquence.
- **Redressement automatique** (Maj+R, ou « Redresser automatiquement » dans le
  recadrage) : LÜMA lit l'horizon et les verticales de la photo et la remet d'aplomb ;
  dans le doute (une photo de près, toute en lignes de fuite), il ne touche à rien.
- **Guides de recadrage** : tiers, nombre d'or, diagonales, triangles d'or, spirale
  d'or et grille ; O passe au guide suivant pendant le recadrage, Maj+O tourne la
  spirale et les triangles.
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
- **« Importer des photos » sur iPhone et iPad** : le sélecteur de Photos s'ouvre
  depuis la fenêtre de LÜMA elle‑même, et non plus depuis une fenêtre cherchée parmi
  celles de l'app (sur un appareil, celle du clavier ou de TestFlight ne montre rien) ;
  s'il ne s'affiche pas, LÜMA le dit au lieu de rester muette.
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
- **Doublons** (« Chercher les doublons ») : la même image dans plusieurs fichiers,
  copiée, exportée à une autre taille ou recompressée, retrouvée par son empreinte et
  montrée groupe par groupe. **En lot** : avec plusieurs photos choisies, la note, le
  drapeau et le libellé vont à toutes.
- **Ouverture deux fois plus rapide** : les niveaux du capteur (noir, blanc) ramenés
  directement depuis ses entiers, sur tous les cœurs, sans copie préalable en
  flottants ; NEF 145 → 67 ms, ARW 143 → 65, RW2 131 → 65, RAF 169 → 73, DNG
  285 → 158, CR3 547 → 399, données identiques au bit près.
- **Synchroniser une sélection** (Maj+Cmd+S, ou « Synchroniser… » dès deux photos
  choisies) : les réglages de la photo ouverte vont aux autres, groupe par groupe
  (recadrage et masques laissés à chacune par défaut). **Balance auto en lot** : chaque
  photo choisie reçoit sa propre balance automatique.
- **Réglages par défaut par boîtier et par ISO** (« Réglages par défaut… », outil
  Lumière) : les réglages de la photo ouverte deviennent le point de départ des photos
  de ce boîtier sans XMP, à tout ISO ou à partir de son ISO (la règle la plus précise
  l'emporte ; recadrage et masques exceptés). **Film d'import** : choisi dans le
  dossier surveillé, il est donné à chaque photo qui arrive.
- **Informations IPTC** (Bibliothèque, « Informations ») : titre, légende, auteur,
  droits et lieu de la prise de vue, pour une photo ou toutes les photos choisies,
  dans le XMP et les JPEG exportés ; **mots‑clés hiérarchiques** (« Lieux > France >
  Paris ») ; **modèles de métadonnées** (auteur, droits, lieu, mots‑clés) enregistrés
  et donnés d'un coup.
- **Instruments de mesure** (au‑dessus de l'histogramme, ou Maj+J) : **forme d'onde**
  (la luminance, colonne par colonne), **parade RVB** (chaque couche à part) et
  **vectorscope** (teinte et saturation, avec les cibles des primaires à 75 % et la
  ligne des tons chair), calculés sur l'aperçu à chaque rendu, épreuvage compris.
- **Mesure au point** : sous l'histogramme, le RVB (sRGB, 0 à 255) et le Lab du point
  que survole le pointeur, lus sur les vraies couleurs même quand l'écrêtage est
  affiché. **Écrêtage couche par couche** : sur la photo comme dans le coin de
  l'histogramme, la couleur dit quelles couches sont brûlées (rouge, jaune pour rouge
  et vert…, blanc pour les trois).
- **Renommage par lot** (« Renommer… » sur les photos choisies, `luma rename`) : un
  modèle avec les repères de l'export (`{date}_{séquence}`, `{boîtier}-{nom}`…),
  l'aperçu des nouveaux noms avant de valider ; le RAW et le JPEG d'une même prise
  gardent un même nom, le XMP et les copies virtuelles suivent, un nom déjà pris reçoit
  `-2` ; au moindre échec, tout revient comme avant. La photothèque et ses collections
  suivent.
- **Instantanés** (outil Lumière, sous l'historique) : les réglages d'un moment, sous
  un nom, rangés dans le XMP de la photo ; un clic y revient (une étape de l'historique,
  qu'on peut annuler).
- **Sauvegardes du catalogue** : une par semaine, d'elle‑même, dans « Sauvegardes » à
  côté du catalogue (les dix dernières gardées) ; `luma catalog --backup`, `--backups`,
  `--restore` (le catalogue remplacé est sauvegardé d'abord). **Photos manquantes** :
  `--missing` les liste, `--relocate ancien nouveau` retrouve un dossier déplacé hors de
  LÜMA, collections comprises.
- **Carte** (Bibliothèque, « Carte ») : les photos du film à l'endroit de leur prise
  de vue (position GPS de l'EXIF, gardée dans la photothèque), regroupées quand elles
  sont proches ; glisser, zoomer à la molette ou à deux doigts, toucher un groupe pour
  voir ses photos. Fond de tuiles libres, téléchargées par le système et gardées sur
  l'appareil ; hors ligne, celles déjà gardées.
- **Les photos du téléphone à leur date et sur la carte** : l'EXIF des images
  développées (JPEG, HEIC, AVIF, PNG, WebP, TIFF) est lu comme celui d'un RAW — date de
  prise de vue, boîtier, objectif, réglages, position GPS. Les photos déjà dans la
  photothèque sont relues une fois.
- **Impression** (« Imprimer… » sur les photos choisies, `luma print`) : un PDF à la
  taille exacte du papier (A4, A3, A3+, 10 × 15 cm…), portrait ou paysage, marges, une
  photo par page ou une **planche contact** (grille, nom sous chaque photo), photos
  entières ou remplissant leur case, accentuation pour papier mat ou brillant, aperçu
  de la page ; les pages converties vers le **profil de l'imprimante** (celui de
  l'épreuvage, noir compensé), à imprimer sans gestion des couleurs du pilote.
- **Livre photo** (« Livre, diaporama, galerie… », `luma book`) : un PDF avec sa couverture
  (la première photo, le titre et le sous‑titre), puis des pages d'une, deux ou quatre
  photos choisies d'après leur orientation ; formats 20 × 20, 30 × 30, A4, 21 × 28, A3.
- **Diaporama** : en plein écran dans l'app (la photo seule, elle avance d'elle‑même,
  Espace pour la pause, Échap pour sortir), et **exporté** (`luma slideshow`) : un
  dossier qui se suffit à lui‑même, index.html et les photos, à ouvrir dans n'importe
  quel navigateur ou à mettre en ligne (fondu, flèches, glissement du doigt, plein
  écran).
- **Galerie web** (« Livre, diaporama, galerie… », `luma gallery`) : un dossier
  statique à mettre en ligne tel quel, la grille des vignettes et chaque photo en grand
  d'un clic (flèches, glissement du doigt, Échap), avec titre, description et légendes.
- **JPEG allégé pour le web** : chrominance 4:2:0 et tables de Huffman calculées pour
  chaque image, de 9 à 29 % plus léger sur les photos d'essai, pour un écart à peine
  mesurable ; dans le préréglage Web, la fenêtre d'export (« Allégé pour le web ») et
  `--web`.
- **Vidéo de base** (`luma video`) : un film de LÜMA (« Portra 400 », « Tri-X 400 »…),
  un préréglage XMP ou des réglages appliqués à chaque image d'une vidéo courte en
  Motion JPEG (le format AVI de nombreux appareils photo), réenregistrée au même format :
  même taille, même rythme, le son recopié tel quel. Lu et écrit en Rust pur.
- **Import depuis une carte** (« Importer une carte… » dans la Photothèque, `luma
  import`) : la carte montée est trouvée seule ; les photos sont rangées par date de
  prise de vue, renommées si on le veut, copiées aussi sur un second disque ; chaque
  copie est relue et comparée à l'original, le RAW, son JPEG et son XMP gardent un même
  nom, une photo déjà importée est sautée, la carte n'est jamais modifiée.
- **Raccourcis clavier personnalisables** (menu de la langue, « Raccourcis
  clavier… ») : chaque commande et ses touches ; cliquer, puis appuyer sur les
  nouvelles touches ; une touche déjà prise passe à la nouvelle commande ; retour aux
  touches par défaut, une à une ou toutes. **Tab** masque les panneaux.
- **Espaces de travail** : Tri, Développement, Retouche, Présentation, et ceux qu'on
  enregistre (l'outil et les panneaux), de Cmd+1 à Cmd+9.
- **iPhone : la grille des photos recalée** : colonnes calculées sur la largeur de
  l'écran, aux marges des îlots, le nom au‑dessus de la photo et non plus dessus ; la
  barre Grille, Carte et Diaporama tient sur une ligne (« Livre, diaporama,
  galerie… » passe dans « ••• ») ; l'histogramme seul dans le panneau Photos. Après
  une mise à jour de l'app, les photos suivent le nouveau dossier de l'app au lieu
  d'apparaître dans un second « Documents ».
- **iPhone et iPad : le toucher tombe juste** : l'image a enfin la taille de l'écran
  (elle était faite pour la zone sûre puis étirée de 11 % en hauteur) ; le bas des
  panneaux s'affiche (Instantanés, sous l'historique), le doigt fait défiler le
  panneau et touche ce qu'il vise. La barre du haut contient ses boutons de 40 points.
- **iPhone et iPad : l'export va dans la Pellicule** : chaque image exportée est ajoutée
  à l'app Photos telle quelle (EXIF, IPTC et profil de couleur gardés ; iOS demande
  l'autorisation la première fois), et reste aussi dans LÜMA/ (app Fichiers). La
  fenêtre Exporter tient dans l'écran de l'iPhone, chaque réglage sous son titre.
- **Films rangés en familles qui se plient** : pellicules couleur, noir et blanc,
  cinéma, instantanés, mes préréglages, mes pellicules ; chaque titre dit combien la
  famille compte de films, et seule celle du film appliqué s'ouvre d'abord.
- **150 films** : 50 de plus. Pellicules couleur (Kodacolor 200, Portra 160VC et
  400VC, Ektachrome 64, Fujicolor 100 Industrial, Superia Venus 800, Sensia 100, Precisa
  100, Agfa Portrait 160, Konica VX 200, E100VS, Phoenix 200, Lomochrome Purple et
  Metropolis, Aerochrome), cinéma (EXR 200T, Eterna 400T, F-64D, XTR 250, Super 16,
  70 mm, Néon, Épouvante, Romance, Documentaire, Été indien, Thriller urbain), noir et
  blanc (Delta 400, Kentmere 400, SFX 200, Fomapan 100, RPX 25, Pancro 400, CHS 100 II,
  T-Max P3200, Pan 400, Verichrome Pan), instantanés (Polaroid i-Type, Spectra et 669,
  Instax Wide, Impossible Project, Instamatic) et créatifs (Pastel, Automne, Hiver,
  Printemps, Brume, Contre-jour, Bichromie). Tous vérifiés sur la mire comme les
  autres : distincts, sans image écrasée, noir et blanc neutres, mêmes rendus sur le
  GPU.
- **Rechercher un film** (outil Films) : par nom ou par description (« Kodak »,
  « chaud », « noir et blanc »), sans accents ni majuscules ; seuls les films trouvés
  restent, dans leurs familles ouvertes.
- **275 films** : 125 de plus. 35 pellicules couleur (Gold 100, Pro Foto 100, Portra
  100T, Ektar 25, Kodachrome 200, Fortia 50, Superia 1600, Vista 400, Ultra 50,
  Agfacolor CT18, Impresa 50, Redscale, Orwocolor NC 19…), 30 films et looks de cinéma
  (Vision 320T, Vision2 250D et 100T, Eastman 5248 et 5287, Eterna 500, Kodachrome
  Super 8, Technicolor IB, Néo-Tokyo, Âge d'or, Science-fiction, Giallo, Nouvelle Vague,
  VHS, Caméscope…), 25 noir et blanc (Tri-X 320, T-Max 100, HP5 à 3200, Ortho 25,
  Silvermax, CMS 20, APX 400, ORWO UN54 et N74, Svema 125, P30, HIE, Neopan 400,
  Technical Pan…), 15 instantanés (Polaroid Duochrome, bleu, Type 100, 55, Time-Zero,
  Instax Mini, Square, Monochrome et sépia, Photomaton…) et 20 créatifs (Ambre, Menthe,
  Lavande, Océan, Forêt, Nuit bleue, Aube, Crépuscule, Mat chaud et froid,
  Clair-obscur, Pop, Sélénium, Cuivre…). Chaque noir et blanc a sa sensibilité aux
  couleurs. Tous vérifiés sur la mire : distincts deux à deux, sans image écrasée, noir
  et blanc neutres, mêmes rendus sur le GPU.
- **iPhone et iPad : un mois d'essai, puis LÜMA Premium à vie pour 89,99 €** : un seul
  achat dans l'App Store, sans abonnement, restaurable sur vos autres appareils et
  partageable en famille. Sur Mac, rien ne change (14 jours d'essai, puis
  l'abonnement).

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
