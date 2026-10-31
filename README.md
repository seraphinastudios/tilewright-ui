# Tilewright UI

Shared widget behavior and application coordination for native tools, browser
clients and C/SDL hosts. This is an experimental, unpublished prototype; its Rust
API and C ABI are not stable releases.

The initial slice is bounded chat history, a Unicode composer, focus, submission,
scrolling and a modal over a world viewport. Applications retain their event
loop, renderer presentation, networking, storage and game/editor authority.

| Package | Responsibility |
| --- | --- |
| `tilewright-ui` | Headless widget state, exact text/composition, input ownership and acknowledged actions |
| `tilewright-app` | Lifecycle, cancellation and prepared/displayed frame coordination |
| `tilewright-ui-render` | Existing Tilewright renderer integration, typography and immutable resources |
| `tilewright-ui-ffi` | Bounded, versioned C facade with explicit ownership |

Native and web host adapters are unpublished workspace examples. Existing
`tilewright-render-ui` remains the lower-level drawing and typography library;
this repository does not replace it or its independent releases.

Read the [architecture](docs/ARCHITECTURE.md), [interaction contract](docs/CONTRACT.md)
and [adoption roadmap](docs/ROADMAP.md). The [native Rust example](examples/native/README.md)
and [C/SDL example](examples/c-sdl/README.md) use the same state and renderer
through different interfaces. The [browser example](examples/web/README.md) uses
native HTML controls over the rendered world and preserves the same semantic
model. Build and browser tests do not establish OS IME, screen-reader or Classic
integration support; those capabilities require their own actual host evidence.

Rust 1.97.1 is pinned. The renderer dependency is the independently released
`0.6.0` family. Before checks, prepare the separately licensed font data outside
the checkout and resolve the real registry lock:

```sh
python3 tools/prepare-fonts.py /absolute/private/font-pack --acquire
export TILEWRIGHT_UI_FONT_ROOT=/absolute/private/font-pack
cargo generate-lockfile
bash tools/check.sh
bash tools/check-c.sh cpu-prototype
```

Preparation requires an absent output directory and verifies exact upstream
bytes and OFL notices. Runtime does not discover or download fonts. Packages
remain `publish = false`; preparing code or a lockfile does not publish crates.

New implementation is MIT. See [provenance](PROVENANCE.md) for permitted source
extractions and separately licensed font metadata. Game-specific code and data
remain in their consuming applications.
