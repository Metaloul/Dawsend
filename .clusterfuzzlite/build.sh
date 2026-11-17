#!/bin/bash -eu
# Construit chaque cible de fuzz/fuzz_targets/ et la copie dans $OUT.
cd "$SRC/dawsend/fuzz"
cargo fuzz build -O --debug-assertions
for f in fuzz_targets/*.rs; do
  cp "target/x86_64-unknown-linux-gnu/release/$(basename "${f%.*}")" "$OUT/"
done
