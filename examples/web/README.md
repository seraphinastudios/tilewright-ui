# Browser chat and world prototype

This unpublished example runs the shared Rust application through native HTML
editing and semantic elements above a Tilewright WebGPU/WebGL2 world. DOM is the
reversible initial presenter; pixel-identical text with native hosts is not
promised. The JavaScript host owns event conversion, presentation commits and
the local echo effect. Widget text, selection, composition, history, focus,
modal state, scrolling, output batches and revisions remain in shared Rust.

## Run locally

Use the pinned Rust toolchain, wasm32 target, wasm-bindgen 0.2.127 and Python 3
already installed. Set `TILEWRIGHT_UI_FONT_ROOT` to the verified 17-font Noto
pack, then run `examples/web/build.sh` from this repository. Serve
`examples/web/` on localhost, for example with `python3 -m http.server --directory
examples/web 8080`, and open `/index.html`. Add `?backend=webgl2` or
`?backend=webgpu` for an explicit backend. Failure reports a visible error;
there is no Canvas 2D world fallback or runtime asset discovery. Font assets
are served locally with the page; no CDN or external font download is used.

The build stages fonts into ignored `assets/font-pack/` and WASM into ignored
`pkg/`. `stage-fonts.py` validates every font and OFL notice before copying and
retains the complete provenance manifest. The browser checks the exact manifest
and font digests before installing explicit `FontFace` resources. This is the
same `tilewright-unicode-noto-v1` pack as the native example, from Google Fonts
revision `6c1dbe27dcf7d817d4b62cec033ba77fa8e2a5ec`; its manifest SHA-256 is
`94b81a712fb41a66eb744c88ae2ed312cc5d0ec1a491e9821fcc278d99410f13`.
Coverage is the pack's declared repertoire, not every Unicode scalar or shaping
sequence. No font bytes enter the library crates.

## Interaction and integration

Type in Message, send with Enter or Send, and use the older/newer controls or
history wheel to scroll. Paste reads the browser clipboard only on activation.
Open modal traps keyboard focus and cancels held world controls. With focus
outside the panel, pressed physical keys appear in the world-control readout;
this illustrates cancellation and issues no gameplay commands.

`load.mjs` is the safe JavaScript entry point. It rejects unmatched UTF-16
surrogates before wasm-bindgen could replace them. Browser selection offsets
are checked as finite integers and translated to UTF-8; the shared model checks
grapheme boundaries. IDs and revisions cross as checked decimal strings.

Native `beforeinput`/`input` supplies one complete value and selection;
Backspace/Delete are never also applied by keydown. Composition updates remain
transient. Enter carries its physical identity and platform composition flag to
Rust, which fences submission until that key releases even when preedit is empty.
Completion commits the
whole native value atomically with exact owner/revision fencing. The browser
owns native candidate positioning; composition events do not expose a reliable
candidate selection, so that capability is explicitly unavailable to Rust.

DOM patches measure current bounds/scroll positions synchronously and acknowledge
the exact prepared revision. Stale pointer selection is refused. Resize, scroll,
DPI and host lifecycle changes invalidate or refresh presentation. Consumers
changing CSS transforms themselves must call `app.invalidate_surface()` before
`render()`. This commit boundary does not claim literal monitor scanout.

Action batches use non-destructive peek and explicit acknowledgment. The demo
records the most recent accepted batch ID before acknowledging, and keeps no
unbounded retry queue. `autoAccept:false` permits backpressure experiments.
Clipboard reads are limited to one outstanding request and carry the originating
widget, control epoch and text revision. Denial or stale completion cannot edit
a newer draft. Cancelled IME sessions retire their native input element and its
listeners; a fresh node carries the authoritative selection. A retained session
marker rejects late composition commits across intervening pointer/key activity
until a new composition lifecycle starts.

`browserKey` maps `event.code` to stable physical IDs, including distinct
Enter/NumpadEnter and left/right modifiers. Unknown codes do not acquire world
controls. Routing consumption does not blanket-cancel browser defaults or
reserved shortcuts. Custom key effects require a fresh `key_effects(physical)`
check; `ClipboardRead.pasteKey` performs that check before starting a keyboard
clipboard request. A consumed quarantined repeat is not eligible for effects.

## Validation limits

Run boundary checks with Node 24/26:
`node --test examples/web/boundary.test.mjs examples/web/clipboard.test.mjs`.
Run Rust checks with `cargo test -p tilewright-ui-web`.

The private pinned browser harness stages the verified fonts, builds WASM, and
runs `browser.test.mjs` with Chromium 156. It exercises native HTML typing,
selection, Backspace, Ctrl+A, safe text rendering, modal focus, history,
composition event conversion, stale clipboard permission stubs, physical keys,
presentation identities and both renderer backends. Synthetic composition
sequences qualify event conversion, not a real OS IME session. Physical assistive
technology, browser permission prompts, touch keyboards, browser undo/redo and
all OS/language combinations remain unqualified. Native HTML semantics and
labels are present; they do not by themselves establish screen-reader acceptance.
