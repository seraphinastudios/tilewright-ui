# Text editing provenance

`src/text.rs` adapts the MIT-licensed exact UTF-8 draft, selection, transient
composition, scalar-offset conversion and grapheme-boundary mechanisms from
`crates/tilewright-editor-ui/src/text_input.rs` in the Tilewright editor source
checkout, exact commit `79ed332723f87e6eefe728b87e9cd63f56d454d0`.
The source header is Copyright 2026 Seraphina Studios, SPDX-License-Identifier:
MIT. The original header is retained. Adaptations replace editor-specific
`ToolError`, `MAX_INPUT_BYTES`, and draft identity with public UI errors,
per-instance limits, and a checked text revision. The surrounding chat,
control, layout and presentation coordination is newly authored for this repo.
No Classic GPL source, proprietary game source, glyphs, or fonts are included.
