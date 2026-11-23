# tilewright-ui

Headless widget state for Tilewright hosts: a bounded chat history, a Unicode
text composer with a single-field mode, caret, selection and IME preedit,
focus, input ownership and acknowledged actions. It has no SDL, DOM, GPU,
network, filesystem or game dependency. The host keeps its event loop,
clipboard, rendering and domain authority.

`tilewright-app` coordinates this state with a host's frames, and
`tilewright-ui-render` draws it through the Tilewright renderer. See the
[repository](https://github.com/seraphinastudios/tilewright-ui) for the
architecture, the interaction contract and the native, C and browser examples.

## Reading and laying out the field

`Ui::snapshot` copies the whole state. To read one value, use
`Ui::composer_text` (the committed text, borrowed), `Ui::composer_revision`,
`Ui::composer_selection` or `Ui::is_composing`; each equals the matching
`snapshot().composer` value at the moment of the call.

`layout_in` lays out an embedded panel with its submit control (Apply in
`SingleField`, Send in `ChatPanel`). A host with its own control calls
`layout_in_with` with options whose `submit_control` is `false`: the composer
takes the whole inner width, and the host submits with
`Input::Activate(SUBMIT)` or Enter. `LayoutOptions` is `#[non_exhaustive]`, so
start from its default (the `LayoutOptions` doc test runs this):

```rust,ignore
let mut options = LayoutOptions::default();
options.submit_control = false;
let layout = layout_in_with(&ui.snapshot(), viewport, bounds, clip, &options)?;
```

The options' `field_style` (a `FieldStyle`, since 0.1.2) sizes and colours
the single field: its text size, the composer box's height and inset, the
text's padding inside the box, and the colours of the text, the box (or no
box, for a host that draws its own background), the selection, the caret and
the preedit underline. The default is the look of earlier versions. The layout
keeps the style (`Layout::field_style`), and `tilewright-ui-render` shapes,
measures, hit-tests and draws the field's text with it, so a layout and its
frame always agree. Only `SingleField` takes a style other than the default;
nonsensical values are refused with `InvalidInput`. `FieldStyle` is
`#[non_exhaustive]` too (its doc test runs this):

```rust,ignore
let mut options = LayoutOptions::default();
options.submit_control = false;
options.field_style.text_size = 18.0;
options.field_style.inset = 0.0;
options.field_style.height = 30.0;
options.field_style.padding_left = 4.0;
options.field_style.box_color = Some([0.12, 0.32, 0.47, 1.0]);
let layout = layout_in_with(&ui.snapshot(), viewport, row, row, &options)?;
```

## Status

This is a 0.x prototype. Its API is not stable. It is published on crates.io
and shares one version with `tilewright-app` and
`tilewright-ui-render`; a host
that uses more than one uses them at the same version. While they are at 0.x, a breaking change to a public API, or
a move to the renderer's next minor version, takes the next minor version;
anything else takes the next patch version. Changes per version are in
[docs/RELEASES.md](https://github.com/seraphinastudios/tilewright-ui/blob/main/docs/RELEASES.md#crate-versions).

Licensed under the MIT license.
