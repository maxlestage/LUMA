# LÜMA — Feuille de route (Apple d'abord)

macOS et iOS/iPadOS sont les plateformes prioritaires. Windows, Linux et Android sont
compilés en CI dès le premier mois (le moteur ne doit rien supposer d'Apple), mais leurs
applications ne sont livrées qu'en phase 6.

Estimation : 24 à 36 mois pour une 1.0 photo professionnelle avec une équipe expérimentée.
C'est une estimation, pas une garantie.

| Phase | Mois | Apple (prioritaire) | Autres plateformes |
|---|---|---|---|
| 1. Faisabilité | M0–M3 | App macOS et app iOS minimales en Rust (`winit` + `objc2`), rendu `wgpu`/Metal, décodage `rawler` sur un corpus (Sony, Canon, Nikon, Fujifilm, ProRAW), signature et TestFlight automatisés | Compilation CI du moteur pour Windows, Linux, Android |
| 2. Moteur | M4–M6 | Pipeline f32, Display P3, EDR/HDR sur écrans XDR, mémoire unifiée sans copie, graphe de retouche, projet versionné | Tests du moteur sur CPU (fallback) |
| 3. Alpha | M7–M12 | Mac : photothèque, retouche, export. iPhone/iPad : import PhotoKit et Fichiers, masques à l'Apple Pencil, gestes. TestFlight public | — |
| 4. Bêta | M13–M18 | Masques par plages de couleur et de luminance, débruitage avancé (sans IA), corrections d'objectif lues dans les RAW, recherche, récupération après plantage | Premier portage Vulkan/DX12 du rendu |
| 5. 1.0 | M19–M24 | App Store (iPhone et iPad), téléchargement direct (Mac), accessibilité, migration, documentation | — |
| 6. Expansion | M25–M36 | Vidéo (timeline, étalonnage), tethering, impression | Applications Windows, Linux et Android sur le même moteur |

## Avancement

**M0 – M3, faisabilité : le moteur de développement existe.** Fait :

- workspace Cargo (`crates/`), règle « 100 % Rust » vérifiée en CI sur le graphe de
  dépendances réel ;
- lecture des RAW Bayer, X‑Trans et DNG linéaire (ProRAW), couleur au sens DNG,
  dématriçage directionnel (Bayer) et X‑Trans ;
- module Développement complet, panneau par panneau (voir
  [`fonctions.md`](fonctions.md)), lecture et écriture des XMP `crs:` ;
- export JPEG, PNG, TIFF 8/16 bits en sRGB ou Display P3 avec profil ICC ;
- 7 boîtiers développés à chaque push (Sony, Canon, Nikon, Fujifilm, iPhone, OM System, Panasonic), mesures
  de réactivité publiées ;
- LÜMA Studio, l'app Mac (`apps/studio`) : module Développement complet, historique,
  XMP automatique, film, raccourcis clavier, Bibliothèque (notes, drapeaux,
  filtre), outil Recadrage, fenêtre Exporter ; finition sur le GPU (Metal), base
  en cache ; masques linéaire, radial et pinceau ;
  corrections de l'objectif et transformation ; `LÜMA.app` universelle fabriquée par
  la CI sur un runner Apple Silicon.

Prochaines étapes, dans l'ordre :

1. rendu GPU, suite : la finition par pixel est faite (`luma-gpu`, identique au
   processeur) ; restent les étapes de voisinage (bruit, tonalité locale, accentuation)
   et l'affichage sans relecture (texture partagée avec l'interface) ;
2. dans l'app : catalogue (plusieurs dossiers, collections, mots‑clés), préréglages ;
3. masques par plages de couleur et de luminance, corrections d'objectif lues dans les RAW, redressement automatique (sans IA) ;
4. iOS : l'app tourne déjà (simulateur, CI) ; restent PhotoKit, les gestes (pincer pour
   zoomer), l'Apple Pencil pour les masques, la signature et TestFlight.

## Jalons de validation

- **M3** : un RAW s'ouvre, se règle et s'exporte sur Mac et sur iPhone, sans dépendance interdite.
- **M6** : rendu identique Mac/iPhone à tolérance ΔE documentée, aperçu P95 < 100 ms sur Mac.
- **M12** : un photographe fait un flux complet (import → tri → retouche → export) sur Mac ou iPad.
- **M18** : les outils avancés passent les tests de qualité, de compatibilité et de performance.
- **M24** : version stable publiée sur l'App Store (iPhone et iPad) et en téléchargement direct (Mac), matrice de compatibilité publique.

## Risques principaux

| Risque | Niveau | Réponse | Où on en est |
|---|---|---|---|
| Couverture RAW | Très élevé | Corpus réel, matrice par boîtier publiée et élargie à chaque version | **Fait.** [`boitiers.md`](boitiers.md) : 735 boîtiers de 20 marques, 714 à couleur étalonnée, générée depuis la base du décodeur (`luma cameras --markdown`) et vérifiée par la CI à chaque push. 7 vrais RAW développés à chaque test (Sony, Canon, Nikon, Fujifilm, iPhone ProRAW, OM System, Panasonic) ; tout DNG est lu. À élargir : un RAW réel de plus par version (Canon CR2, Pentax). |
| Vidéo sans bibliothèque tierce | Très élevé | Décodage via les frameworks système Apple, ou périmètre réduit ; audit dès M0 | **Décidé : périmètre réduit.** La v1 est un éditeur photo, sans vidéo. L'audit : décoder H.264/HEVC/ProRes en Rust pur n'est pas réaliste ; AVFoundation (système, admis) demande des liaisons Objective‑C, donc du code `unsafe`, interdit aujourd'hui dans LÜMA. Si la vidéo revient, ce sera dans une couche `luma-platform` isolée, la seule autorisée à l'`unsafe`, auditée à part. |
| Intégration iOS en Rust (UIKit, Pencil, PhotoKit) | Élevé | Prototype dès M0, couche `luma-platform` fine et testée | **Prototype fait** : l'app tourne sur iPhone et iPad (simulateur lancé à chaque push, capture vérifiée), Metal, RAW par l'app Fichiers. **Reste** : PhotoKit (photothèque), Apple Pencil (pression pour le pinceau), partage. Tous demandent des liaisons Objective‑C : même règle que la vidéo, une couche `luma-platform` isolée. |
| Couleur pas assez fidèle | Élevé | Tests colorimétriques, rendus de référence | **Tests en place** (`crates/luma-render/tests/color.rs`) : charte ColorChecker vue par un vrai capteur (Sony A7 III), ΔE 2000 < 1 en lumière du jour et sous lampe 3200 K (interpolation des matrices), teintes gardées par le développement complet ; 100 films mesurés sur mire (`luma-image/tests/presets.rs`), GPU identique au processeur. **Mesure sur vrai boîtier** : `luma colorchecker photo.RAW` (photo d'une vraie charte, recadrée) donne le ΔE 2000 de chaque plage. **Reste** : des photos de chartes réelles dans le corpus, et des profils au‑delà des matrices (tables de teinte, profils DCP). |
| Mémoire sur iPhone | Élevé | Tuiles, cache borné, traitement progressif | **Caches bornés en octets** : 600 Mo sur iPhone et iPad, 3 Go ailleurs (`luma_render::memory_budget`), les plus anciens partent d'abord, testé. Aperçus à la taille de l'écran. **Reste** : l'export pleine définition en tuiles (aujourd'hui l'image entière en mémoire : environ 600 Mo pour 48 Mpx). |
| Revue App Store | Moyen | Respect des règles dès l'alpha, TestFlight continu | **Prêt côté app** : manifeste de confidentialité (`PrivacyInfo.xcprivacy` : aucun suivi, aucune donnée collectée), pas de chiffrement à déclarer, Info.plist complet. **Reste** : un compte Apple Developer (signature, TestFlight), et l'abonnement sur iPhone et iPad par les achats intégrés (règle de l'App Store, voir [`premium.md`](premium.md)). |
