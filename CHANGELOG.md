# Nouveautés

## 0.4.0 — à venir

- **Comparaison plus rapide** : deux photos développées à la fois, aux mêmes images
  (quatre RAW en 1,57 s au lieu de 2,0 s).
- **Zones nettes dans la comparaison** : Maj+F les peint sur toutes les photos
  comparées à la fois, à la sensibilité choisie.
- **Sensibilité des zones nettes gardée** d'une séance à l'autre (« Tout remettre à
  zéro » la remet à la normale).
- **Corrections d'après le JPEG du boîtier plus rapides** : deux photos à la fois, aux
  mêmes mesures (sept RAW en 2,15 s au lieu de 2,9 s).
- **`luma before-after`** : l'avant / après de chaque photo d'un dossier (ou des photos
  données), à côté d'elle, avec ses réglages ; les mêmes noms que dans l'app.
- **Netteté dans la comparaison** : sous chaque photo comparée (C), sa part au point ;
  la plus nette est signalée en couleur.
- **Avant / après par lot plus rapide** : deux photos à la fois, aux mêmes fichiers
  (sept RAW en 2,35 s au lieu de 2,8 s) ; deux photos du même nom (IMG.ARW, IMG.JPG)
  ne s'écrasent jamais.
- **Télécharger depuis le site** : les boutons « Télécharger » (Mac, Linux, Windows)
  mènent aux fichiers de la dernière Release quand le site ne les sert pas lui‑même.
- **Tri d'une rafale au clavier** : Maj+P retient la plus nette de la sélection, Maj+X
  rejette les floues (raccourcis modifiables comme les autres).

## 0.3.0 — novembre 2026

La photothèque professionnelle, étape par étape.

- **`luma sharpness`** : la part au point de chaque photo, la plus nette d'abord, la
  même mesure que l'app ; `--pick` retient la plus nette, `--reject` rejette les
  floues (drapeaux dans les XMP).
- **Netteté mesurée plus vite** : deux photos à la fois (pendant qu'une se lit, l'autre
  se rend), aux mêmes parts au point : sept RAW en 1,7 s au lieu de 2,3 s.
- **Trier par netteté** : la part au point de chaque photo du film mesurée en
  arrière‑plan, puis le film rangé, la plus nette d'abord.
- **Sensibilité des zones nettes** (basse, normale, haute) : avec un objectif doux,
  la haute marque aussi les détails fins ; la basse ne garde que les contours francs.
- **Interface plus fluide** : l'image de chaque aperçu mise en texture sur tous les
  cœurs, aux mêmes couleurs : de 2,2 à 1,0 ms par aperçu sur le fil de l'interface.
- **Ordre inverse**, gardé d'une séance à l'autre : le film à l'envers (la plus récente
  ou la moins bien notée d'abord) ; l'ordre choisi revient au prochain lancement.
- **Trier par** : le film rangé par nom, par date de prise de vue ou par note (la
  meilleure d'abord) ; la photo ouverte et la sélection restent les mêmes.
- **Aperçus plus rapides** : l'histogramme de chaque aperçu compté sur tous les cœurs,
  aux mêmes comptes : de 3,3 à 1,3 ms par aperçu de 2,8 Mpx.
- **Rejeter les floues** : dans la barre de sélection, les photos choisies qui ont
  moins du tiers de la part au point de la plus nette reçoivent le drapeau « Rejetée ».
- **Correction : « Rendu… » sans fin** quand une collection ou une recherche était
  montrée pendant le calcul d'un aperçu de la photo ouverte : la photo est rendue de
  nouveau.
- **Curseurs plus réactifs** : la mémoire des étapes déjà calculées et chassées du
  cache reprise par l'étape suivante, aux mêmes pixels : l'étape d'entrée passe de
  9,6 à 5,7 ms, un rendu d'aperçu d'environ 9 % plus rapide.
- **Retenir la plus nette** : dans la barre de sélection, la part au point de chaque
  photo choisie (une rafale) mesurée en arrière‑plan ; la plus nette reçoit le drapeau
  « Retenue », les autres restent telles quelles.
- **Zones nettes** (Maj+F, bouton « Zones nettes ») : les détails au point de la photo
  en vert, sans IA, et leur part de la photo dans un badge, pour départager les
  photos d'une rafale ; une photo floue n'en a pas.
- **Exposition plus réactive** : la réduction du bruit de couleur (active par défaut,
  refaite à chaque mouvement de l'Exposition) écrite en boucles que le processeur mène
  par colonnes, aux mêmes pixels : de 18 à 23 % plus rapide, un rendu d'aperçu
  d'environ 7 %.
- **Export en WebP sans perte** : les mêmes pixels qu'un PNG 8 bits, de 20 à 45 %
  plus léger, avec le profil de couleur, l'EXIF et le XMP (app, préréglages,
  `luma develop -o photo.webp`).
- **L'avant / après de toute une sélection** : « Avant / après » dans la barre de
  sélection écrit, en arrière‑plan, un JPEG à côté de chaque photo choisie, chacune avec
  ses réglages.
- **Zoom 1:1 sur le point cliqué** : un clic sur la photo ouvre le zoom centré là où
  l'on a cliqué, et non plus sur le coin haut gauche.
- **Navigateur du zoom 1:1** : la photo entière en petit dans un coin, avec le cadre de
  ce qu'on voit ; un clic ou un glissement y mène la vue.
- **La photo et sa référence en une image** : « Exporter la paire » (vue de référence)
  écrit un JPEG à côté de la photo, la référence à gauche, la photo réglée à droite.
- **Séparation avant / après tournée** (Alt+Y) : l'avant en haut, l'après en bas, la
  séparation se glisse de haut en bas ; pour les photos en hauteur.
- **L'avant / après en une image** : « Exporter l'avant / après » (dans la vue côte à
  côte) écrit un JPEG à côté de la photo, la photo sans ses retouches à gauche (sa
  géométrie gardée), retouchée à droite, à partager. En ligne de commande :
  `luma develop --before-after`.
- **Curseurs plus réactifs** : le tampon de travail des flous gardé d'un flou à
  l'autre, aux mêmes pixels : l'accentuation de l'aperçu passe d'environ 80 à 60 ms,
  un rendu de réglage à 3072 px de 14 % plus rapide.
- **Un instantané comparé à maintenant** : « Comparer » à côté d'un instantané ouvre la
  vue côte à côte avec lui à gauche (son nom au‑dessus), la photo telle qu'elle est
  réglée à droite : pour choisir entre deux versions d'une retouche. Y referme.
- **Accorder sur la référence** : avec une photo de référence affichée, « Accorder »
  donne à la photo la répartition de couleurs de la référence (clarté, teintes,
  saturation : moyennes et étendues en OKLab, sans IA), en un film « Accord … » à
  l'intensité réglable, enregistré pour les photos suivantes. En ligne de commande :
  `luma film-lut --reference <réf> <photo>`.
- **Grille de composition sur la photo** (H) : les guides du recadrage (tiers, nombre
  d'or, diagonales, triangles et spirale d'or, grille) posés sur la photo pendant le
  réglage, pour juger un cadrage, un horizon, des verticales ; Maj+O les tourne.
- **Vignettes plus rapides** : les aperçus intégrés aux RAW décodés sur plusieurs
  cœurs, aux mêmes pixels : un quart à un tiers de temps en moins par vignette (ORF 37 →
  26 ms, ARW 27 → 18 ms, CR3 20 → 13 ms).
- **Photo de référence** : clic droit sur une vignette, « Utiliser comme référence » :
  elle s'affiche entière à gauche, avec ses propres réglages, la photo qu'on règle à
  droite ; pour accorder une série sur une photo réussie. Maj+Y (ou « Référence ») la
  cache et la remontre.
- **Avant / après côte à côte** (Y, ou « Côte à côte » dans la barre du haut) : la
  photo sans ses retouches à gauche d'une séparation, la photo retouchée à droite ; on
  glisse la séparation. L'« avant » garde la géométrie (recadrage, Transformation,
  Distorsion) : les deux moitiés se raccordent exactement.
- **Défrangeage automatique plus rapide** : violet et vert mesurés ensemble, les
  réglages essayés plusieurs à la fois, aux mêmes choix : 356 → 201 ms sur la photo où
  il faut tout essayer, 287 → 159 ms sur de vraies franges de synthèse.
- **Défrangeage automatique** (Corrections de l'objectif) : les franges violettes et
  vertes de l'aberration chromatique longitudinale cherchées le long des contours, là
  où ils sont plus colorés que leurs alentours, et le plus petit défrangeage qui les
  retire réglé ; une vraie couleur d'objet au bord, des feuilles sur le ciel, n'est
  pas touchée.
- **Corrections d'après le JPEG du boîtier plus rapides** : un tiers de temps en moins
  par photo (≈ 860 → 580 ms), aux mêmes valeurs : le JPEG décodé et ses détails
  cherchés une fois pour les trois mesures.
- **Mesure des pellicules et du rendu du boîtier plus rapide** : le lissage de la LUT
  (400 tours sur 35 937 nœuds) passe de 75 à 41 ms, aux mêmes bits.
- **La distorsion et le vignetage des objectifs Canon, Nikon, Pentax…** : « Corrections
  d'après le JPEG du boîtier » (Corrections de l'objectif, quand le fichier n'en porte
  pas que LÜMA lise) compare la photo au JPEG que le boîtier a rangé dans le RAW, déjà
  corrigé de son objectif, et règle la Distorsion et le Vignetage pour la même
  géométrie et les mêmes bords ; les bords vides sont retirés si la photo n'est pas
  recadrée. Sans IA : les mêmes détails trouvés des deux côtés, ajustés par moindres
  carrés ; pour le vignetage, la courbe de tons du boîtier apprise au centre, puis
  défaite vers les bords. Sur toutes les photos choisies d'un coup (bouton de la
  sélection), en arrière‑plan, chacune mesurée pour elle‑même (la focale change tout).
  En ligne de commande : `luma lens <raw|dossier>… [--write]`.
- **Le coup d'œil à l'ouverture deux à trois fois plus rapide** : l'aperçu que le
  boîtier a rangé dans le RAW, réduit sur tous les cœurs, aux mêmes pixels. À 2048 px :
  RAF 177 → 77 ms, NEF 165 → 77 ms, DNG 284 → 140 ms, ORF 201 → 61 ms ; la mesure du
  rendu du boîtier en profite aussi.
- **Le rendu du boîtier, mesuré** : « Mesurer le rendu du boîtier » (Films › Mes
  pellicules) compare le RAW ouvert au JPEG que le boîtier y a rangé et en fait un film,
  nommé d'après le boîtier (« Sony ILCE-7M3 JPEG ») : ses RAW rendus comme il le fait,
  à intensité réglable. Sans IA : la mesure des pellicules, appliquée à la même prise.
  Sur les sept marques du corpus, l'écart au JPEG du boîtier passe de 8 à 29 à moins
  de 2, en une demi‑seconde. Avec d'autres RAW du même boîtier choisis (douze au
  plus), la mesure les prend tous : plus de couleurs, un film plus juste sur les photos
  suivantes ; ceux d'un autre boîtier sont laissés de côté. En ligne de commande :
  `luma film-lut --camera`.
- **Tout remettre à zéro** : dans le menu de la langue (« ••• » sur iPhone), après une
  confirmation qui dit ce qui change et ce qui reste : les réglages des photos choisies
  et de la photo ouverte sont effacés (notes, mots‑clés et fichiers gardés ; Annuler
  les rend à la photo ouverte), et la langue, les raccourcis, les panneaux, les filtres
  et les choix d'export, d'impression, de diaporama, de livre et de galerie reviennent
  comme au premier lancement. La photothèque, les préréglages et la licence restent.
- **Arbre des dossiers plus léger** : refait seulement quand la photothèque change, et
  non plus à chaque image affichée (0,7 ms par image en moins sur 2 000 dossiers).
- **Importer depuis l'app Fichiers sur iPhone et iPad** : « Depuis Fichiers » sous
  « Importer des photos » (et dans le menu « ••• ») ouvre l'app Fichiers : les RAW d'une
  carte mémoire branchée au lecteur, d'iCloud Drive ou d'un disque arrivent dans LÜMA,
  plusieurs à la fois.
- **« Se connecter avec Apple » sur iPhone et iPad** : LÜMA Premium s'ouvre pour les
  personnes à qui il est offert, en se connectant avec leur compte Apple depuis l'écran
  Premium ; le serveur vérifie le jeton d'Apple et le courriel dans la liste des accès
  offerts.
- **LÜMA pour Linux** : le même moteur et le même rendu que sur Mac, avec le même
  abonnement. Une archive qui s'installe sans mot de passe (`./install.sh`, avec son
  lanceur dans le menu des applications et l'ouverture des RAW par « Ouvrir avec »), ou
  un paquet `.deb` pour Debian et Ubuntu ; x86_64, Ubuntu 22.04, Debian 12, Fedora 36
  ou plus récent, avec un pilote graphique Vulkan. La ligne de commande `luma` est
  comprise.
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
  courts (24 Mpx : 372 → 299 ms) et 144 Mo de mémoire en moins, mêmes octets. Puis
  l'export par lot avec un logo : le logo lu une seule fois pour tout le lot (40 ms de
  moins par photo pour un logo de 1200 × 600). Puis la conversion vers l'espace de
  sortie (JPEG), huit pixels à la fois : 24 Mpx en sRGB 230 → 150 ms,
  mêmes octets ; et pour les TIFF et PNG en 16 bits : sRGB environ 35 % plus court
  (24 Mpx : 295–366 → 190–204 ms), mêmes octets. Puis la couleur avec le mélangeur
  et l'étalonnage : les passages vers OKLab et retour huit pixels à la fois, environ
  30 % plus courts (aperçu 2048 px : 50–60 → 34–43 ms), mêmes bits. Puis la tonalité
  locale (hautes lumières, ombres, clarté, texture) : ses trois bases calculées en même
  temps, 12 à 18 % plus courte (aperçu 2048 px : 48–55 → 42–45 ms), mêmes pixels.
  Puis la réduction du bruit par ondelettes, sans image neuve à chaque échelle : 1,8 fois
  plus rapide (24 Mpx : 1,17 → 0,65 s), et l'accentuation par déconvolution, huit pixels
  à la fois : 28 % plus courte (0,83 → 0,60 s), mêmes pixels. Puis le filtre qui suit
  les contours (Ombres, Hautes lumières, Clarté, bords des masques et du ciel), sans
  plans intermédiaires : presque deux fois plus rapide (24 Mpx : 87 → 47 ms), mêmes
  valeurs. Puis l'export JPEG, converti en 8 bits bande par bande : 72 Mo de mémoire en
  moins pour 24 Mpx, mêmes fichiers octet pour octet. Puis Hautes lumières, Ombres,
  Clarté et Texture, appliquées en une seule passe : 15 à 20 % plus rapides en pleine
  définition (RAF 26 Mpx : 0,84–0,92 → 0,72 s), mêmes pixels. Puis la réduction du
  bruit de luminance : 104 Mo de mémoire en moins pour 26 Mpx, mêmes pixels. Puis le
  vignetage : 37 % plus court (24 Mpx : 108 → 68 ms), presque deux fois plus rapide
  avec une rondeur (400 → 210 ms), mêmes pixels. Puis Supprimer (en cercle ou en
  trait) : 28 % plus court, puis encore 22 % (presque deux fois plus rapide en tout),
  mêmes pixels. Puis l'agrandissement à l'export : 18 % plus court (6 → 24 Mpx :
  440 → 360 ms), mêmes pixels. Puis le poids maximal d'un JPEG, trouvé en trois
  codages le plus souvent au lieu de huit : l'export sous 2 000 Ko d'un RAW de 24 Mpx
  17 % plus court (3,95 → 3,3 s), mêmes fichiers.
- **Plus de carré vide à l'écran** : deux descriptions de films et le message
  « Pellicule mesurée » contenaient des caractères absents de la police de l'app ; un
  essai vérifie désormais chaque texte de l'interface, dans les trois langues.
- **Fiabilité** : relecture complète du code. Plus de hautes lumières rendues noires
  quand les Blancs montent fort ; plus de plantage sur un fichier HEIC, ICC ou DNG
  malformé, une image d'un pixel de large, une retouche hors de la photo, un recadrage
  à proportions verrouillées tiré au plus petit, un panorama démesuré. Un export n'écrase
  plus jamais une photo originale ; l'import ne remplace plus un XMP de la photothèque ni
  un fichier du second disque, et garde le XMP de chaque photo d'une prise ; restaurer la
  plus ancienne sauvegarde du catalogue fonctionne, et un catalogue illisible est gardé
  dans les sauvegardes. Dans l'app, l'aperçu de la photo d'avant ne recouvre plus celle
  qu'on ouvre, les masques à bord adouci s'affichent comme ils s'exportent, et un GPU qui
  ne répond plus laisse le processeur finir l'aperçu. Deux essais montent la garde :
  des RAW abîmés (copie coupée, octets brouillés, fichier vide) s'ouvrent en image ou en
  erreur, jamais en plantage, et des réglages aux valeurs aberrantes (infinies, énormes)
  se rendent en pixels sains, dans les masques, les retouches et la fluidité aussi. Une
  suppression tracée longue et très large ne fige plus l'export (24 Mpx : 2 minutes →
  4 secondes), mêmes pixels. Dans l'app, un calcul qui échoue sur une photo singulière
  ne fige plus rien : l'aperçu suivant arrive, l'ouverture, l'export, la fusion ou la
  mesure d'une pellicule disent l'erreur, et un lot d'export continue avec les autres.
  Plus de réglages perdus d'un coup : un seul masque, une seule retouche ou un seul coup
  de fluidité illisible dans un XMP (une forme d'une version plus récente, une valeur
  abîmée) n'efface plus les autres ; un préréglage d'export abîmé ne fait plus perdre
  les autres, et un fichier de préréglages illisible est gardé de côté
  (`Exports.json.illisible`) avant d'être remplacé. De même pour les instantanés d'une
  photo, les réglages par défaut par boîtier et les modèles de métadonnées : un seul
  élément abîmé n'emporte plus les autres. Le XMP d'une photo s'écrit d'un coup (un
  fichier temporaire renommé) : une app arrêtée ou un disque plein pendant l'écriture
  laisse l'ancien XMP entier, jamais un XMP coupé ; de même pour un préréglage
  enregistré, le XMP d'une fusion et l'état de la licence (une clé ou un achat ne se
  perd plus sur un fichier coupé). Les exports aussi (JPEG, PNG, TIFF) : un export
  coupé ne laisse plus un fichier abîmé sous le nom voulu, ni n'abîme l'export d'avant
  du même nom. Et les pellicules mesurées (`.cube`) : une pellicule ne disparaît plus
  de « Mes pellicules » sur un enregistrement coupé.
- **Plus de place perdue** : un doublon de travail ou une vignette dont l'écriture
  échoue (disque plein) ou s'interrompt (app fermée) ne laisse plus de fichier à moitié
  écrit ; le ménage retire ceux d'avant (sur la machine d'essai : 5,3 Go retrouvés).
- **Corrections d'objectif toujours appliquées** : un doublon de travail écrit par une
  version d'avant la lecture des corrections d'objectif du fichier (Sony, Fujifilm,
  Olympus, Panasonic, DNG) est refait une fois, au lieu de montrer la photo sans elles.
- **Distorsion des DNG plus rapide** : la correction de la distorsion écrite dans un
  DNG (téléphones, boîtiers qui enregistrent en DNG) environ 35 % plus rapide (6 Mpx :
  230 → 150 ms), sans changer un pixel.
- **Diaporama exporté : transitions et musique** : au choix, le fondu, le glissement
  (la photo suivante pousse la précédente) ou un fondu avec un lent zoom sur chaque
  photo ; une musique (MP3, AAC, Ogg, WAV, FLAC) copiée dans le dossier et jouée en
  boucle, mise en pause avec le diaporama, coupée par M (« Livre, diaporama et
  galerie », `luma slideshow --transition --music`). Le diaporama en plein écran de
  l'app passe d'une photo à l'autre avec la même transition (avant : d'un coup). Le
  libellé « Fondu » du diaporama est aussi bien traduit en anglais et en espagnol.
- **Import photo par photo** : « Choisir photo par photo » montre l'aperçu de chaque
  photo de la carte (celui intégré au RAW, lu en arrière‑plan) ; un clic en laisse une
  sur la carte, avec son JPEG ; « Toutes », « Aucune » ; avec les jours choisis
  (`luma import --skip photo`).
- **Curseurs plus vifs** : chaque rendu de l'aperçu copiait sa base à part, sur un
  seul cœur ; la copie se fait maintenant dans la première passe, sur tous les cœurs.
  Un rendu aux réglages de base 4 à 23 % plus court selon le boîtier (DNG d'iPhone
  120 → 97 ms, ORF 86 → 67 ms), sans changer un pixel.
- **La carte éjectée après l'import** (Mac, Linux) : « Éjecter la carte après
  l'import », une fois chaque photo copiée et vérifiée (`luma import --eject`) ; si le
  système refuse (une app lit encore la carte), l'import le dit, sans échouer.
- **Import sous Windows** : la carte branchée est trouvée seule (un lecteur de D: à Z:
  avec un dossier DCIM), et éjectée après l'import comme sur Mac et Linux. Les cartes
  sont cherchées en arrière‑plan, toutes les deux secondes tant que la fenêtre est
  ouverte : une carte branchée après coup apparaît, et un lecteur réseau absent ne
  fige plus la fenêtre.
- **Pages du livre** : « Pages du livre… » montre chaque page du livre photo en petit,
  ses photos à leur place ; glisser une photo sur une autre les échange, et le PDF suit
  ce nouvel ordre (la couverture comprise). Sous chaque page, 1, 2 ou 4 : ses photos ;
  les pages d'avant gardent les leurs, celles d'après suivent (« Disposition
  automatique » pour revenir ; `luma book --layout 4,1,2`).
- **Déplacer une photo du livre** : lâchée sur le fond d'une page (hors de ses photos),
  une photo en devient la première, les autres se décalent d'une place ; lâchée sur une
  photo, elles s'échangent toujours. Chaque photo de « Pages du livre » se nomme
  aussi pour les lecteurs d'écran (« fichier · Page 2 »).
- **Hautes lumières, ombres, clarté et étalonnage plus vifs** : leurs derniers calculs
  huit pixels à la fois, et les repères de la photo (centiles) cherchés deux fois plus
  vite ; un rendu 3 à 15 % plus court quand ces réglages bougent (ombres d'un RW2 :
  78 → 66 ms ; étalonnage d'un ARW : 68 → 60 ms), sans changer un pixel.
- **DNG d'iPhone deux fois plus vite à l'ouverture** : un DNG déjà en RGB (ProRAW) se
  réduit directement depuis ses pixels, sans copier d'abord toute l'image (145 Mo) ; et
  un quart de tour se fait par carreaux. Premier aperçu 220 → 109 ms, sans changer un
  pixel ; toute photo en portrait gagne aussi sur l'orientation (15 → 9 ms).
- **Les masques d'un XMP venu d'ailleurs** : un XMP écrit par un autre logiciel de
  développement apporte ses dégradés linéaires et radiaux
  (`crs:MaskGroupBasedCorrections`, et les anciennes listes de dégradés), avec leur
  intensité, leur inversion et leurs réglages locaux (exposition, contraste, hautes
  lumières, ombres, blancs, noirs, température, teinte, saturation, clarté, texture,
  netteté, bruit, voile), ramenés dans le recadrage. Les pinceaux et les masques de
  sujet, faits de pixels, ne sont pas repris.
- **Trace GPX sur iPhone et iPad** : « Suivre une trace GPX… » ouvre l'app Fichiers
  (iCloud Drive, une montre, une app de randonnée) ; la trace choisie place les photos
  sans position, comme sur Mac.
- **Premier aperçu plus rapide** : la réduction du capteur qui le prépare ne cherche
  plus la couleur de chaque photosite (une table par place dans le motif, et un chemin
  à part pour les capteurs Bayer) : 30 % plus courte (X‑Trans 90 → 58 ms) ; le premier
  aperçu 4 à 19 % plus court (RAF 216 → 175 ms, NEF 127 → 110 ms), sans changer un
  pixel.
- **Impression en CMJN** : un profil d'imprimante CMJN (une presse, un imprimeur) est
  accepté pour l'impression et le livre photo ; les pages sont converties vers ses
  quatre encres (noir compensé) et le PDF est en CMJN, ses encres gardées sans perte
  (« Imprimer… », `luma print --profile`, `luma book --profile`).
- **Ouvrir le PDF fait** : « Ouvrir … » ouvre le dernier PDF, livre, diaporama ou
  galerie avec l'app du système (le lecteur de PDF, d'où l'on imprime ; le navigateur) ;
  la fenêtre « Imprimer » reste ouverte une fois le PDF fait. Sous Windows, les liens
  (compte, paiement) s'ouvrent enfin : LÜMA y lançait un programme de Linux.
- **Diaporama dans un ordre aléatoire** : « Ordre aléatoire », dans l'app (chaque
  photo une fois avant un nouveau tirage) et dans le diaporama exporté (un nouveau
  tirage à chaque ouverture de la page ; `luma slideshow --shuffle`).
- **Première photo ouverte plus vite** : les bases de boîtiers et d'objectifs du
  décodeur (≈ 75 ms, une fois par séance) se chargent au démarrage de l'app, pendant
  que sa fenêtre s'ouvre, et non plus à la première photo : 60 à 95 ms de moins pour
  elle (NEF 173 → 80 ms, RAF 176 → 90 ms, ARW 160 → 98 ms).
- **Pages en JPEG** : l'impression et le livre photo s'enregistrent aussi en JPEG, une
  image par page (« JPEG » dans la fenêtre d'enregistrement, `-o planche.jpg` :
  `planche-1.jpg`, `planche-2.jpg`…), à la résolution choisie et en sRGB, pour un
  service d'impression de livres ou pour partager une planche contact.
- **Les notes déjà posées ailleurs** : sans XMP à côté, LÜMA lit celui que le fichier
  porte en lui (un JPEG, un TIFF, un DNG, un PNG noté par un autre logiciel ; un RAW
  noté sur le boîtier) : sa note, son libellé et ses mots‑clés entrent dans la
  photothèque et dans l'app. Un XMP à côté l'emporte toujours.
- **Export plus léger en mémoire** : en pleine définition (un export, le zoom 1:1), le
  rendu se fait dans l'image de base elle‑même au lieu d'une copie : le pic de mémoire
  d'un export baisse de 19 à 23 % (24 Mpx : 1,24 → 0,96 Go), les fichiers restent
  identiques octet pour octet.
- **Les réglages qu'un RAW porte en lui** : un RAW ou un DNG développé par un autre
  logiciel, qui a rangé ses réglages dans le fichier, s'ouvre avec eux quand il n'a pas
  de XMP à côté (dans l'app et `luma develop`, `print`, `book`) ; jamais ceux d'un JPEG,
  déjà dans ses pixels. La note et le libellé écrits en éléments sont lus aussi, et le
  XMP intégré n'est lu qu'une fois par fichier (64 Ko au plus, gardés en mémoire).
- **Entretien de la photothèque dans l'app** : « Entretien… », sous les dossiers,
  liste les photos manquantes dossier par dossier ; un dossier se retrouve en
  choisissant sa nouvelle place (quand tout le dossier parent a bougé, les dossiers
  voisins suivent d'eux‑mêmes) ou se retire de la photothèque, collections comprises.
  Les sauvegardes du catalogue s'y voient par date : une de plus à la demande, ou une
  ancienne remise en place après confirmation (la photothèque du moment est
  sauvegardée d'abord).
- **Aperçus plus rapides sur le processeur** : l'image de l'aperçu d'avant sert de
  mémoire au suivant (plus de mémoire neuve à chaque retouche, 64 Mo gardés au plus),
  et la conversion vers l'écran se fait d'un coup, sur tous les cœurs : un aperçu de
  4,4 Mpx 10 à 15 % plus vite, la conversion d'une image de 24 Mpx 38 % plus vite
  (zoom 1:1, comparaison, aperçus des films) ; les mêmes pixels, octet pour octet.
- **Envoyer à l'imprimante** : dans « Imprimer… », le PDF fait part directement vers
  une imprimante du système (la sienne par défaut d'abord), avec le nombre
  d'exemplaires, sans passer par le lecteur de PDF ; sur Mac et Linux, par le service
  d'impression du système. Sans imprimante, l'app le dit et « Ouvrir » reste là.
- **Les photos par date** : le panneau Photothèque range toutes les photos par année
  (la plus récente d'abord, avec leur nombre) ; un triangle déplie les mois ; un clic
  sur une année ou un mois montre ses photos.
- **Conversion 8 bits plus rapide** : chaque octet d'un JPEG ou de l'aperçu se lit
  dans une table d'octets au lieu d'être interpolé puis arrondi (19 % de moins pour
  24 Mpx, à l'export comme à l'écran) ; les mêmes octets, vérifiés pour chacun du
  milliard de nombres possibles entre 0 et 1.
- **Dates par rapport à aujourd'hui** : la recherche et les collections dynamiques
  comprennent `date:30j` (les trente derniers jours, aujourd'hui compris),
  `date:aujourd'hui` et `date:hier` (`30d`, `today`, `yesterday` en anglais) ; une
  collection « date:7j » suit les jours.
- **Trouver ce qui reste à faire** : la recherche et les collections dynamiques
  prennent la note en plage ou exacte (`note:..2`, `note:2..4`, `note:=0` pour les
  photos pas encore notées) et `sans:` : `sans:note`, `sans:mot` (aucun mot‑clé),
  `sans:lieu` (pas de position), `sans:libellé`, à combiner.
- **JPEG et PNG cinq fois plus vite à ouvrir** : une image de 8 bits par canal (JPEG,
  PNG, TIFF, WebP, PSD…) passe en valeurs linéaires par une table de ses 256 valeurs
  (et de ses opacités), sur tous les cœurs, sans image intermédiaire en flottants :
  un JPEG de 24 Mpx s'ouvre en 0,18 s au lieu de 0,9 s ; les mêmes valeurs, au bit
  près. De même les images opaques de 16 bits par canal (TIFF, PNG), par une table
  de leurs 65 536 valeurs : un TIFF 16 bits de 24 Mpx en 0,25 s au lieu de 0,95 s.
- **« À faire » dans la photothèque** : le panneau Photothèque compte les photos pas
  encore notées, sans mot-clé et sans lieu ; un clic les montre (une ligne disparaît
  quand il n'en reste plus).
- **Toutes les écritures à l'écran** : un nom de fichier, un mot‑clé ou une légende en
  japonais, en chinois, en coréen, en arabe ou en hébreu s'affiche avec une police du
  système (Mac, Windows, Linux), trouvée à la première lettre qui manque, au lieu de
  petites boîtes vides ; rien n'est chargé tant que rien ne manque.
- **Collections dynamiques depuis l'app** : la recherche affichée s'enregistre comme
  collection dynamique (« Enregistrer comme collection… », un nom, Entrée), qui la
  refait à chaque fois ; un clic droit sur une collection la supprime (ses photos
  restent). Jusqu'ici, seule la ligne de commande le permettait.
- **Collections manuelles depuis l'app** : le menu « Collection » range la photo
  ouverte et les photos choisies dans une collection (une existante, ou une nouvelle,
  nommée sur place), et les retire de la collection affichée.
- **PNG 16 bits plus rapides et plus légers** : écrits par LÜMA sur tous les cœurs
  (l'image en bandes filtrées et compressées chacune de son côté, un seul flux) :
  l'export d'une photo de 24 Mpx en PNG 16 bits prend 1,8 à 2,0 s au lieu de 2,5 à
  2,7 s, pour un fichier 10 % plus léger ; les mêmes pixels. Le PNG 8 bits garde son
  codeur, déjà le plus rapide et le plus léger (mesuré).
- **Les mots‑clés dans la photothèque** : une section « Mots-clés » du panneau
  Photothèque, repliée, liste chaque mot‑clé avec son nombre de photos ; un clic
  montre ses photos. La recherche garde les espaces entre guillemets
  (`mot:"coucher de soleil"`, guillemets droits, courbes ou chevrons).
- **Les boîtiers et les objectifs dans la photothèque** : deux sections repliées,
  « Boîtiers » et « Objectifs », comme les mots‑clés : chaque boîtier et chaque
  objectif avec son nombre de photos, un clic montre ses photos (le nom entier, espaces
  compris).
- **Photothèque plus vive** : les listes du panneau (dossiers, mots‑clés, boîtiers,
  objectifs) se comptent sans copier chaque valeur de chaque photo ; pour 100 000
  photos, leur mise à jour après un changement passe d'environ 200 ms à 90 ms ; les
  mêmes listes. Les nombres de « À faire » et des collections se comptent sans trier
  les photos : 11 ms au lieu de 36 ms pour les trois lignes de « À faire ».
- **Un mot‑clé renommé ou supprimé partout** : dans la liste des mots‑clés, un clic
  droit renomme un mot‑clé dans toutes les photos qui l'ont, ou l'en retire ; leurs
  XMP sont réécrits (la note, le drapeau et le reste ne bougent pas), et une photo qui
  avait déjà le nouveau nom ne l'a pas deux fois.
- **Une collection renommée** : le clic droit sur une collection la renomme (le nom
  s'écrit sur place) ; elle garde ses photos et sa règle, reprend sa place dans l'ordre
  alphabétique, et reste affichée si elle l'était ; un nom déjà pris est refusé.
- **Mots‑clés et collections en ligne de commande** : `luma catalog` renomme un
  mot‑clé dans toutes ses photos (`--rename-keyword ancien nouveau`) ou l'en retire
  (`--delete-keyword`), les XMP réécrits sans rien perdre d'autre ; renomme une
  collection (`--rename-collection`) et en retire une photo (`--remove-from`) : ce
  que fait l'app, par le même code.
- **Notes et libellés dans la photothèque** : deux sections repliées de plus,
  « Notes » (le nombre de photos de 5 à 1 étoile) et « Libellés » (chaque libellé et
  son nombre de photos) ; un clic montre leurs photos.
- **Catalogue plus rapide à lire et à écrire** : pour 100 000 photos, l'écriture
  passe de 59 à 43 ms et la lecture de 108 à 78 ms (un hachage rapide pour la table
  des textes, le numéro du dossier, du boîtier et de l'objectif de la photo d'avant
  repris sans recherche) ; le fichier est le même, octet pour octet.
- **Un dossier retiré de la photothèque** : le clic droit sur un dossier du panneau
  Photothèque le retire de la photothèque (ses photos ne sont plus suivies, ni dans
  les collections ; sous‑dossiers compris ; les fichiers ne sont pas touchés) ou
  l'ouvre dans le gestionnaire de fichiers du système.
- **La liste des photos en CSV** : « Exporter la liste (CSV)… » (et
  `luma catalog --csv`) écrit les photos montrées — une collection, une recherche, une
  date — en tableau pour un client ou des archives : fichier, date, boîtier, objectif,
  ISO, ouverture, vitesse, focale, note, drapeau, libellé, mots‑clés, position.
- **Vues de la photothèque plus rapides** : en montrant toutes les photos, une
  collection ou une recherche, l'app regarde si chaque photo est encore là sur tous
  les cœurs (et de même pour les photos manquantes de « Entretien… ») : 3 fois plus
  vite sur un disque local (20 000 photos : 17 → 5 ms), davantage sur un disque
  externe ou en réseau ; les mêmes photos, dans le même ordre.
- **Les dossiers en arbre** : le panneau Photothèque range les dossiers en arbre ;
  chacun avec les photos de tout son arbre, un triangle qui déplie ses sous‑dossiers,
  et les chemins sans photo fondus en une ligne (« Photos/2024 » plutôt que chaque
  marche du disque). Retiré de la photothèque, un dossier emporte ses sous‑dossiers.
- **Chercher par dossier** : `dossier:mariage` (ou `folder:`) garde les photos dont
  le chemin du dossier contient ce texte, majuscules et accents mis à part, dans la
  recherche et les collections dynamiques ; dans l'arbre des dossiers, le clic droit
  « Toutes ses photos » montre un dossier et tous ses sous‑dossiers d'un coup.
- **LÜMA pour Windows** : Windows 10 et 11 (x64), rendu par DirectX 12 (ou Vulkan) ;
  un installateur sans mot de passe administrateur (menu Démarrer, désinstallation) et
  une archive .zip à décompresser n'importe où, avec la ligne de commande `luma.exe`.
  Réglages, catalogue et préréglages dans `%APPDATA%\LÜMA`, caches dans
  `%LOCALAPPDATA%\LÜMA`. Fabriqués, installés, lancés avec un vrai RAW et désinstallés
  à chaque push par la CI.
- **Un mois d'essai partout** : sur Mac, Linux et Windows aussi, l'essai complet dure
  désormais un mois (14 jours auparavant), comme sur iPhone et iPad.
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
- **Courbes à points dans l'app** (Lumière, Courbe des tonalités) : luminosité,
  rouge, vert et bleu ; un clic ajoute un point, on le glisse, un double‑clic le
  retire.
- **Poids maximal d'un JPEG** (Exporter, « Poids maximal », ou `--max-kb 500`) : la
  meilleure qualité qui tient dans le poids, métadonnées comprises, pour un site ou un
  formulaire qui limite la taille des fichiers ; gardé dans les préréglages.
- **Correcteur et tampon tracés** (Retouche) : glisser le long d'une ride ou d'une
  rayure la reprend d'un seul geste, la source suivant le trait.
- **Taille de tirage** (Exporter, Dimensions, « Tirage (cm) », ou
  `--resize 30x40cm@300`) : le cadre en centimètres et la résolution en points par
  pouce, inscrite dans le JPEG, le PNG et le TIFF pour l'imprimeur.
- **Dupliquer un masque** (Masques) : une copie, ou une copie inversée aux réglages à
  zéro, pour régler le sol après le ciel sans retracer la zone.
- **Suppression tracée** (Retouche, Supprimer) : glisser le long d'un fil, d'une rayure
  ou d'un câble l'efface d'un seul geste, remplacé par la photo qui l'entoure. Sans IA.
- **Tons auto** (Lumière, Tonalité, ou `--auto-tone`) : un clic règle l'exposition, le
  contraste, les hautes lumières et les ombres d'après l'histogramme de la photo, un
  point de départ à affiner. Sans IA.
- **Réduction du bruit dans les masques** : lisser des ombres débouchées ou un ciel
  bruité au pinceau, en gardant les contours nets, sans adoucir le reste de la photo.
- **Netteté dans les masques** : relever le détail d'un regard ou d'une matière au
  pinceau, ou adoucir un fond, sans toucher au reste de la photo.
- **Dématriçage X‑Trans directionnel** (Fujifilm) : le vert suit les contours au lieu de
  les moyenner en travers, jusque dans les ombres bruitées (3 dB de mieux sur nos
  essais). Les doublons de travail sont refaits une fois.
- **Texture et Correction du voile dans les masques** : adoucir une peau ou un fond au
  pinceau, déboucher un ciel voilé dans un dégradé, sans toucher au reste de la photo.
- **Agrandir au besoin** (Exporter, Dimensions, ou `--enlarge`) : une taille plus grande
  que la photo l'agrandit, jusqu'à quatre fois chaque côté, nette et sans halo autour
  des contours ; pour les grands tirages. Sans IA.
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
- **Les copies virtuelles dans la photothèque** : elles apparaissent aussi dans « toutes
  les photos », les recherches et les collections, avec la date et le boîtier de leur
  original et leur note à elles ; une copie supprimée en sort.
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
- **Doublons** (`luma catalog --duplicates`) : les photos en double du catalogue — même
  boîtier, même instant, fichiers de même taille et de même type, comme une carte
  importée deux fois — listées avec la place qu'elles prennent, sans rien effacer.
- **Carte** (Bibliothèque, « Carte ») : les photos du film à l'endroit de leur prise
  de vue (position GPS de l'EXIF, gardée dans la photothèque), regroupées quand elles
  sont proches ; glisser, zoomer à la molette ou à deux doigts, toucher un groupe pour
  voir ses photos. Fond de tuiles libres, téléchargées par le système et gardées sur
  l'appareil ; hors ligne, celles déjà gardées.
- **Les photos du téléphone à leur date et sur la carte** : l'EXIF des images
  développées (JPEG, HEIC, AVIF, PNG, WebP, TIFF) est lu comme celui d'un RAW — date de
  prise de vue, boîtier, objectif, réglages, position GPS. Les photos déjà dans la
  photothèque sont relues une fois.
- **Une trace GPX pour placer les photos** (`luma geotag --gpx trace.gpx`) : chaque
  photo prend la position de la trace à l'instant de sa prise de vue, écrite dans son
  XMP ; l'écart de l'horloge du boîtier se donne avec `--offset`, ou se devine
  (`--offset auto`). La photothèque et la carte la montrent aussitôt. Dans l'app sur Mac :
  « Suivre une trace GPX… » sur la carte, l'écart deviné.
- **Placer une photo sur la carte à la main** : sur la carte, la photo affichée qui n'a
  pas de position montre un viseur au centre ; on fait glisser la carte dessous, puis
  « Placer ici » l'inscrit dans son XMP et dans la photothèque. Les photos choisies qui
  n'ont pas de position se placent d'un coup (« Placer les 12 photos ici »).
- **Impression** (« Imprimer… » sur les photos choisies, `luma print`) : un PDF à la
  taille exacte du papier (A4, A3, A3+, 10 × 15 cm…), portrait ou paysage, marges, une
  photo par page ou une **planche contact** (grille, nom sous chaque photo), photos
  entières ou remplissant leur case, accentuation pour papier mat ou brillant, aperçu
  de la page ; les pages converties vers le **profil de l'imprimante** (celui de
  l'épreuvage, noir compensé), à imprimer sans gestion des couleurs du pilote.
- **Repères de coupe et texte au bas de la page** (« Imprimer… », `luma print
  --crop-marks --text`) : des traits fins aux coins de chaque photo, pour couper au
  massicot, et une ligne de texte (un titre, un nom, des droits) dans la marge du bas.
- **Livre photo** (« Livre, diaporama, galerie… », `luma book`) : un PDF avec sa couverture
  (la première photo, le titre et le sous‑titre), puis des pages d'une, deux ou quatre
  photos choisies d'après leur orientation ; formats 20 × 20, 30 × 30, A4, 21 × 28, A3.
- **Des pages de texte dans le livre** (« Livre, diaporama, galerie… », `luma book
  --opening --closing`) : une page après la couverture (une dédicace, un mot
  d'introduction) et une dernière page (un lieu, une date, des remerciements), leurs
  lignes centrées au milieu de la page. Et la disposition au choix : le nombre de photos
  de chaque page (« 1,2,4 », répété ; `--pages`).
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
- **N'importer que certains jours** : la fenêtre d'import liste les jours de prise de vue
  de la carte, chacun avec son nombre de photos ; on décoche ceux qu'on ne veut pas
  (`luma import --days 2026-10-26,2026-10-27`).
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
