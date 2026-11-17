# Adoption roadmap

This is an incremental shared-library project, not a simultaneous client/server
rewrite. The first deliverable proves one interaction model across native Rust,
C/SDL and browser hosts. A broader widget catalog follows real consumer needs.

## Shared contracts before consumer migrations

1. Bound chat, text, history, pending actions, prepared frames and font resources.
   Preserve exact UTF-8, transient composition, selection and input ownership.
2. Prove failure behavior: a full queue preserves the draft; modal/focus changes
   cancel controls even under backpressure; failed presentation cannot authorize
   new hit geometry; stale clipboard replies and old composition cannot mutate a
   different field.
3. Qualify all three hosts with actual rendering and input. C ABI compilation,
   Rust tests and browser boundary tests are necessary but do not establish OS
   IME, accessibility or a complete application integration by themselves.
4. Publish `tilewright-ui`, `tilewright-app` and `tilewright-ui-render` at
   `0.1.0` once their packaged graph, licensing and supported-host evidence are
   verified (see [the release plan](RELEASES.md)). The C facade stays
   unpublished; C library/header release compatibility is separate from Rust
   package versioning.

## Steps toward the first publication

1. Bring the repository current: crate metadata and READMEs, documentation,
   and Windows in CI for the three crates.
2. Move the editor's text rules into the shared crates (U+2028 and U+2029
   rejection, markers for stored line breaks) and let the shared renderer
   borrow the host's font system instead of loading its own. Nothing is
   published yet, so these may still change public APIs.
3. Release readiness: release and publish workflows modelled on
   `tilewright-world`, release notes and the owner's first-publication
   checklist. The owner then publishes the first versions and adds trusted
   publishers.
4. The Tilewright editor adopts the shared field for one string field in its
   inspector, opt-in at first.

## Consumer responsibilities

| Consumer | Shared use | Consumer-owned responsibilities |
| --- | --- | --- |
| Tilewright editor | Text controls, focus, panels, viewport composition and application lifecycle | Document capabilities, tools, World transactions, Review/Accept and undo/redo |
| Existing C/SDL game client | A C facade hosted in an existing widget slot | Existing loop/presenter, protocol commands, gameplay, old widget coexistence and capture privacy |
| Future native client for that game | Safe Rust UI/app APIs and native host adapter | Session snapshots, authorization, network protocol and domain actions |
| Future browser client for that game | Shared state/actions with a browser presenter | Browser services, transport, session and game UI policy |
| Studio game | Shared interaction and renderer integration where appropriate | Game-specific data/assets, transport, gameplay and product-specific interface policy |

Names such as `game-sdl-client` or `game-web-client` belong to consumer
executables/adapters, not dependencies of this library. A C application need not
be repackaged as a Rust crate to embed the C facade. Existing client package
names need not change merely to adopt common widgets.

## Migration order

- Use the renderer crates pinned at `0.7.1`. The UI crates have their own
  version sequence and preserve earlier immutable artifacts. Once the editor
  depends on them, a renderer update moves through the renderer, then this
  repository, then the editor.
- Prove the shared chat/input/modal slice in standalone hosts.
- Adopt one editor string inspector control, after the first publication,
  without changing document or transaction authority. Retain existing regression and native acceptance
  coverage. In parallel, add one optional local-echo chat panel to the C/SDL
  game client through its existing widget/event/deinit hooks.
  Keep its single presenter, layering, coordinate and privacy contracts. Compare
  primitive submission with a bounded renderer-produced overlay before choosing
  a wider migration.
- Adapt native and browser game clients independently. A shared UI release must
  not require every consumer to deploy together.
- Remove old implementations only after their replacement has feature parity,
  supported-platform evidence and a defined rollback path.

## Later decisions

The first browser example uses native HTML controls. A common GPU appearance can
be added independently; its IME and accessibility obligations differ. Native
assistive-technology support also needs named OS/tool qualification. These
choices do not change the shared state, action and lifecycle contracts.

Other follow-ups include docking and saved layouts, richer inspectors, gamepad
navigation, touch/mobile behavior, themes, virtualized lists and typed rich-text
spans. They remain separate milestones; chat transport and executable markup
never enter the shared widget core.
