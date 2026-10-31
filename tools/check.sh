#!/usr/bin/env bash
# Deterministic source checks. Registry availability is required; no local patches.
set -euo pipefail
cd "$(dirname "$0")/.."
bash tools/check-architecture.sh
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets -- -D warnings
