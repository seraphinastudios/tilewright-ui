# Tilewright UI

Shared widget behavior and application coordination for native tools, browser
clients and C/SDL hosts. This is an experimental 0.x prototype; its Rust API
and C ABI are not stable. Three of its crates are on crates.io.

The initial slice is bounded chat history, a Unicode composer, focus, submission,
scrolling and a modal over a world viewport. Applications retain their event
loop, renderer presentation, networking, storage and game/editor authority.

| Package | Responsibility | crates.io |
| --- | --- | --- |
| `tilewright-ui` | Headless widget state, exact text/composition, input ownership and acknowledged actions | Published |
| `tilewright-app` | Lifecycle, cancellation and prepared/displayed frame coordination | Published |
| `tilewright-ui-render` | Existing Tilewright renderer integration, typography and immutable resources | Published |
| `tilewright-ui-ffi` | Bounded, versioned C facade with explicit ownership | Not published |

The three published crates were first published at `0.1.0` and share one
version; [the release plan](docs/RELEASES.md#crate-versions) lists what each
version changes. While they are at 0.x, a breaking change to a public API, or a move to
the renderer's next minor version, takes the next minor version; anything else
takes the next patch version. The C facade stays unpublished; its library and
header are a separate track. Native and web host adapters are unpublished
workspace examples. Existing `tilewright-render-ui` remains the lower-level
drawing and typography library; this repository does not replace it or its
independent releases.

Read the [architecture](docs/ARCHITECTURE.md), [interaction contract](docs/CONTRACT.md)
and [adoption roadmap](docs/ROADMAP.md), plus the separate
[package and release plan](docs/RELEASES.md). The [native Rust example](examples/native/README.md)
and [C/SDL example](examples/c-sdl/README.md) use the same state and renderer
through different interfaces. The [browser example](examples/web/README.md) uses
native HTML controls over the rendered world and preserves the same semantic
model. Build and browser tests do not establish OS IME, screen-reader or Classic
integration support; those capabilities require their own actual host evidence.

Rust 1.97.1 is pinned. The renderer crates are pinned at exactly `0.7.2`, the
family published from renderer source release `v1.29.0`. The committed
`Cargo.lock` resolves registry packages only, and Dependabot proposes
dependency and Actions updates weekly. CI runs on GitHub-hosted runners: Linux
runs every check below, and Windows tests the three published crates. The
required status `Source checks` passes only when both pass. Before checks,
prepare the separately licensed font data outside the checkout:

```sh
python3 tools/prepare-fonts.py /absolute/private/font-pack --acquire
export TILEWRIGHT_UI_FONT_ROOT=/absolute/private/font-pack
bash tools/check.sh
bash tools/check-c.sh cpu-prototype
```

Preparation requires an absent output directory and verifies exact upstream
bytes and OFL notices. Runtime does not discover or download fonts. Only the
three published crates go to crates.io; every other package sets
`publish = false`. Each merge to `main` cuts a release, and the release that
first carries a crate version publishes it; see [releasing](docs/RELEASING.md)
and the owner's [first-publication checklist](docs/CRATE-FIRST-PUBLICATION.md).

New implementation is MIT. See [provenance](PROVENANCE.md) for permitted source
extractions and separately licensed font metadata. Game-specific code and data
remain in their consuming applications.
