# Package and release plan

The current workspace is an unpublished prototype. Every package has
`publish = false`; declaring version `0.1.0` does not reserve a registry name or
authorize an upload. Renderer `0.6.0` and its immutable source release remain an
independent, unchanged delivery. This workspace targets the published `0.6.2` successor through a committed
registry-only `Cargo.lock`. This dependency qualification does not publish or
stabilize the UI packages.

## Renderer dependency provenance

Renderer `0.6.2` was published from immutable source release `v1.21.0`
(release ID `429939049`), revision
`55f13ac311a5574b242e2520942377e51fd6b7cb`. Its `crate-release.json` SHA-256 is
`a5f5157fed68245b7eb74766f82a66b85f58920e31382512c3462d61dc14d43f`.
All eight released crate archives were verified against the release manifest,
registry archive bytes and sparse-index checksums, with owner `zoeyrose`
(registry user ID `437663`). Every intra-family dependency requires `=0.6.2`.

The lock contains seven family packages, all from crates.io at `0.6.2` with the
verified archive checksums. The eighth, `tilewright-render-sdl3`, is pinned in
workspace dependencies but unused by current workspace packages; its compatible
release was verified separately. The native example uses `sdl3` directly.
This is the first committed lockfile, so there is no earlier committed
transitive-version baseline to compare. Future updates must preserve a verified,
compatible renderer cohort and review the full lockfile delta.

## Proposed first package family

| Package | Direct shared-library dependencies | Initial status |
| --- | --- | --- |
| `tilewright-ui` | Unicode segmentation | Unpublished headless core |
| `tilewright-app` | `tilewright-ui` | Unpublished host coordination |
| `tilewright-ui-render` | `tilewright-ui`, released renderer `0.6.2` APIs | Unpublished renderer adapter |
| `tilewright-ui-ffi` | `tilewright-ui`, `tilewright-app`; optional UI renderer | Unpublished C facade |

The native demo and browser host packages remain unpublished. Avoid creating
separate public chat, focus, theme or platform crates until independent consumers
or release needs justify them. Domain-specific client executables and adapters
stay in their consuming projects.

The initial UI family can use a coherent `0.1.0` release after qualification,
published in dependency order: UI first, app/render next, facade last. This is a
proposal to prepare and review, not publication approval. Names must be checked
again at that point; a previous registry 404 is not a reservation.

## Required evidence before publication

1. Resolve and commit the real registry dependency graph after renderer `0.6.2`
   is published. A task-local renderer patch or generated candidate lock is not
   registry-consumer evidence.
2. Reconcile implemented APIs with the documented contracts; retain independent
   review and passing required source, C, native and browser checks at the exact
   release revision. Report platform and accessibility limits explicitly.
3. Verify standalone crate contents, licenses, provenance, dependency order,
   checksums and SBOMs. Font bytes remain separate licensed data with their
   original notices; no application/game content enters the packages.
4. Prepare a source release and, separately, C headers and native library assets.
   Rust package versions and the C ABI version have distinct compatibility
   contracts. The prototype currently negotiates ABI 2; no stable C release has
   been made.
5. Review the exact new release policy, repository/producer identity, immutable
   source assets and first-publication ownership flow. Existing renderer
   publication tooling is bound to a different repository and must not be copied
   or invoked as though it already authorizes this one.
6. Obtain approval for the concrete publication batch. First publication of new
   names and later trusted-publisher setup remain owner operations under the
   accepted project workflow; credentials never belong in source or chat.

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
