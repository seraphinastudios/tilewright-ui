#!/usr/bin/env bash
# Deterministic source checks. Registry availability is required; no local patches.
set -euo pipefail
cd "$(dirname "$0")/.."
bash tools/check-architecture.sh
cargo fmt --all -- --check
: "${TILEWRIGHT_UI_FONT_ROOT:?Prepare the verified font pack with tools/prepare-fonts.py first}"
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --doc --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
