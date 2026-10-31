# C17 SDL3 prototype host

This host consumes the same shared UI and CPU reference renderer through the
versioned C facade. It owns SDL initialization, events, clipboard, the neutral
procedural map and one presentation per frame. It does not implement a separate
widget model or text painter.

Build the unpublished Rust library with `--features cpu-prototype`, then use an
explicit pinned SDL3 3.4.18 development prefix:

```sh
cargo build -p tilewright-ui-ffi --features cpu-prototype
cmake -S examples/c-sdl -B /tmp/twui-c \
  -DCMAKE_PREFIX_PATH=/path/to/pinned-sdl3 \
  -DTILEWRIGHT_UI_LIBRARY="$PWD/target/debug/libtilewright_ui_ffi.so"
cmake --build /tmp/twui-c
/tmp/twui-c/tilewright-ui-c-sdl --font "Internal Family Name" /path/font.ttf SHA256
```

No host packages are installed by this example. Font bytes are caller-supplied;
Family names and SHA256 must come from the trusted font manifest. Multiple `--font FAMILY PATH SHA256`
arguments form a fallback pack. Use only licensed fonts. `--frames N` exits after
N successful presents on an owned test display.

Tab toggles text focus; F1 toggles the modal; Enter/Send submits to local echo.
Ctrl+A/C/X/V selects/copies/cuts/pastes. SDL preedit offsets are scalar positions;
all conversions and edits run in the shared core. Window/session filtering,
press identity, cancellation epochs and exact presentation stamps fence input.
Both hosts composite premultiplied reference pixels over the same authored map
in integer RGBA, then upload one opaque texture and acknowledge after present.
This qualifies neither production compositor performance nor OS accessibility.

The host advertises only composition rendering through the checked SDL hint
before SDL initialization; the OS owns candidate UI. A conflicting environment
hint is overridden and verified.

The optional `--trace NEW_PATH` uses the same private bounded TSV schema as the
Rust host. It requires a new 0600 file, refuses symlinks/existing files, and fails
on saturation (16,384 records/4 MiB). Keep exact text evidence private.
`SDL_IME_IMPLEMENTED_UI=candidates ./tilewright-ui-c-sdl --check-ime` verifies the
pre-initialization override in a fresh process without creating a window.
