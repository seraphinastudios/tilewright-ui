# tilewright-ui-render

Draws `tilewright-ui` widgets through the Tilewright renderer crates, pinned
at exactly `0.7.2`. It reuses the renderer's text shaping, glyph resources and
drawing; it does not add a text engine of its own. See the
[repository](https://github.com/seraphinastudios/tilewright-ui) for the
architecture and the examples.

## Status

This is a 0.x prototype. Its API is not stable, and the crate is not yet
published to crates.io. It shares one version with `tilewright-ui` and
`tilewright-app`. While they are at 0.x, a breaking change to a public API, or
a move to the renderer's next minor version, takes the next minor version;
anything else takes the next patch version.

Licensed under the MIT license. Fonts are separate data under their own
licences; this crate contains none and never searches for system fonts.

## Usage

`UiRenderer::compile(&prepared.snapshot, &prepared.layout, physical_size)`
translates the exact neutral `tilewright-ui::Layout` retained by the app.
It targets `tilewright-render-ui =0.7.2` for Unicode shaping, selection geometry,
glyph rasterization and immutable resources. It does not create widget bounds.

Construct `UiRenderer` with a verified `FontPack`, text/paint budgets and a
caller-reserved `Namespace`; it owns its own `TextSystem`. A host that already
has a `TextSystem` for its own text constructs `UiCompiler` from the namespace
and budgets alone and lends that system to each call, so fonts load once and
the shared widgets shape exactly as the host's own text:

```rust,ignore
let mut compiler = UiCompiler::new(namespace, UiLimits::default())?;
let frame = compiler.compile(&mut host_text, &snapshot, &layout, physical_size)?;
```

Reserve its `atlas_resource` as an ID the host can never compute for its own
atlases, and lend the same `TextSystem` on every call. A revision lower than one
the compiler was issued before, or the same revision with different bytes, is
refused and changes nothing, so a compiler never returns two different atlases
under one (ID, revision). To switch systems, retire every frame carrying the
compiler's atlas ID and create a new `UiCompiler`. Host and shared compiles may
alternate on one system: an unchanged atlas keeps its exact identity per atlas
ID, so nothing is re-uploaded. The text system remembers only a small fixed
number of recently used atlas IDs; an ID compiled again after more other IDs
than that, or after `clear_cache`, loses reuse, never correctness. See
`docs/CONTRACT.md` in the repository.

Fonts are supplied as explicit bytes and SHA-256 digests. Reserve the two resource IDs for the engine's entire lifetime; retain
each `CompiledFrame` while a frame is prepared, in flight or displayed.
Namespaces are checked again when composing an existing world scene.

`CompiledFrame::compose` returns an ordinary scene and resource provider for the
existing renderer. `standalone` creates an overlay. The optional `prototype`
feature enables `Presentation::render_rgba`, using the existing CPU reference
renderer; it is not a production GPU path or performance qualification.
A transparent reference target returns premultiplied RGB; use premultiplied
composition (or composite onto an opaque background) when uploading it to SDL.

The `prototype` feature is outside the compatibility promise: it serves the
unpublished examples and C library, adds `tilewright-render-testkit` as a
public dependency, and `render_rgba` returns a `tilewright-render-api` type.
Any release, a patch release included, may change or remove it. Do not enable
it in a production consumer.

The crate root re-exports the renderer types needed to construct and drive
`UiRenderer`/`UiCompiler` and to read a layout: `TextSystem`, `TextLimits`,
`UiLimits`, `FontPack`, `FontBlob`, `UnicodeLayout`, `VisualMotion` and
`unicode_fonts`. A host that composes its own scene or reads frame internals
uses the renderer crates directly, at the same exact version: `SceneBundle`,
`ResourceProvider`, `Quad`, `UiViewport`, and the error payloads `TextError`
and `tilewright_scene::Error`. The renderer's own `TextCursor` and
`TextAffinity` are not re-exported: this crate's API takes and returns
`tilewright_ui::TextCursor`, while `UnicodeLayout`'s methods take the
renderer's, from `tilewright-render-ui` directly.

Logical viewport dimensions must be integral. Supply physical dimensions
explicitly; all paint clipping and DPI transformation happen in the existing
renderer. `TextGeometry` remains in logical coordinates and uses the exact
immutable layout that produced the glyphs. Hosts pair it with the acknowledged
application frame stamp before pointer selection, visual movement or IME use.
`TextGeometry::step_caret` moves the editable text's caret one visual step and
returns the new focus in committed text, stepping past the zero-width guard of
a stored-break marker (see `docs/CONTRACT.md`, Single-line text).
Composer text stays complete even when horizontally clipped/scrolled. Current
renderer primitive text is unwrapped; history lines clip within their bounds.

Unit tests require no ambient fonts. The explicit external-font integration
test requires feature `prototype`, `TILEWRIGHT_UI_FONT_ROOT` pointing at the
verified renderer Unicode pack. Missing explicit fonts fail that feature's tests.
It checks Unicode,
clipping, DPI geometry, immutable atlas retention, stale snapshots, namespaces
and actual CPU reference pixels. No OS IME or screen-reader acceptance is implied.
