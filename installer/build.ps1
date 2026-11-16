# Construit l'installateur Windows sous Windows.
#
# Prérequis : Rust (MSVC), Git, NSIS 3 (https://nsis.sourceforge.io).
# Utilisation, depuis le dossier du projet :  powershell -ExecutionPolicy Bypass -File installer\build.ps1
# Résultat : target\installer\Dawsend-Setup-<version>.exe
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$Staging = Join-Path $Root "installer\staging"

Write-Host "==> Compilation du plugin"
cargo xtask bundle dawsend-plugin --release
if ($LASTEXITCODE -ne 0) { throw "échec de la compilation du plugin" }

Write-Host "==> Compilation de l'appli"
cargo build --release -p dawsend-app --bin dawsend-console
if ($LASTEXITCODE -ne 0) { throw "échec de la compilation de l'appli" }

Write-Host "==> Préparation des fichiers"
if (Test-Path $Staging) { Remove-Item -Recurse -Force $Staging }
New-Item -ItemType Directory -Force "$Staging\Dawsend.vst3\Contents\x86_64-win", "target\installer" | Out-Null
Copy-Item target\bundled\Dawsend.vst3\Contents\x86_64-win\Dawsend.vst3 "$Staging\Dawsend.vst3\Contents\x86_64-win\"
Copy-Item target\bundled\Dawsend.clap $Staging
Copy-Item target\release\dawsend-console.exe $Staging
Copy-Item LICENSE "$Staging\LICENSE.txt"
Copy-Item LICENSES\GPL-3.0.txt "$Staging\GPL-3.0.txt"
Copy-Item LICENSES\README.md "$Staging\LICENCES.txt"
Copy-Item installer\LISEZMOI.txt "$Staging\LISEZMOI.txt"

Write-Host "==> Installateur NSIS"
$Makensis = "${env:ProgramFiles(x86)}\NSIS\makensis.exe"
if (-not (Test-Path $Makensis)) { $Makensis = "makensis" }
& $Makensis -V2 "-DVERSION=$Version" installer\dawsend.nsi
if ($LASTEXITCODE -ne 0) { throw "échec de makensis" }
Get-Item "target\installer\Dawsend-Setup-$Version.exe"
