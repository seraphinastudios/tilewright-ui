# Shared UI and application foundations

This repository owns reusable MIT widget behavior and host-neutral application
coordination. Read `docs/ARCHITECTURE.md` and `docs/CONTRACT.md` before changing
interfaces. Public names and version numbers do not imply a stable or published
release; packages remain unpublished until their release is explicitly approved.

- `tilewright-ui` owns headless widget/text/focus semantics. No SDL, DOM, GPU,
  network, filesystem, game protocol, editor document or consumer dependency.
- `tilewright-app` coordinates bounded host effects, lifecycle and command
  contexts. The host keeps its event loop, executor, windows and domain authority.
- `tilewright-ui-render` adapts to the existing released Tilewright renderer.
  Reuse its shaping, glyph resources and drawing; never add a second text engine.
- `tilewright-ui-ffi` alone owns audited C exports. Keep Rust internals and native
  GPU/SDL types out of the ABI; document pointer, allocator, thread and lifetime
  contracts. No unwinding across C. Other logical crates forbid unsafe code.
- Native/browser adapters and examples begin unpublished. The C host owns SDL
  initialization, event polling and presentation. A standalone host does not
  establish Classic client integration or production performance.
- Input ownership, transient IME, exact UTF-8, queued output/backpressure,
  cancellation and acknowledged presentation identities are correctness rules.
  Never execute game actions against stale or undisplayed geometry.
- MIT editor code may be extracted only with exact source attribution and
  provenance. Do not copy/adapt Classic GPL or proprietary game implementations,
  assets, fixtures or markup. Use independently authored neutral examples.
- Fonts retain their own licenses and explicit verified delivery. No implicit
  system-font search, runtime download or arbitrary project execution.
- Pin Rust, dependency graph and Actions; keep renderer requirements registry
  based. Development overrides stay outside committed manifests/config/locks.
- Preserve the independent renderer 0.6.0 release and its immutable artifacts.
  New names need their own reviewed release/ownership/publication plan.

Use owned worktrees and preserve other workers' edits. Every PR opens as draft,
has an intent brief and fresh-context independent review, then uses the installed
`pr-delivery` self-review and readiness gates. The owner merges. Run formatting,
targeted regression tests, strict Clippy, dependency/architecture checks, then
the relevant native, C and browser proofs. Record actual capabilities and gaps;
do not infer assistive-technology or OS IME acceptance from unit tests.
