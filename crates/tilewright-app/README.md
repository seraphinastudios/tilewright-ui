# tilewright-app

Host-neutral application coordination for `tilewright-ui`: lifecycle,
cancellation, prepared and displayed frames, and presentation stamps. A host
acknowledges a frame only after it has actually presented it, so input is
never applied to geometry the user has not seen. The host keeps its event
loop, executor, windows and domain authority.

See the [repository](https://github.com/seraphinastudios/tilewright-ui) for
the architecture, the interaction contract and the native, C and browser
examples.

## Status

This is a 0.x prototype. Its API is not stable. It is published on crates.io
and shares one version with `tilewright-ui` and
`tilewright-ui-render`; a host
that uses more than one uses them at the same version. While they are at 0.x, a breaking change to a public API, or
a move to the renderer's next minor version, takes the next minor version;
anything else takes the next patch version. Changes per version are in
[docs/RELEASES.md](https://github.com/seraphinastudios/tilewright-ui/blob/main/docs/RELEASES.md#crate-versions).

Licensed under the MIT license.
