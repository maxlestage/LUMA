# Installe LÜMA pour vous seul, sans droits administrateur : les programmes dans
#   %LOCALAPPDATA%\Programs\LUMA, ajoutés à votre PATH, avec un raccourci au menu Démarrer.
# Pour installer :    powershell -ExecutionPolicy Bypass -File install.ps1
# Pour désinstaller : powershell -ExecutionPolicy Bypass -File install.ps1 -Remove
param([switch]$Remove)
$ErrorActionPreference = 'Stop'

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dest = Join-Path $env:LOCALAPPDATA 'Programs\LUMA'
$menu = [Environment]::GetFolderPath('Programs')
$shortcut = Join-Path $menu 'LÜMA.lnk'

function Set-UserPath([string]$dir, [bool]$add) {
    $path = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @($path -split ';' | Where-Object { $_ -and $_ -ne $dir })
    if ($add) { $parts += $dir }
    [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
}

if ($Remove) {
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    if (Test-Path $shortcut) { Remove-Item -Force $shortcut }
    Set-UserPath $dest $false
    Write-Host 'LÜMA est désinstallé (vos photos, réglages et films restent à leur place).'
    return
}

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Force (Join-Path $here 'bin\*') $dest -Recurse

# Le raccourci au menu Démarrer, avec l'icône embarquée dans l'exécutable.
$exe = Join-Path $dest 'luma-studio.exe'
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut($shortcut)
$lnk.TargetPath = $exe
$lnk.WorkingDirectory = $dest
$lnk.IconLocation = $exe
$lnk.Description = 'LÜMA, développement photo RAW'
$lnk.Save()

Set-UserPath $dest $true
Write-Host 'LÜMA est installé : cherchez « LÜMA » dans le menu Démarrer,'
Write-Host 'ou lancez « luma-studio » (et « luma » en ligne de commande, après réouverture du terminal).'
