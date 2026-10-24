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
| 4. Bêta | M13–M18 | IA locale (sujet, ciel, débruitage) sur GPU Apple, profils boîtiers/objectifs, recherche, récupération après plantage | Premier portage Vulkan/DX12 du rendu |
| 5. 1.0 | M19–M24 | Mac App Store et App Store, accessibilité, migration, documentation | — |
| 6. Expansion | M25–M36 | Vidéo (timeline, étalonnage), tethering, impression | Applications Windows, Linux et Android sur le même moteur |

## Jalons de validation

- **M3** : un RAW s'ouvre, se règle et s'exporte sur Mac et sur iPhone, sans dépendance interdite.
- **M6** : rendu identique Mac/iPhone à tolérance ΔE documentée, aperçu P95 < 100 ms sur Mac.
- **M12** : un photographe fait un flux complet (import → tri → retouche → export) sur Mac ou iPad.
- **M18** : les outils avancés passent les tests de qualité, de compatibilité et de performance.
- **M24** : version stable publiée sur les deux App Stores, matrice de compatibilité publique.

## Risques principaux

| Risque | Niveau | Réponse |
|---|---|---|
| Couverture RAW | Très élevé | Corpus réel, matrice par boîtier publiée et élargie à chaque version |
| Vidéo sans bibliothèque tierce | Très élevé | Décodage via les frameworks système Apple, ou périmètre réduit ; audit dès M0 |
| Intégration iOS en Rust (UIKit, Pencil, PhotoKit) | Élevé | Prototype dès M0, couche `luma-platform` fine et testée |
| Couleur moins fidèle que la concurrence | Élevé | Tests colorimétriques, rendus de référence |
| Mémoire sur iPhone | Élevé | Tuiles, cache borné, traitement progressif |
| Revue App Store | Moyen | Respect des règles dès l'alpha, TestFlight continu |
