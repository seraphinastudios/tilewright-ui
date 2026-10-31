# Shared renderer adapter

`UiRenderer::compile(&prepared.snapshot, &prepared.layout, physical_size)`
translates the exact neutral `tilewright-ui::Layout` retained by the app.
It targets `tilewright-render-ui =0.6.1` for Unicode shaping, selection geometry,
glyph rasterization and immutable resources. It does not create widget bounds.

Construct `UiRenderer` with a verified `FontPack`, text/paint budgets and a
caller-reserved `Namespace`. Fonts are supplied as explicit bytes and SHA-256
digests. Reserve the two resource IDs for the engine's entire lifetime; retain
each `CompiledFrame` while a frame is prepared, in flight or displayed.
Namespaces are checked again when composing an existing world scene.

`CompiledFrame::compose` returns an ordinary scene and resource provider for the
existing renderer. `standalone` creates an overlay. The optional `prototype`
feature enables `Presentation::render_rgba`, using the existing CPU reference
renderer; it is not a production GPU path or performance qualification.
A transparent reference target returns premultiplied RGB; use premultiplied
composition (or composite onto an opaque background) when uploading it to SDL.

Logical viewport dimensions must be integral. Supply physical dimensions
explicitly; all paint clipping and DPI transformation happen in the existing
renderer. `TextGeometry` remains in logical coordinates and uses the exact
immutable layout that produced the glyphs. Hosts pair it with the acknowledged
application frame stamp before pointer selection, visual movement or IME use.
Composer text stays complete even when horizontally clipped/scrolled. Current
renderer primitive text is unwrapped; history lines clip within their bounds.

Unit tests require no ambient fonts. The explicit external-font integration
test requires feature `prototype`, `TILEWRIGHT_UI_FONT_ROOT` pointing at the
verified renderer Unicode pack. Missing explicit fonts fail that feature's tests.
It checks Unicode,
clipping, DPI geometry, immutable atlas retention, stale snapshots, namespaces
and actual CPU reference pixels. No OS IME or screen-reader acceptance is implied.
