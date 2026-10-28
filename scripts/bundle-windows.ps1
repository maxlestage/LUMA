# Fabrique LÜMA pour Windows (x64) dans target\windows\ :
# - LUMA-windows-x64.zip : l'app, la ligne de commande, l'icône et un LISEZMOI, à
#   décompresser n'importe où (rien à installer) ;
# - LUMA-windows-x64-setup.exe : l'installateur (Inno Setup, s'il est là), pour vous
#   seul sans mot de passe administrateur, avec le menu Démarrer et la désinstallation.
# Des noms sans numéro de version : les liens du site ne changent pas.
# À lancer sur Windows : pwsh scripts/bundle-windows.ps1
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
$out = 'target\windows'
$name = 'LUMA-windows-x64'
$root = Join-Path $out $name

cargo build --release -p luma-studio -p luma-cli
if ($LASTEXITCODE -ne 0) { throw 'cargo build' }
cargo build -q --release -p luma-studio --example icon
if ($LASTEXITCODE -ne 0) { throw 'icône' }

if (Test-Path $root) { Remove-Item -Recurse -Force $root }
New-Item -ItemType Directory -Force $root | Out-Null
Copy-Item target\release\luma-studio.exe, target\release\luma.exe $root
& target\release\examples\icon.exe (Join-Path $root 'luma.ico') windows
if ($LASTEXITCODE -ne 0) { throw 'icône' }

@"
LÜMA $version pour Windows (x64)

Lancer : luma-studio.exe (double-clic), ou glissez une photo ou un dossier dessus.
La ligne de commande : luma.exe --help

Windows 10 ou 11, 64 bits, avec un pilote graphique DirectX 12 ou Vulkan (ceux
d'Intel, d'AMD et de NVIDIA, à jour, conviennent).

Vos réglages, le catalogue et les préréglages : %APPDATA%\LÜMA
Les caches (doublons de travail, tuiles de la carte) : %LOCALAPPDATA%\LÜMA

14 jours d'essai, complet, puis LÜMA Premium : https://maxlestage.github.io/LUMA/
"@ | Set-Content -Encoding utf8 (Join-Path $root 'LISEZMOI.txt')

$zip = Join-Path $out "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $root -DestinationPath $zip
Write-Output $zip

# L'installateur, si Inno Setup est là (il l'est sur les machines Windows de GitHub).
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
