#!/usr/bin/env bash
# Link the real Rust static library from a strict C17 caller and execute it.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked -p tilewright-ui-ffi
output="$(mktemp -d)"
trap 'rm -rf "$output"' EXIT
"${CC:-cc}" -std=c17 -Wall -Wextra -Werror -pedantic -Iinclude \
  tests/c/abi.c "${CARGO_TARGET_DIR:-target}/debug/libtilewright_ui_ffi.a" \
  -o "$output/c-abi" -ldl -lpthread -lm
"$output/c-abi"
