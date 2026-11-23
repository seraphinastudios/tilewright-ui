# Package and release plan

Three crates are on crates.io: `tilewright-ui`, `tilewright-app` and
`tilewright-ui-render`, first published at `0.1.0` on 2026-11-23 (UTC) under the
owner's approval of 2026-11-17. They set `publish = ["crates-io"]`, and every
other package `publish = false`. Releases and publication run as
[RELEASING.md](RELEASING.md) describes; the owner's one-time first-publication
steps are in [CRATE-FIRST-PUBLICATION.md](CRATE-FIRST-PUBLICATION.md). From
0.1.1 on, a new crate version reaches crates.io through trusted publishing
alone, with no stored token. The renderer crates are pinned at exactly `0.7.2`
through a committed registry-only `Cargo.lock`.

## What is published

| Package | Direct shared-library dependencies | crates.io |
| --- | --- | --- |
| `tilewright-ui` | Unicode segmentation | Published, `0.1.0`; `0.1.1` with the release that owns it |
| `tilewright-app` | `tilewright-ui` | Published, `0.1.0`; `0.1.1` with the release that owns it |
| `tilewright-ui-render` | `tilewright-ui`, released renderer `0.7.2` APIs | Published, `0.1.0`; `0.1.1` with the release that owns it |
| `tilewright-ui-ffi` | `tilewright-ui`, `tilewright-app`; optional UI renderer | Not published; the C library and header are a separate track |

The native demo (`tilewright-ui-native-demo`) and the browser adapter
(`tilewright-ui-web`) are workspace examples and are not published either.

The three published crates share one version, the workspace version. While
they are at 0.x, a breaking change to a public API, or a move to the renderer's
next minor version, takes the next minor version; anything else takes the next
patch version. They are published in dependency order: `tilewright-ui`, then
`tilewright-app` and `tilewright-ui-render`.

Publishing follows the model of `tilewright-world`. The owner published the
first version of each new crate name herself, from a browser, with a
short-lived token used once by the publish workflow
([CRATE-FIRST-PUBLICATION.md](CRATE-FIRST-PUBLICATION.md)), then added a
trusted publisher for each crate; after that, releases publish through trusted
publishing only, with no stored token. CI packages the three crates with
Cargo's verification build on every pull request and tests them on Linux and
Windows.

## Crate versions

What each crate version changes for a host, newest first. Every entry keeps
the renderer at `0.7.2` and the C ABI and header as they were. Source release
tags (`vX.Y.Z`) and crate versions are numbered separately; each release's
notes say which crate version it owns (crate 0.1.1 is owned by the first
release cut after it merges).

### 0.1.1

A patch release: nothing is removed or changed in meaning, so a host on 0.1.0
can move to `=0.1.1` (all three crates together) without code changes.

- **Caret steps no longer stall at rounding-equal stops**
  (`tilewright-ui-render`). The renderer's layout can give one position two
  caret stops whose x differ only by floating-point rounding (97.700005 and
  97.7). `TextGeometry::step_caret` took the step between them, a change of
  affinity alone, as a direction-boundary stop, so an arrow press did nothing:
  next to a stored-break marker at 20 px (`Trail⏎mar⏎ker¶`, Left from End
  visited 9 twice) and in plain right-to-left text (`אבג`, 2, 4, 4, 6). A step
  that changes only the affinity is now a stop only when the caret is drawn
  more than 1/64 logical pixel away on either axis; otherwise it steps on.
  Genuine direction-boundary stops, which lie a glyph advance apart, are kept.
  Hosts that call `step_caret` (and the C `twui_move_visual`) get this with
  no change.
- **Scene limits for hosts** (`tilewright-ui-render`).
  `CompiledFrame::quad_count` gives the number of quads a frame adds to a
  composed scene, and `CompiledFrame::compose_with_limits` composes under the
  host's `SceneLimits` instead of the defaults. `compose` is unchanged.
- **Reads without a snapshot** (`tilewright-ui`). `Ui::composer_text`
  (borrowed committed text), `Ui::composer_revision`, `Ui::composer_selection`
  and `Ui::is_composing` read the current composer state without cloning a
  `UiSnapshot`.
- **A single field without its Apply control** (`tilewright-ui`).
  `layout_in_with` takes `LayoutOptions`; with `submit_control = false` an
  embedded panel lays out the composer across the whole inner width with no
  submit paint or hit region, for a host that draws its own control and sends
  `Input::Activate(SUBMIT)`. `layout_in` is `layout_in_with` with the
  defaults and lays out the control as before.
- `tilewright-app` changes only its version.

### 0.1.0

The first publication.

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
   provenance, dependency order and checksums. Releases carry no SBOM yet; one would be
   separate future work. Font bytes remain separate licensed data with their
   original notices; no application/game content enters the packages.
4. Cut a source release (`release.yml`). C headers and native library assets are a separate,
   later track. Rust package versions and the C ABI version have distinct
   compatibility contracts. The prototype currently negotiates ABI 2; no stable
   C release has been made.
5. The release policy, producer identity, immutable source assets and
   first-publication flow are this repository's own (`tools/release/`),
   modelled on World's design. Renderer or World publication tooling is bound
   to its own repository and is never invoked for this one.
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
