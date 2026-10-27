# Bibliothèques corrigées

Deux bibliothèques d'egui 0.37, copiées telles quelles depuis crates.io, avec une seule
correction pour l'iPhone et l'iPad (`[patch.crates-io]` du `Cargo.toml` à la racine) :

- `eframe` (`src/native/wgpu_integration.rs`, au redimensionnement) ;
- `egui-wgpu` (`src/winit.rs`, à la création de la surface).

Sur iOS, egui met en page sur tout l'écran (`window.outer_size()`, pour que le fond
passe sous la Dynamic Island ; la zone sûre est donnée à part), mais eframe donnait à la
surface la taille de la zone sûre (`window.inner_size()`). L'image, plus petite que
l'écran, était étirée dessus (11 % en hauteur sur un iPhone 17 Pro Max en portrait),
le bas de l'interface n'était jamais affiché et les touchers tombaient de plus en plus
bas que ce qu'ils visaient. La surface prend maintenant la taille de l'écran entier.

Les lignes changées portent le commentaire « LÜMA : ». À retirer quand une version
d'eframe corrigée sera adoptée.
