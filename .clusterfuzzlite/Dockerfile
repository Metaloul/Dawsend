# Image de construction des cibles de fuzzing (Rust nightly + cargo-fuzz), épinglée
# par empreinte ; Dependabot la met à jour.
FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:3428d0a90b1e7efbe2ba947bd1d0c9a70594986b4b6553804547f1fb24cfc92f
COPY . $SRC/dawsend
WORKDIR $SRC/dawsend
COPY .clusterfuzzlite/build.sh $SRC/
