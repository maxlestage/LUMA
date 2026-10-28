# Fabrique LÜMA pour Windows (x86_64) dans target\windows\ :
# - LUMA-windows-x86_64\ : les exécutables luma-studio.exe et luma.exe et l'icône (bin\),
#   install.ps1 (installation sans droits administrateur) et un LISEZMOI.txt ;
# - LUMA-windows-x86_64.zip : la même chose, compressée ;
# - LUMA-windows-x86_64-setup.exe : l'installateur (Inno Setup, s'il est là, comme sur
#   les machines Windows de GitHub), pour vous seul sans mot de passe administrateur,
#   avec le menu Démarrer et la désinstallation.
# Des noms sans numéro de version : les liens du site ne changent pas.
# À lancer sur Windows :  powershell -ExecutionPolicy Bypass -File scripts\bundle-windows.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
$out = 'target\windows'
$name = 'LUMA-windows-x86_64'
$root = Join-Path $out $name
$bin = Join-Path $root 'bin'

cargo build --release -p luma-studio -p luma-cli
if ($LASTEXITCODE -ne 0) { throw 'cargo build' }
cargo build -q --release -p luma-studio --example icon
if ($LASTEXITCODE -ne 0) { throw 'icône' }
if (Test-Path $root) { Remove-Item -Recurse -Force $root }
New-Item -ItemType Directory -Force -Path $bin | Out-Null

Copy-Item -Force 'target\release\luma-studio.exe' $bin
Copy-Item -Force 'target\release\luma.exe' $bin
Copy-Item -Force 'packaging\windows\install.ps1' $root
# L'icône (16 à 256 px), dessinée par le code (luma-brand) : raccourcis et installateur.
& 'target\release\examples\icon.exe' (Join-Path $bin 'luma.ico') windows
if ($LASTEXITCODE -ne 0) { throw 'icône' }

@"
LÜMA $version pour Windows (x86_64)

Installer (pour vous seul, sans droits administrateur) :
  powershell -ExecutionPolicy Bypass -File install.ps1
Désinstaller :
  powershell -ExecutionPolicy Bypass -File install.ps1 -Remove
Ou sans installer : lancez bin\luma-studio.exe

Windows 10 ou 11, 64 bits, avec un pilote graphique à jour (Direct3D 12 ou Vulkan) ; à
défaut, LÜMA bascule sur le processeur. La ligne de commande « luma » est dans
bin\luma.exe.

Vos réglages, le catalogue et les préréglages : %APPDATA%\LÜMA
Les caches (doublons de travail, tuiles de la carte) : %LOCALAPPDATA%\LÜMA

14 jours d'essai, complet, puis LÜMA Premium : https://maxlestage.github.io/LUMA/
"@ | Set-Content -Encoding UTF8 (Join-Path $root 'LISEZMOI.txt')

$zip = Join-Path $out "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $root -DestinationPath $zip
Write-Output $zip

# L'installateur, si Inno Setup est là.
$iscc = @(
  (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source,
  "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
  "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if ($iscc) {
  & $iscc /Qp "/DVersion=$version" "/DSource=$((Resolve-Path $root).Path)" "/O$((Resolve-Path $out).Path)" packaging\windows\luma.iss
  if ($LASTEXITCODE -ne 0) { throw 'Inno Setup' }
  Write-Output (Join-Path $out "$name-setup.exe")
} else {
  Write-Warning "Inno Setup absent : pas d'installateur, l'archive seulement."
}
