#!/usr/bin/env bash
# Uploads the PENDING crates, in dependency order, from the release checkout
# that tools/release/publish.py verified. `--no-verify` builds nothing, so no
# dependency code runs while the token is held; Cargo packages the same bytes
# the verification compared with the attested assets. Used by publish-crate.yml.
set -euo pipefail
if [[ -z ${CARGO_REGISTRY_TOKEN:-} ]]; then
  echo "::error::no crates.io token: for credential first-publication-token, the environment secret CRATES_IO_FIRST_PUBLICATION_TOKEN must be set (docs/CRATE-FIRST-PUBLICATION.md). Nothing was uploaded."
  exit 1
fi
read -r -a pending <<<"${PENDING}"
order=" tilewright-ui tilewright-app tilewright-ui-render "
for crate in "${pending[@]}"; do
  if [[ ${order} != *" ${crate} "* ]]; then
    echo "::error::refusing to publish '${crate}'"
    exit 1
  fi
done
cd "${RUNNER_TEMP}/release"
for crate in "${pending[@]}"; do
  echo "::group::cargo publish -p ${crate}"
  CARGO_TARGET_DIR="${RUNNER_TEMP}/upload-target" cargo publish --locked --no-verify -p "${crate}"
  echo "::endgroup::"
done
