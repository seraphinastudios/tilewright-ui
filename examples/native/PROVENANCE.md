# Prototype host provenance

The window/event/clipboard adapters and procedural map are independently authored
MIT code for this repository. No Classic code or consumer assets were copied.

The checked SDL composition capability setup and isolated conflicting-environment
test pattern, and conservative whole-drain Enter fence are adapted from MIT `tilewright-editor-app/src/native_input.rs` at
editor commit `82dbeb514527f2cf078da62dfd05144819edc286`. They advertise composition
rendering before SDL initialization and leave candidate UI to the OS. The C host
implements the corresponding documented SDL calls independently.

Text shaping, glyph pixels, selection geometry and IME rectangles come from the
existing Tilewright renderer through `tilewright-ui-render`; the hosts introduce
no font parser, glyph rasterizer or widget state model.
