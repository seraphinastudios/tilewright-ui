# Prototype C boundary

`include/tilewright_ui.h` defines ABI version 1. This is an unpublished prototype,
not a stable ABI release. The host owns its event loop, SDL, windows, clipboard,
world controls and presentation. No host callback, path loading, network access
or native GPU handle crosses this facade.

All contexts belong to their creating thread. Calls are nonreentrant. Rust
unwinding is caught and poisons a context; destruction remains available. Abort,
allocation failure and undefined behavior in a caller are not recovered. Every
pointer must reference valid, aligned storage of the advertised extent, and
pointer arguments must not overlap. Null and size checks cannot prove arbitrary
C addresses valid. Destroy exactly once, then never use the handle again.

Inputs are copied within calls. Outputs copy into host buffers; no borrowed
library allocation escapes. A query with zero capacity reports required bytes,
and a short buffer never changes state or consumes an action. UTF-8 output has
no trailing NUL. The host accepts and deduplicates actions by context identity
and batch ID before acknowledging. Sampling a changed control epoch must cancel
held world input even if the final `world_input_allowed` value is true.

Text commands carry generation, control epoch and text revision. Obtain these
from `twui_get_state` at event capture; do not replace an old queued event's
identity with current values. Key input also carries a stable physical identity
independent of its semantic meaning, so Return and Numpad Enter remain distinct.
Apply a key successfully and query `twui_key_effect_allowed` before clipboard or
other host editing effects. Consumed events may be quarantined and do not alone
authorize those effects. `twui_move_visual` also checks physical ownership. A
valid world click clears composer focus and preedit before handoff; re-sample
the control epoch before applying world input.

Prepare does not display: acknowledge only after
a successful serialized host presentation. Discard only frames guaranteed never
to present. Invalidate the surface before resize, transform change or loss.

The optional `cpu-prototype` feature adds a shared-renderer reference overlay.
Define `TWUI_CPU_PROTOTYPE` in C when linking that build. Fonts are explicit
family/byte/digest records; the shared renderer checks digests before parsing.
The host supplies and verifies its licensed manifest. Rendered pixels are
premultiplied RGBA8, retain exact shaped geometry, and use caller-copy buffers.
IME rectangles, pointer caret hits and visual navigation require the exact
acknowledged, live revision. A first composer click focuses it; a subsequent
click after presentation can select a caret. Pixel output is limited to 4096 per
dimension and four million pixels per retained frame. Prepared-frame limits
bound retention, with one additional displayed frame. CPU reference rendering
does not establish production performance, accessibility or physical IME support.

`tests/c/abi.c` exercises strict C17 linkage, malformed text, stale targets,
action backpressure, copy sizing, modal cancellation epochs, presentation
ordering, wrong-thread rejection and repeated create/use/destroy. The Rust unit
test injects a panic and reentry guard state. C sanitizers can check the C harness
and its buffers; an ordinary Rust static library is not sanitizer-instrumented.
