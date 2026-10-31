# Rust SDL3 prototype host

This unpublished host owns its SDL3 window, event loop, clipboard, procedural map
and single presentation. Shared UI owns the bounded chat/composer, selection,
composition, focus/modal state, key ownership and actions. Shared renderer 0.6
shapes and paints the overlay through its CPU reference path. This is a bounded
CPU demonstration, not GPU performance or Classic integration acceptance.

Supply one or more explicitly licensed fonts and their trusted manifest digests:

```sh
cargo run -p tilewright-ui-native-demo -- --font /path/font.ttf SHA256
```

There is no system-font discovery, download or digest inference. `--frames N`
exits after N successful presents for an isolated display harness. Launch only
on an owned test display. Tab toggles composer focus, F1 toggles the modal,
Enter/Send submits to a local echo, and Ctrl+A/C/X/V selects/copies/cuts/pastes.
Use the wheel to scroll history. The map is independently authored geometry.

Text and preedit events are filtered to the window and current text session.
SDL scalar preedit offsets go to the shared checked converter. Key release uses
the semantic identity captured on keydown even when modifiers change. Every
control epoch cancels held map keys. Renderer geometry becomes usable only after
successful SDL presentation; resize and surface events invalidate it first.

Native accessibility and OS IME/candidate-window behavior require actual host
qualification. A compile or injected-event check does not establish those.
