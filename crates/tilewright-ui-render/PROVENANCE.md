# Renderer adapter provenance

The adapter targets `tilewright-render-ui =0.7.2` directly for shaping, bidi,
grapheme geometry, rasterization and immutable ATX1 resources. It contains no
second text engine. Font bytes remain explicit caller-owned inputs; no font
assets, font discovery or downloads are included.

The composition underline expansion policy and resource identity validation
were adapted from MIT `tilewright-editor-app/src/workspace_render.rs`, commit
`79ed332723f87e6eefe728b87e9cd63f56d454d0`, copyright 2026 Seraphina Studios.
`TextGeometry::step_caret` and its tests are new here (they moved from
`tilewright-ui`, where they were also new). `CompiledFrame::raster_sample_count`
counts samples with the rule of the private `raster_samples` function in MIT
`tilewright-scene` 0.7.2 (`src/lib.rs`, published from renderer source release
`v1.29.0`, copyright 2026 Seraphina Studios), whose whole-pixel span
arithmetic it follows so that the count equals what the scene limit charges.
Remaining adapter code was
independently authored against public renderer and shared UI contracts. No
Classic GPL or private game source, assets or markup were used.

The optional `prototype` feature uses the existing renderer's CPU reference
implementation. It demonstrates bounded RGBA output and does not establish
production GPU, compositor, OS IME or accessibility performance/acceptance.
