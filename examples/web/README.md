# Unpublished browser adapter

`tilewright-ui-web` wraps the same `tilewright-app` and `tilewright-ui` as the
native and C hosts. JavaScript receives immutable snapshots and tokens, never an
independent widget model. The browser presentation choice is still pending;
these modules supply the presentation-independent boundary.

`build.sh` requires the pinned Rust toolchain, wasm target, and wasm-bindgen
0.2.127 already installed. It uses the workspace lock and produces an ignored
`pkg/` ES module beside its wasm file. No CDN, runtime download, game assets or
private account is used. Serve the directory over localhost to import the module.

```js
import { createUi, createWorld } from './load.mjs';
const app = await createUi();
app.focus(true);
const state = JSON.parse(app.snapshot());
app.replace('Hello 👩🏽‍💻', 13, 13,
  state.generation, state.control.epoch, state.textRevision);
// Always re-read the snapshot after a successful mutation.
```

Use the `load.mjs` facade for browser calls: it applies `exactText` before
passing browser strings to wasm and rejects unmatched UTF-16 surrogates that wasm-bindgen would otherwise
silently replace. Selection offsets accepted by `replace` and `select` use DOM
UTF-16 units. Rust converts to exact UTF-8 offsets; the shared model validates
complete grapheme boundaries. IDs/revisions are decimal strings checked as u64,
not imprecise JavaScript numbers or wrapping BigInt coercions.

A host routes browser input only once: native `input` supplies a complete value
and selection through `replace`; it must not also implement Backspace/Delete
through keydown. During IME, send `preedit` separately and do not submit Enter.
At composition completion, `replace` atomically accepts the complete native
value and selection, including cancellation of transient preedit. If validation
fails, restore the authoritative snapshot. Only the current generation, control
epoch and text revision can edit. Browser composition events do not expose a
portable candidate selection; the preedit selection is explicitly unavailable.

After applying a snapshot, synchronously measure the actual presentation,
`prepare(width,height,scale)`, then `ack_presented` that exact returned stamp
only once the matching presentation commit succeeds. Before pointer-based
selection call `select` with that stamp. Resizing or changing transforms must
call `invalidate_surface` before further geometric input. Failed commits must
call `discard_prepared`, never acknowledge. A rendering callback alone does not
prove monitor scanout. Retained prepared frames are bounded by the shared app.

`peek_actions` is non-destructive. Accept each batch once under a host context
identity, then call `ack_actions` with its exact ID. Do not add a local retry
queue or clear the composer before submit succeeds. `PressOwnership` observes
every control epoch and quarantines held releases across cancellation, even if
world permission becomes true again. `ClipboardRead` permits one request at a
time and applies completion only against its originating widget/revision.
Cancellation, rejection, denied permission and malformed clipboard strings do
not edit the current draft.

`createWorld(canvas,width,height,'auto')` renders the existing Tilewright
renderer testkit's neutral procedural world through released WGPU APIs. Explicit
`webgpu` or `webgl2` choices never fall back. There is no Canvas 2D world fallback.
Call `render` to present and `free` only after no call can reuse the object.

Unit checks: `node --test examples/web/boundary.test.mjs examples/web/clipboard.test.mjs` (Node 24 or 26). Rust bridge
checks run with `cargo test -p tilewright-ui-web`. Browser, OS IME, clipboard
permissions and assistive technology acceptance require actual host evidence;
unit tests do not qualify them.

`browser.test.mjs` requires generated `pkg/`, Node 24/26, Chromium and
playwright-core already installed in the private pinned test image. Run it
separately from the fast boundary tests. It qualifies real WASM execution and
both explicit renderer backends using a neutral canvas, but does not qualify
browser-native composition events, accessibility, clipboard permission prompts,
or a complete DOM presenter.
