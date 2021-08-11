#!/bin/bash
set -eu

TARGET_NAME="learn_words.wasm"
TARGET_NAME2="learn_words_bg.wasm"
DOCS=../learn_words_wasm2

# export RUSTFLAGS=--cfg=web_sys_unstable_apis

rm -f ${DOCS}/${TARGET_NAME}

echo "Building rust…"
cargo build --release --target wasm32-unknown-unknown

echo "Generating JS bindings for wasm…"
wasm-bindgen "target/wasm32-unknown-unknown/release/${TARGET_NAME}" \
  --out-dir ${DOCS} --no-modules --no-typescript

wasm-strip ${DOCS}/${TARGET_NAME2}

echo "Finished"
