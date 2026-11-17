#!/bin/bash -eu
# Construit chaque cible de fuzz/fuzz_targets/ et la copie dans $OUT.

# Le nightly fourni par l'image d'OSS-Fuzz peut être plus ancien que le rust-version
# du workspace (1.93) : on prend un nightly daté, pour des builds reproductibles. À
# avancer quand Dependabot met à jour l'image.
NIGHTLY=nightly-2026-11-15
rustup toolchain install "$NIGHTLY" --profile minimal
export RUSTUP_TOOLCHAIN="$NIGHTLY"

cd "$SRC/dawsend/fuzz"
cargo fuzz build -O --debug-assertions
for f in fuzz_targets/*.rs; do
  cp "target/x86_64-unknown-linux-gnu/release/$(basename "${f%.*}")" "$OUT/"
done
