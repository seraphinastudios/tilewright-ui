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

## Status

This is a 0.x prototype. Its API is not stable, and the crate is not yet
published to crates.io. It shares one version with `tilewright-app` and
`tilewright-ui-render`. While they are at 0.x, a breaking change to a public
API, or a move to the renderer's next minor version, takes the next minor
version; anything else takes the next patch version.

Licensed under the MIT license.
