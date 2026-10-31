#!/usr/bin/env bash
# Ubuntu 24.04 hosted-runner prerequisites. Do not run on development hosts.
# Scoped from tilewright-renderer 4488ffb/tools/install-ci-dependencies.sh (MIT).
set -euo pipefail
test "${GITHUB_ACTIONS:-}" = true
test -n "${RUNNER_TEMP:-}"
test -n "${GITHUB_PATH:-}"

sudo apt-get update
sudo apt-get install --no-install-recommends --yes \
  build-essential cmake pkg-config python3 \
  libasound2-dev libdbus-1-dev libdecor-0-dev \
  libdrm-dev libegl1-mesa-dev libgbm-dev libgl1-mesa-dev libgles2-mesa-dev \
  libibus-1.0-dev libpipewire-0.3-dev libpulse-dev libsndio-dev libudev-dev \
  liburing-dev libvulkan-dev libwayland-dev libx11-dev libxcursor-dev \
  libxext-dev libxfixes-dev libxi-dev libxkbcommon-dev libxrandr-dev \
  libxss-dev libxtst-dev mesa-vulkan-drivers

rustup toolchain install 1.97.1 --profile minimal --component clippy,rustfmt \
  --target wasm32-unknown-unknown
printf '%s\n' "${HOME}/.cargo/bin" >> "${GITHUB_PATH}"
# Record the hosted distribution's dependency versions alongside the build log.
dpkg-query -W -f='${Package}=${Version}\n' > "${RUNNER_TEMP}/tilewright-ui-ci-packages.txt"
rustup run 1.97.1 rustc --version
cmake --version
