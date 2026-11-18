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

The single-line rules in `src/text.rs` (refusal of control characters, U+2028
and U+2029 in new input; lossless opening of stored values; the stored-break
markers and zero-width guards; `mark_stored_breaks`, `DisplayMarker`, the
marker constants, and the display/committed position mapping in `snapshot` and
`committed_cursor`) are adapted from the same MIT-licensed file at exact
commit `fb5ac4159faae0c4a222e0d2d3a1e1b0670c7af8` of
`seraphinastudios/tilewright-editor` (Copyright 2026 Seraphina Studios,
SPDX-License-Identifier: MIT). Adaptations: `ToolError` becomes
`UiError::LimitExceeded` and `UiError::InvalidInput`, the predicate is public
as `refuses_single_line`, and the editor's English `stored_break_hint` is left
to hosts. `tests/stored_breaks.rs` is written for this crate's API; its cases
follow the intent of the editor's `tests/stored_breaks.rs` and
`tests/text_input.rs` at that commit.
