# Rust SDL3 prototype host

This unpublished host owns its SDL3 window, event loop, clipboard, procedural map
and single presentation. Shared UI owns the bounded chat/composer, selection,
composition, focus/modal state, key ownership and actions. Shared renderer 0.6
shapes and paints the overlay through its CPU reference path. This is a bounded
CPU demonstration, not GPU performance or Classic integration acceptance.

Supply each font’s exact internal family name, explicitly licensed file, and trusted manifest digest:

```sh
cargo run -p tilewright-ui-native-demo -- --font "Internal Family Name" /path/font.ttf SHA256
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

The host advertises only composition rendering through the checked SDL hint
before SDL initialization; the OS owns candidate UI. A conflicting environment
hint is overridden and verified.

For an authorized private acceptance run, `--trace NEW_PATH` writes a newly
created 0600 TSV file. It refuses existing files and symlinks. The trace contains
exact draft/preedit/action bytes (hex), so keep it private. Both hosts share the
`twui-trace-v1` schema: event records contain name, rejected, consumed, composer
hex, preedit hex, control epoch, world allowed, held-key count, focused, modal,
anchor byte, caret byte, scroll and history count. Action records contain batch
ID and text hex; present records contain surface/frame/geometry/UI revision.
Limits are 16,384 records and 4 MiB; saturation fails the run explicitly.
