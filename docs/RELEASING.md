# Releasing tilewright-ui

Releases follow the design of `tilewright-world`: every merge to `main` is
released once the first release exists, releases are immutable GitHub
releases tagged `vX.Y.Z` whose assets carry build provenance, and the crates
reach crates.io through trusted publishing from the release that owns their
version. `.github/workflows/release.yml` and `publish-crate.yml` do the work;
`tools/release/` holds their code, standard-library Python with tests. The
owner's one-time steps are in [CRATE-FIRST-PUBLICATION.md](CRATE-FIRST-PUBLICATION.md).

## What a release contains

| Asset | Contents |
| --- | --- |
| `tilewright-ui-X.Y.Z-source.tar.gz` | `git archive` of the released commit |
| `tilewright-ui-C.crate`, `tilewright-app-C.crate`, `tilewright-ui-render-C.crate` | The three crates at the workspace crate version C, as `cargo package` made them, after Cargo's verification build |
| `SHA256SUMS` | The SHA-256 of every other asset |

Check an asset with `sha256sum --check --ignore-missing SHA256SUMS`, and its
provenance with the command each release's notes print:
`gh attestation verify FILE --repo seraphinastudios/tilewright-ui --cert-identity https://github.com/seraphinastudios/tilewright-ui/.github/workflows/release.yml@refs/heads/main --source-ref refs/heads/main --source-digest COMMIT`.
Pin the ref: any writer can run a modified `release.yml` on another branch.

## Versions

- **The source release** (`vX.Y.Z`). Pull requests are squash-merged with
  their Conventional Commits title (`pr-title.yml` checks it). Every release
  from `main` takes the next minor version, as in World, if any commit since
  the last release has a type, reverts a commit or carries a
  `BREAKING CHANGE:` note. The first release is `v0.1.0`, and only a manual
  run of the Release workflow on `main` cuts it.
- **The crate version** (`workspace.package.version`), shared by the three
  crates, which require each other exactly (`=C`). Only a pull request that
  changes it on purpose moves it. While at 0.x, a breaking change to a public
  API, or a move to the renderer's next minor version, takes the next minor
  version; anything else the next patch. CI's "Crate version stays or moves
  one step" check refuses any other move, and anything at 1.0.0 or above.
- **Ownership.** The first release at a crate version owns it, for good: its
  `.crate` assets are what crates.io receives. Later releases at the same
  crate version carry archives with other bytes (the revision is inside each)
  that are never uploaded. Content changed after a crate version was
  released reaches crates.io only with the next crate version.

To publish a new crate version, open a pull request that bumps
`workspace.package.version` and the three `=C` requirements in
`[workspace.dependencies]` together (the policy check fails otherwise), and
merge everything meant for that version before it or with it.

## What runs

| Event | What happens |
| --- | --- |
| Pull request touching the release files or a manifest | **Plan release** and **Package release assets** as a check, under version `0.0.0-check.N`. Nothing is tagged, attested or published |
| Every pull request and push (`ci.yml`) | `cargo package` with verification of the three crates, the packaged-content check, the crate version step check and the release tool tests |
| Merge to `main` before any release | **Plan release** only, which says the first release has to be started by hand |
| Merge to `main` | Plan, package, attest, tag `vX.Y.Z` at the merge commit, publish the immutable release, then **Start crate publication** |
| Run workflow on `main` | The same, and it can cut the first release |

**Start crate publication**, for a release that owns its crate version,
starts **Publish crates** with trusted publishing once every crate name
exists on crates.io. Before the owner's first publication it reports "First
publication pending" and starts nothing, successfully. For a release that does
not own its crate version it only says which release does.

**Publish crates** (dispatch on `main` only; inputs `operation`
`verify`/`publish`, `credential`, `release_tag`):

| Job | Permissions | Does |
| --- | --- | --- |
| Verify the release's crate archives | `contents: read`, `attestations: read` | Checks the tag is on `main` and owns its crate version; the release is published, stable and immutable; each `.crate` matches `SHA256SUMS`, GitHub's asset digest and build provenance from `release.yml` on `main` at the tagged commit on a GitHub-hosted runner; `cargo package --no-verify` of the tagged commit gives the same bytes; crates.io lacks or has exactly those bytes. Decides what to upload, and for `publish` checks the environment. With trusted publishing, a crate name crates.io lacks stops it: "first publication pending", nothing uploaded |
| Publish to crates.io | the above, `id-token: write`, environment `crates-io-release` | Verifies again on its own runner, exchanges the OIDC token for a 30-minute crates.io token (`rust-lang/crates-io-auth-action`, which revokes it at the end), runs `cargo publish --locked --no-verify` for each missing crate in order (`tilewright-ui`, `tilewright-app`, `tilewright-ui-render`), then waits until crates.io's index lists the attested checksums |

Only an upload step receives a crates.io token. `--no-verify` builds
nothing, so no dependency code runs in the job that can request an OIDC
token; Cargo packages the same bytes the job compared with the attested
assets, and the last step compares crates.io's checksums with them again.
Dispatching again is safe: present crates are skipped, and a run with
nothing to upload still exchanges the token, as a canary.

Every action is pinned to a full commit SHA on the organization's allowlist,
no job restores a cache or uses a stored secret, and runners are
GitHub-hosted. The first-publication token is the one exception, once,
described in the checklist.

## Repository settings it relies on

The workflows change no settings. They rely on squash merges with the pull
request's title and description (on); immutable releases (on); the
organization ruleset "Tilewright 04 - Immutable release tags", which covers
`refs/tags/v[0-9]*` in this repository (on); the Actions allowlist, which
already admits every action used; the environment `crates-io-release`,
admitting only `main` with no administrator bypass, no reviewers and no
secrets except during the first publication; and one crates.io trusted
publisher per crate naming this repository, `publish-crate.yml` and
`crates-io-release`. Renaming the workflow file, the environment or the
repository breaks publication until all three publishers are set up again.

## When something fails

- **Before "Publish the GitHub release"**: nothing was published. Fix the
  cause; the next merge releases everything since the last release, or
  re-run the failed jobs. A failed first release needs another manual run.
- **"Publish the GitHub release"**: re-run the failed jobs within 30 days.
  The re-run accepts a tag the failed attempt created at the same commit,
  deletes leftover drafts and publishes, or, if the release is out, only
  checks it. Until that release is published, every later run stops at
  "Stop at an unfinished release". A release that came out mutable cannot be
  fixed in place: turn immutable releases back on and ask the owner.
- **"Start crate publication" or "Publish crates"**: the release is out and
  stays. Dispatch **Publish crates** again for the owning release, with
  `operation=verify` first if unsure. A 403 from the token exchange or the
  upload means a missing or wrong trusted publisher (checklist step 8). If
  crates.io has a version with other bytes, or one is yanked, the run stops:
  publish the content as the next crate version.
