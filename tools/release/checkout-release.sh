#!/usr/bin/env bash
# Checks out RELEASE_TAG's commit, clean, at $RUNNER_TEMP/release, and installs
# the Rust toolchain that commit pins. Used by publish-crate.yml.
set -euo pipefail
if [[ ! ${RELEASE_TAG:-} =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "::error::release_tag must be vX.Y.Z, not '${RELEASE_TAG:-}'"
  exit 1
fi
git worktree add --detach "${RUNNER_TEMP}/release" "refs/tags/${RELEASE_TAG}^{commit}"
channel=$(python3 -c 'import sys, tomllib; print(tomllib.load(open(sys.argv[1], "rb"))["toolchain"]["channel"])' \
  "${RUNNER_TEMP}/release/rust-toolchain.toml")
if [[ ! ${channel} =~ ^1\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::the release pins an unexpected toolchain '${channel}'"
  exit 1
fi
rustup toolchain install "${channel}" --profile minimal
(cd "${RUNNER_TEMP}/release" && cargo --version)
