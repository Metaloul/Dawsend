# Construit l'installateur Windows sous Windows.
#
# Prérequis : Rust (MSVC), Git, Node.js et pnpm, NSIS 3 (https://nsis.sourceforge.io).
# Utilisation, depuis le dossier du projet :  powershell -ExecutionPolicy Bypass -File installer\build.ps1
# Résultat : target\installer\Dawsend-Setup-<version>.exe et Dawsend-<version>-portable.zip
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$Staging = Join-Path $Root "installer\staging"

Write-Host "==> Compilation du plugin"
cargo xtask bundle dawsend-plugin --release
if ($LASTEXITCODE -ne 0) { throw "échec de la compilation du plugin" }

Write-Host "==> Interface de l'appli"
pnpm --dir ui install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { throw "échec de pnpm install" }
pnpm --dir ui build
if ($LASTEXITCODE -ne 0) { throw "échec de la compilation de l'interface" }

Write-Host "==> Compilation de l'appli"
cargo build --release -p dawsend-gui -p dawsend-app --bin dawsend --bin dawsend-console
if ($LASTEXITCODE -ne 0) { throw "échec de la compilation de l'appli" }

Write-Host "==> Préparation des fichiers"
if (Test-Path $Staging) { Remove-Item -Recurse -Force $Staging }
New-Item -ItemType Directory -Force "$Staging\Dawsend.vst3\Contents\x86_64-win", "target\installer" | Out-Null
Copy-Item target\bundled\Dawsend.vst3\Contents\x86_64-win\Dawsend.vst3 "$Staging\Dawsend.vst3\Contents\x86_64-win\"
Copy-Item target\bundled\Dawsend.clap $Staging
Copy-Item target\release\dawsend.exe $Staging
Copy-Item target\release\dawsend-console.exe $Staging
Copy-Item crates\dawsend-gui\icons\icon.ico "$Staging\dawsend.ico"
Copy-Item LICENSE "$Staging\LICENSE.txt"
Copy-Item LICENSES\GPL-3.0.txt "$Staging\GPL-3.0.txt"
Copy-Item LICENSES\OFL-1.1.txt "$Staging\OFL-1.1.txt"
Copy-Item LICENSES\README.md "$Staging\LICENCES.txt"
Copy-Item installer\LISEZMOI.txt "$Staging\LISEZMOI.txt"

Write-Host "==> Installateur NSIS"
$Makensis = "${env:ProgramFiles(x86)}\NSIS\makensis.exe"
if (-not (Test-Path $Makensis)) { $Makensis = "makensis" }
& $Makensis -V2 "-DVERSION=$Version" installer\dawsend.nsi
if ($LASTEXITCODE -ne 0) { throw "échec de makensis" }
Write-Host "==> Version portable (zip)"
# Mêmes fichiers que l'installateur, plus portable.txt qui active le mode portable
# (réglages et journaux dans data\ à côté de dawsend.exe).
$Portable = Join-Path $Root "installer\portable-staging\Dawsend"
$Zip = Join-Path $Root "target\installer\Dawsend-$Version-portable.zip"
if (Test-Path (Split-Path -Parent $Portable)) { Remove-Item -Recurse -Force (Split-Path -Parent $Portable) }
New-Item -ItemType Directory -Force $Portable | Out-Null
Get-ChildItem $Staging -Exclude dawsend.ico | Copy-Item -Destination $Portable -Recurse
Copy-Item installer\portable.txt "$Portable\portable.txt"
if (Test-Path $Zip) { Remove-Item -Force $Zip }
# tar de Windows (bsdtar) : chemins en « / » comme le veut le format zip, ce que
# Compress-Archive de Windows PowerShell 5 ne fait pas.
tar.exe -a -c -f $Zip -C (Split-Path -Parent $Portable) Dawsend
if ($LASTEXITCODE -ne 0) { throw "échec de la création du zip portable" }

Get-Item "target\installer\Dawsend-Setup-$Version.exe", $Zip
