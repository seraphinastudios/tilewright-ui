# Package and release plan

Nothing from this workspace is published yet. Every package has
`publish = false`, and the repository has no release or publish workflow;
declaring version `0.1.0` does not reserve a registry name. The renderer crates
are pinned at exactly `0.7.2` through a committed registry-only `Cargo.lock`.
Renderer `0.6.0` and its immutable source release remain an independent,
unchanged delivery.

## What will be published

| Package | Direct shared-library dependencies | crates.io |
| --- | --- | --- |
| `tilewright-ui` | Unicode segmentation | Planned, first version `0.1.0` |
| `tilewright-app` | `tilewright-ui` | Planned, first version `0.1.0` |
| `tilewright-ui-render` | `tilewright-ui`, released renderer `0.7.2` APIs | Planned, first version `0.1.0` |
| `tilewright-ui-ffi` | `tilewright-ui`, `tilewright-app`; optional UI renderer | Not published; the C library and header are a separate track |

The native demo (`tilewright-ui-native-demo`) and the browser adapter
(`tilewright-ui-web`) are workspace examples and are not published either.

The three published crates share one version, the workspace version. While
they are at 0.x, a breaking change to a public API, or a move to the renderer's
next minor version, takes the next minor version; anything else takes the next
patch version. They are published in dependency order: `tilewright-ui`, then
`tilewright-app` and `tilewright-ui-render`.

Publishing follows the model of `tilewright-world`. The owner publishes the
first version of each new crate name herself, then adds a trusted publisher
for each crate; after that, releases publish through trusted publishing only,
with no stored token. The release and publish workflows, release notes and the
owner's first-publication checklist are still to be added. The crate metadata,
per-crate READMEs and the packaged licence file are in place, and CI tests the
three crates on Linux and Windows.

## Renderer dependency provenance

Renderer `0.7.2` was published from immutable source release `v1.29.0`
(release ID `449670579`), revision
`f1afc4d25f1c0b6203bba47135000c4f6c8d72ea`. Its `crate-release.json` SHA-256 is
`de8c7893e7d52deb52a35c6cdf4639ce13dcfe1eb2ac90554e82e099ba35d41c`.
All eight released crate archive digests match the release manifest, its
`SHA256SUMS` and the crates.io sparse-index checksums, with owner `zoeyrose`
(registry user ID `437663`). Every intra-family dependency requires `=0.7.2`.

The lock contains seven family packages, all from crates.io at `0.7.2` with the
verified archive checksums. The eighth, `tilewright-render-sdl3`, is pinned in
workspace dependencies but unused by current workspace packages; its index
entry requires `sdl3 ^0.22.1`, matching the native example's direct `=0.22.1`.

`0.7.2` changes no public API and no rendering. In `tilewright-render-ui`, a
text system now keeps an unchanged atlas's exact identity per atlas ID,
whatever other IDs were compiled in between, for a small fixed number of
recently used IDs; `0.7.1` kept it only when the previous compile used the same
ID. No other crate's source changes. Its external dependency requirements and
features equal `0.7.1`'s, so the lockfile delta is the seven family versions and
checksums alone.

`0.7.1` changed no public API and no rendering. It added tests inside the
crates, bounded the ground-grid tile side in `tilewright-render-testkit` (an
out-of-range side returns `LimitExceeded`) and added comments.

The earlier update from `0.6.2` to `0.7.0` skipped the intermediate `0.6.3`
cohort. `0.6.3` moved `tilewright-render-ui` text layout from cosmic-text to
Parley `0.12` (fontique without system-font discovery, swash rasterization),
which removed cosmic-text, fontdb and ttf-parser from the lock. `0.7.0` moved
`tilewright-render-sdl3` to `sdl3 0.22.1`. From `0.6.3` on, each host-supplied
font's declared family must equal the face's primary family name exactly (its
typographic family name, else its family name); `0.6.2` also accepted any other
family name the face carried. Fonts that loaded under a secondary family name,
or the same font bytes supplied twice under different names, are refused as
invalid fonts. SDL itself stays at `sdl3-sys 0.7.2+SDL-3.4.18`. Future updates
must preserve a verified, compatible renderer cohort and review the full
lockfile delta.

## Package boundaries

Avoid creating separate public chat, focus, theme or platform crates until
independent consumers or release needs justify them. Domain-specific client
executables and adapters stay in their consuming projects. Check the crate
names again just before the first publication; a previous registry 404 is not a
reservation.

## Required evidence before publication

1. The committed lock resolves the published renderer `0.7.2` crates from the
   registry. A task-local renderer patch or generated candidate lock is not
   registry-consumer evidence.
2. Reconcile implemented APIs with the documented contracts; retain independent
   review and the passing required `Source checks` status at the exact release
   revision. Report platform and accessibility limits explicitly.
3. Verify standalone crate contents (`cargo package --list` shows each crate's
   sources, README, `LICENSE` and, where present, `PROVENANCE.md`), licenses,
   provenance, dependency order, checksums and SBOMs. Font bytes remain separate licensed data with their
   original notices; no application/game content enters the packages.
4. Prepare a source release. C headers and native library assets are a separate,
   later track. Rust package versions and the C ABI version have distinct
   compatibility contracts. The prototype currently negotiates ABI 2; no stable
   C release has been made.
5. Review the exact new release policy, repository/producer identity, immutable
   source assets and first-publication ownership flow. Existing renderer
   publication tooling is bound to a different repository and must not be copied
   or invoked as though it already authorizes this one.
6. First publication of each new name and the trusted-publisher setup are owner
   operations, done from the first-publication checklist; credentials never
   belong in source or chat.

Future automatic reconciliation must be enabled only for a reviewed set of
releases. Do not import the renderer's historical unpublished cohorts or its
version policy into this repository.

## Consumer adoption

Released Rust consumers pin compatible versions and a verified lockfile. C
consumers consume a checked header/library pair and validate ABI negotiation.
Browser consumers use the maintained JavaScript facade around its matching WASM
artifact, preserving checked string and integer conversion. Local prototype
overrides never become production dependencies.

Archive decisions for superseded application repositories and full Classic
cutover remain separate work. A shared UI release does not imply either has
already happened.
