# LÜMA pour Windows

L'empaquetage de LÜMA pour Windows 10 et 11 (x86_64). Le même moteur Rust que sur Mac
et Linux : une photo a le même rendu partout.

- `install.ps1` : installe LÜMA pour l'utilisateur courant, **sans droits
  administrateur**, dans `%LOCALAPPDATA%\Programs\LUMA` ; ajoute ce dossier au PATH de
  l'utilisateur et pose un raccourci au menu Démarrer. `-Remove` désinstalle.

L'archive se fabrique avec `scripts/bundle-windows.ps1`, à lancer **sur Windows** (ou sur
le runner Windows de la CI, voir `.github/workflows/windows.yml`). Elle produit
`target/windows/LUMA-windows-x86_64.zip` : les exécutables `luma-studio.exe` et
`luma.exe`, `install.ps1` et un `LISEZMOI.txt`. Le nom de l'archive ne porte pas de
numéro de version, pour que les liens du site ne changent pas.

Le rendu passe par le processeur graphique (wgpu) : Vulkan d'abord, puis Direct3D 12. Il
faut un pilote graphique à jour ; à défaut, LÜMA bascule sur le processeur.
