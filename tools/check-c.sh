#!/usr/bin/env bash
# Link the real Rust static library from a strict C17 caller and execute it.
set -euo pipefail
cd "$(dirname "$0")/.."
features=()
cflags=()
case "${1:-}" in
  '') ;;
  cpu-prototype) features=(--features cpu-prototype); cflags=(-DTWUI_CPU_PROTOTYPE) ;;
  *) echo 'usage: bash tools/check-c.sh [cpu-prototype]' >&2; exit 2 ;;
esac
cargo build --locked -p tilewright-ui-ffi "${features[@]}"
output="$(mktemp -d)"
trap 'rm -rf "$output"' EXIT
"${CC:-cc}" -std=c17 -Wall -Wextra -Werror -pedantic "${cflags[@]}" -Iinclude \
  tests/c/abi.c "${CARGO_TARGET_DIR:-target}/debug/libtilewright_ui_ffi.a" \
  -o "$output/c-abi" -ldl -lpthread -lm
"$output/c-abi"

"${CC:-cc}" -std=c17 -Wall -Wextra -Werror -pedantic "${cflags[@]}" -Iinclude \
  crates/tilewright-ui-ffi/tests/embedded.c "${CARGO_TARGET_DIR:-target}/debug/libtilewright_ui_ffi.a" \
  -o "$output/c-embedded" -ldl -lpthread -lm
"$output/c-embedded"
