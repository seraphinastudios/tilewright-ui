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
`UiError::RefusedText`, `UiError::LimitExceeded` and `UiError::InvalidInput`
(refused characters now have their own status, checked before the budget), the
predicate is public as `refuses_single_line`, and the editor's English
`stored_break_hint` is left to hosts. `TextInputSnapshot::step_visual` is new
here. `tests/stored_breaks.rs` is adapted from the editor's
`crates/tilewright-editor-ui/tests/stored_breaks.rs` and
`crates/tilewright-editor-ui/tests/text_input.rs` at that commit (same
organization, MIT): the class table, the round-trip and preedit-mapping
checks and their coordinate tables come from there, rewritten for this
crate's API, with new cases for this crate's `Ui`, statuses and budget.
