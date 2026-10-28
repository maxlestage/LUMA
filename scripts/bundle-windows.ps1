# Fabrique LÜMA pour Windows (x86_64) dans target\windows\ :
# - LUMA-windows-x86_64\ : les exécutables luma-studio.exe et luma.exe, install.ps1
#   (installation sans droits administrateur) et un LISEZMOI.txt ;
# - LUMA-windows-x86_64.zip : la même chose, compressée. Un nom sans numéro de version :
#   les liens du site ne changent pas.
# À lancer sur Windows :  powershell -ExecutionPolicy Bypass -File scripts\bundle-windows.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
$out = 'target\windows'
$name = 'LUMA-windows-x86_64'
$root = Join-Path $out $name

cargo build --release -p luma-studio -p luma-cli
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
New-Item -ItemType Directory -Force -Path (Join-Path $root 'bin') | Out-Null

Copy-Item -Force 'target\release\luma-studio.exe' (Join-Path $root 'bin')
Copy-Item -Force 'target\release\luma.exe' (Join-Path $root 'bin')
Copy-Item -Force 'packaging\windows\install.ps1' $root

@"
LÜMA $version pour Windows (x86_64)

Installer (pour vous seul, sans droits administrateur) :
  powershell -ExecutionPolicy Bypass -File install.ps1
Désinstaller :
  powershell -ExecutionPolicy Bypass -File install.ps1 -Remove
Ou sans installer : lancez bin\luma-studio.exe

Il faut un pilote graphique à jour (Vulkan, ou Direct3D 12) ; à défaut, LÜMA bascule sur
le processeur. La ligne de commande « luma » est dans bin\luma.exe.

14 jours d'essai, complet, puis LÜMA Premium : https://maxlestage.github.io/LUMA/
"@ | Set-Content -Encoding UTF8 (Join-Path $root 'LISEZMOI.txt')

$zip = Join-Path $out "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $root -DestinationPath $zip
Write-Output $zip
