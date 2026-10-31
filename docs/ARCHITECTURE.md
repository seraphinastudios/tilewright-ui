# Shared UI and application architecture

Tilewright UI provides reusable interaction and host-neutral application coordination. Its first proof is one bounded chat/composer, focus/modal, scrolling and map-viewport slice running in native Rust, a browser and a small C/SDL host. Implement this slice before expanding the widget catalog or migrating entire applications.

## Dependency direction

```text
consumer application and domain adapters
    -> tilewright-app       -> tilewright-ui
    -> tilewright-ui-render -> tilewright-ui
                            -> existing Tilewright renderer libraries
    -> native/web adapters  -> tilewright-app + tilewright-ui
C consumer
    -> tilewright-ui-ffi    -> tilewright-app + tilewright-ui
                           -> optional presentation adapter
```

The existing renderer never depends on the UI/application library or consumers. World/document/network adapters remain in consumers. No shared UI crate acquires filesystem or game authority.

| Package | Ownership |
| --- | --- |
| `tilewright-ui` | Headless widget state, semantic input, focus/capture, editing/composition, bounded chat, accessibility semantics, neutral paint descriptions and acknowledged semantic actions. Widget core and simple widgets begin as modules rather than separate public packages. |
| `tilewright-app` | Portable application lifecycle, control cancellation coordination, bounded effect protocol, scheduling hints and prepared/displayed presentation bookkeeping. It composes UI without owning the host loop, process, executor or network. |
| `tilewright-ui-render` | Translation of neutral UI output through existing Tilewright renderer shapes, fonts, Unicode shaping and resources. It owns no second text engine or consumer world model. |
| `tilewright-ui-ffi` | Versioned C-facing ownership, buffers, IDs, statuses and event/action/presentation calls. Platform-native ownership stays in a narrow host adapter. |

Native and browser adapters begin unpublished. Separate target dependencies prevent SDL/GPU dependencies from entering headless builds and native libraries from entering browser builds. Promote a platform package only after demonstrated consumer reuse. Do not add standalone chat, theme, input, accessibility, async or runtime crates for the prototype.

The application/UI split permits a widget-only consumer and gives different hosts one portable lifecycle/output contract. UI has no backedge to app; app consumes UI actions/snapshots. Renderer and FFI code must not become alternate implementations of editing or routing. [CONTRACT.md](CONTRACT.md) defines atomic output, control cancellation and acknowledged geometry boundaries.

## One model, explicit presenters

State and semantic actions are shared; presentation can vary. A renderer presenter uses the existing renderer's typography, geometry and resources. A DOM presenter can use native browser inputs and semantic elements beside a rendered world viewport. Both identify widgets and revisions consistently, implement the same submission/focus contract, and qualify composition and accessibility on actual hosts. Exact glyph pixels and line wrapping across browser and GPU text are not promised.

Browser-native editing requires an explicit adapter between DOM selection/composition and the shared model, not a second independent state store. Renderer-painted editing requires geometry from the exact shaped run displayed. Both require a presenter commit boundary before geometric input, with stale-generation/revision rejection.

The world viewport is a rectangle/clip and input region. The consumer supplies scene/resources and decides what is visible, selectable and authorized. Widgets produce intentions, not gameplay commands or direct document writes. Chat is bounded text and typed style spans; protocol markup, network transport, identity and link policy remain consumer-owned.

## Incremental embedding

Hosts retain initialization, event pumping, windows, present order, shutdown and async execution. An application can replace one panel without replacing its world renderer. Existing compositors may consume a bounded primitive/atlas packet, or a renderer-produced RGBA overlay after explicit color/alpha/clip and performance qualification. Text shaping and glyph production remain in the shared renderer; the host performs primitive/texture submission only.

Insert a shared panel at its actual z-position in the host widget tree, preserving existing modal layers. Keep one presenter per surface. Do not expose backend GPU handles or assume portable zero-copy sharing. A standalone C host is a necessary portability proof but does not establish compatibility with every existing compositor.

## Licensing and release boundaries

New shared implementation is MIT. Reuse only materials with recorded compatible provenance; no GPL or proprietary consumer implementation is copied or adapted into this repository. Consumer behavior can be specified independently as generic requirements. Fonts/assets retain their own exact licenses, provenance and notices.

The existing renderer source release and approved `0.6.0` package artifacts remain unchanged. This repository has an independent release sequence. During development, any necessary local renderer overrides stay task-local; production dependency manifests use immutable released versions. Initial source/package preparation does not authorize registry publication or imply a supported C ABI has already been frozen.

## Delivery gates

1. Prove the headless state/output and presentation contracts with deterministic failure/backpressure tests.
2. Run the same chat/input/focus slice through native Rust, browser and a real C17/SDL host. Add actual IME, keyboard and presentation-failure evidence, not merely compile checks.
3. Qualify renderer/DOM capabilities and accessibility explicitly. Unsupported combinations remain documented limitations.
4. Migrate consumer panels incrementally through their own adapters and validation. Keep protocol, persistence and world-renderer changes independently scoped.
5. Before public package/ABI release, reconcile implemented APIs with the contract, check dependency/provenance graphs, package contents and exact release versions, and retain independent review.

Capability support is reported separately from compile-time features. A successful build does not establish native screen-reader support, browser clipboard permission, IME behavior or compositor performance. See [CONTRACT.md](CONTRACT.md) for the concrete minimum regression cases.
