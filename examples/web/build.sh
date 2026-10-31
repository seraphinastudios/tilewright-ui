#!/usr/bin/env bash
# Build the local unpublished browser example with existing pinned tooling.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
actual=$(wasm-bindgen --version)
if [[ "$actual" != 'wasm-bindgen 0.2.127' ]]; then
  echo 'Expected wasm-bindgen 0.2.127; no tooling is installed automatically.' >&2
  exit 1
fi
cargo build --locked --release --target wasm32-unknown-unknown --package tilewright-ui-web
wasm-bindgen --target web --out-dir examples/web/pkg "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/tilewright_ui_web.wasm"

python3 examples/web/stage-fonts.py --output examples/web/assets/font-pack
