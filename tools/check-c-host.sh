#!/usr/bin/env bash
# Build the real C/SDL sample against this invocation's bundled SDL, then test
# adapter behavior with SDL's dummy driver. This does not qualify desktop IME.
set -euo pipefail
cd "$(dirname "$0")/.."
output="$(mktemp -d)"
trap 'rm -rf "$output"' EXIT
if ! cargo build --locked --package tilewright-ui-native-demo --message-format=json > "$output/native-build.json"; then
  cat "$output/native-build.json"
  exit 1
fi
target_root="$(python3 -c 'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve())' "${CARGO_TARGET_DIR:-target}")"
sdl_config="$(python3 tools/find_sdl_config.py "$output/native-build.json" "$target_root")"
cargo build --locked --package tilewright-ui-ffi --features cpu-prototype
cmake -S examples/c-sdl -B "$target_root/c-host" \
  -DSDL3_DIR="$sdl_config" \
  -DCMAKE_FIND_USE_PACKAGE_REGISTRY=OFF -DCMAKE_FIND_USE_SYSTEM_PACKAGE_REGISTRY=OFF \
  -DTILEWRIGHT_UI_LIBRARY="$target_root/debug/libtilewright_ui_ffi.so"
cmake --build "$target_root/c-host" --parallel 2
SDL_VIDEODRIVER=dummy ctest --test-dir "$target_root/c-host" --output-on-failure --no-tests=error
SDL_IME_IMPLEMENTED_UI=candidates "$target_root/c-host/tilewright-ui-c-sdl" --check-ime
