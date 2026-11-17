#!/usr/bin/env bash
# Construit l'installateur Windows depuis Linux (compilation croisée mingw-w64).
#
# Prérequis (Debian/Ubuntu) :
#   sudo apt install mingw-w64 nsis
#   rustup target add x86_64-pc-windows-gnu
#   Node.js et pnpm (interface de l'appli)
#
# Résultat : target/installer/Dawsend-Setup-<version>.exe
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TARGET=x86_64-pc-windows-gnu
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
STAGING="$ROOT/installer/staging"

echo "==> Compilation du plugin ($TARGET)"
rm -rf target/bundled
cargo xtask bundle dawsend-plugin --release --target "$TARGET"

echo "==> Interface de l'appli"
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build

echo "==> Compilation de l'appli ($TARGET)"
cargo build --release -p dawsend-gui -p dawsend-app --bin dawsend --bin dawsend-console --target "$TARGET"

echo "==> Préparation des fichiers"
rm -rf "$STAGING"
mkdir -p "$STAGING/Dawsend.vst3/Contents/x86_64-win" target/installer
cp target/bundled/Dawsend.vst3/Contents/x86_64-win/Dawsend.vst3 "$STAGING/Dawsend.vst3/Contents/x86_64-win/"
cp target/bundled/Dawsend.clap "$STAGING/"
cp "target/$TARGET/release/dawsend.exe" "target/$TARGET/release/dawsend-console.exe" "$STAGING/"
cp crates/dawsend-gui/icons/icon.ico "$STAGING/dawsend.ico"
# Fichiers texte au format Windows (CRLF).
crlf() { sed 's/\r$//; s/$/\r/' "$1" > "$2"; }
crlf LICENSE "$STAGING/LICENSE.txt"
crlf LICENSES/GPL-3.0.txt "$STAGING/GPL-3.0.txt"
crlf LICENSES/OFL-1.1.txt "$STAGING/OFL-1.1.txt"
crlf LICENSES/README.md "$STAGING/LICENCES.txt"
crlf installer/LISEZMOI.txt "$STAGING/LISEZMOI.txt"

echo "==> Installateur NSIS"
LC_ALL=C.UTF-8 makensis -V2 -DVERSION="$VERSION" installer/dawsend.nsi
ls -l "target/installer/Dawsend-Setup-$VERSION.exe"
