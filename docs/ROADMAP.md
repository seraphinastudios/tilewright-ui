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
4. Publish the UI family only after its renderer dependencies, packaged graph,
   licensing and supported-host evidence are verified. C library/header release
   compatibility is separate from Rust package versioning.

## Consumer responsibilities

| Consumer | Shared use | Consumer-owned responsibilities |
| --- | --- | --- |
| Tilewright editor | Text controls, focus, panels, viewport composition and application lifecycle | Document capabilities, tools, World transactions, Review/Accept and undo/redo |
| Classic Atrinik C/SDL | A C facade hosted in an existing widget slot | Existing loop/presenter, protocol commands, gameplay, old widget coexistence and capture privacy |
| Future Atrinik native client | Safe Rust UI/app APIs and native host adapter | Session snapshots, authorization, network protocol and domain actions |
| Future Atrinik browser client | Shared state/actions with a browser presenter | Browser services, transport, session and game UI policy |
| Studio game | Shared interaction and renderer integration where appropriate | Game-specific data/assets, transport, gameplay and product-specific interface policy |

Names such as `atrinik-sdl-client` or `atrinik-web-client` belong to consumer
executables/adapters, not dependencies of this library. A C application need not
be repackaged as a Rust crate to embed the C facade. Existing client package
names need not change merely to adopt common widgets.

## Migration order

- Finish the existing renderer `0.6.0` cohort unchanged. UI/app packages use their
  own future version sequence and do not rewrite immutable renderer artifacts.
- Prove the shared chat/input/modal slice in standalone hosts.
- Adopt one editor control path without changing document or transaction
  authority. Retain existing regression and native acceptance coverage.
- Add one optional Classic panel through its existing widget/event/deinit hooks.
  Keep its single presenter, layering, coordinate and privacy contracts. Compare
  primitive submission with a bounded renderer-produced overlay before choosing
  a wider migration.
- Adapt native and browser game clients independently. A shared UI release must
  not require every consumer to deploy together.
- Remove old implementations only after their replacement has feature parity,
  supported-platform evidence and a defined rollback path.

## Later decisions

The browser presenter may use native HTML controls or a common GPU appearance;
its IME and accessibility obligations differ. Native assistive-technology support
also needs named OS/tool qualification. These choices do not change the shared
state, action and lifecycle contracts.

Other follow-ups include docking and saved layouts, richer inspectors, gamepad
navigation, touch/mobile behavior, themes, virtualized lists and typed rich-text
spans. They remain separate milestones; chat transport and executable markup
never enter the shared widget core.
