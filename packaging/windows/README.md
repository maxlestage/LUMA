# LÜMA pour Windows

L'empaquetage de LÜMA pour Windows 10 et 11 (x86_64). Le même moteur Rust que sur Mac
et Linux : une photo a le même rendu partout.

- `install.ps1` : installe LÜMA pour l'utilisateur courant, **sans droits
  administrateur**, dans `%LOCALAPPDATA%\Programs\LUMA` ; ajoute ce dossier au PATH de
  l'utilisateur et pose un raccourci au menu Démarrer. `-Remove` désinstalle.
- `luma.iss` : l'installateur (Inno Setup 6), `LUMA-windows-x86_64-setup.exe` : pour
  l'utilisateur courant sans mot de passe administrateur (dans
  `%LOCALAPPDATA%\Programs\LÜMA`), menu Démarrer, raccourci de bureau au choix,
  désinstallation depuis les Paramètres, en français, anglais et espagnol.

L'archive se fabrique avec `scripts/bundle-windows.ps1`, à lancer **sur Windows** (ou sur
le runner Windows de la CI, voir `.github/workflows/windows.yml`). Elle produit
`target/windows/LUMA-windows-x86_64.zip` : les exécutables `luma-studio.exe` et
`luma.exe` et l'icône `luma.ico` (dans `bin\`), `install.ps1` et un `LISEZMOI.txt` ;
et, si Inno Setup est là, `target/windows/LUMA-windows-x86_64-setup.exe`. Le nom de l'archive ne porte pas de
numéro de version, pour que les liens du site ne changent pas.

Le rendu passe par le processeur graphique (wgpu) : Direct3D 12 ou Vulkan. Il faut un
pilote graphique à jour ; à défaut, LÜMA bascule sur le processeur. Réglages, catalogue
et préréglages vont dans `%APPDATA%\LÜMA`, les caches dans `%LOCALAPPDATA%\LÜMA`.
