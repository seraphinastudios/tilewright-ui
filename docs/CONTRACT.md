# Prototype contracts

This document specifies the initial shared chat and input prototype. API spellings below are a coordination proposal until checked against the implemented public API; the behavioral requirements are normative. This is not a promise that all listed host capabilities have been implemented or qualified.

## Scope and ownership

The slice contains bounded chat history, one single-line composer, submit button, focus, scrolling, one modal, and a map viewport. It does not require a general widget catalog, network client, filesystem, executor, or replacement host loop. The same semantic state and actions run in native Rust, browser and a small C/SDL host. A DOM presenter and a renderer presenter may have different text pixels and wrapping while obeying the same edit/action contract.

`tilewright-ui` owns widget state, semantic input, bounded actions and control state. `tilewright-app` coordinates host lifecycle and presentation snapshots around that UI. `tilewright-ui-render` adapts neutral paint/text requests to the existing renderer. `tilewright-ui-ffi` exposes a bounded C facade. Adapters own platform event conversion; applications own effects and domain authority.

## Recommended coordination types

Use distinct checked newtypes for `WidgetId`, `WidgetGeneration`, `UiRevision`, `TextRevision`, `BatchId`, `ControlEpoch`, `SurfaceEpoch`, `FrameId`, and `GeometryEpoch`. IDs are never silently wrapped or reused during a context lifetime. C representations use fixed-width integers and checked conversion.

```rust,ignore
struct Limits {
    message_bytes: usize,
    history_messages: usize,
    history_bytes: usize,
    composer_bytes: usize,
    queued_batches: usize,
    prepared_frames: usize,
}

struct ActionBatch { id: BatchId, actions: Vec<Action> }
enum Action { SubmitChat { text: String } }
enum Disposition { Consumed, Unhandled }
enum UiError { InvalidInput, LimitExceeded, QueueFull, Stale, Exhausted }

impl Ui {
    fn new(limits: Limits) -> Result<Self, UiError>;
    fn apply(&mut self, input: Input) -> Result<Disposition, UiError>;
    fn peek_actions(&self) -> Option<&ActionBatch>;
    fn ack_actions(&mut self, id: BatchId) -> Result<(), UiError>;
    fn control(&self) -> ControlSnapshot;
    fn snapshot(&self) -> UiSnapshot;
}
```

The actual implementation may use equivalent names or fixed-capacity storage. Snapshots are immutable values or explicit immutable borrows; the FFI must state how long a returned view remains valid. `Input` distinguishes text commit, composition update/cancel, selection/edit command, focus/modal change and activation; it never conflates a text commit with a physical key event. Chat submission through Enter and the button uses the same operation.

## Atomic state and output

For every ordinary input transaction, validate all limits and reserve all required output capacity before committing any observable widget state, text revision or action. Commit the entire state/action batch, or return an error without changing those values. A submit that returns `QueueFull` preserves composer text and selection exactly and creates no action. Clearing the composer is part of the successful submit transaction, never a preceding step. Do not recover by dropping the oldest unacknowledged action.

An accepted action belongs to one immutable monotonic batch. `peek_actions` returns the oldest batch repeatedly without removing it. The host records the batch ID, accepts its actions once, and acknowledges that exact batch only after acceptance. Acknowledgment of the oldest batch removes it; duplicate acknowledgment of an already acknowledged ID is idempotent; acknowledgment of a future/out-of-order ID fails. A host that retries a batch must deduplicate by context identity plus batch ID. This guarantees no loss or duplication through normal backpressure/retries, not exactly-once remote effects across a process crash. Remote idempotency remains application-owned.

For this slice, one submit can be one batch containing one action; do not introduce a general transaction engine. Read-only snapshots and preparation never acknowledge actions. Changing focus, rendering, resizing or suspending must not drain accepted actions. Explicit final context destruction ends their lifetime; the host decides whether to accept pending actions first.

| Operation | If capacity/validation fails | If accepted |
| --- | --- | --- |
| Submit | Composer, selection, revision and queue unchanged | One immutable batch; composer clear and revision change committed together |
| Receive chat message | Existing history unchanged | Apply selected bounded retention policy and append atomically |
| Text/preedit edit | Text, preedit and revision unchanged | Entire checked edit commits |
| Acknowledge action | Queue unchanged for unknown/future ID | Remove exactly the oldest batch; repeat acknowledgment harmless |
| Prepare frame | Displayed snapshot unchanged | Retain one immutable prepared snapshot within the frame bound |

## Cancellation cannot wait behind ordinary traffic

Maintain a latest-value `ControlSnapshot` separately from ordinary action queues:

```rust,ignore
struct ControlSnapshot {
    epoch: ControlEpoch,
    world_input_allowed: bool,
    focused_widget: Option<(WidgetId, WidgetGeneration)>,
    modal_open: bool,
    suspended: bool,
}
```

Any transition that can revoke ownership or invalidate held controls increments the epoch. The host compares epochs after each input/lifecycle call and before applying world controls; an epoch change cancels all held world controls and pointer captures before new controls are accepted. The epoch is a durable cancellation barrier even when the final `world_input_allowed` value becomes true again before the host samples it. Do not only cancel when the sampled Boolean is false.

Control updates require no action-queue slot and overwrite a bounded latest-value snapshot. Closing a widget, losing window focus, entering a modal, suspension and context shutdown cannot fail merely because chat output is full. Within the initial slice these operations emit no ordinary action. If future operations combine ordinary output with safety cancellation, cancellation still takes effect immediately; the rejected ordinary transaction must not roll that barrier back. This is an explicit safety transition, not a partial accepted submit.

Physical key/button ownership remains attached to a press until release or cancellation. A release belonging to a cancelled or widget-owned press is consumed/quarantined, not reinterpreted as world input. A focused text widget owns down/up/text/composition events. Platform adapters must not independently run world shortcuts against an already consumed event.

The physical ownership ledger is safety state, separate from ordinary widget
transactions. A failed submission still records ownership of its triggering
press, so subsequent repeats or releases cannot escape to world controls after
focus changes. Its bounded representation covers the entire admitted key domain;
ordinary action-queue pressure must not make ownership recording fallible.

`KeyEvent.physical` identifies a physical press independently of its semantic
`key`. Return and keypad Enter may request the same command but must keep
different physical identities. Native adapters use scancodes; browser adapters
map physical `KeyboardEvent.code` values, not text or layout-dependent labels.
Helpers such as visual caret movement and keyboard clipboard operations run only
after successful key routing and a positive `can_apply_key_effect(physical)`
check. `Consumed` alone is insufficient: a quarantined press is also consumed.

`platform_composing` is conservative metadata about that key event. It preserves
the Enter fence when a platform reports composition without visible preedit
text, including browser `Process`/229 events and native IME event batches. It is
not a persistent composition model. Presenters still track their own native
composition lifecycle and reject submission while it is active. Keyup releases
only the matching physical key's fence. Explicit active composition cancellation
advances the control epoch; ordinary empty-preedit completion and text commit do
not, so a legitimate final commit remains possible.

For future async effects, every request carries context/widget generation, request ID and expected revision. Closing its owner immediately invalidates acceptance independently of worker completion. A full completion inbox returns `QueueFull`; the producer retains the completion for retry or explicitly cancels the request, and the configured outstanding-work budget bounds both. Never silently drop a successful completion or allocate an unbounded retry list. Apply lifecycle/cancellation barriers before queued completions, then process completions in accepted sequence order and ordinary input in host-supplied order. Do not add background workers for this prototype.

## Prepared geometry is not displayed geometry

```rust,ignore
struct PresentationStamp {
    surface: SurfaceEpoch,
    frame: FrameId,
    geometry: GeometryEpoch,
    ui_revision: UiRevision,
}
struct PreparedFrame {
    stamp: PresentationStamp,
    // Immutable paint, bounds, transforms, clips, hit regions and text layouts.
}
impl App {
    fn prepare(&mut self, viewport: Viewport) -> Result<PreparedFrame, AppError>;
    fn ack_presented(&mut self, stamp: PresentationStamp) -> Result<(), AppError>;
    fn invalidate_surface(&mut self) -> Result<SurfaceEpoch, AppError>;
    fn displayed(&self) -> Option<&DisplayedFrame>;
}
```

Preparing, submitting or successfully encoding a frame does not make it displayed. The host calls `ack_presented` only after the matching presentation succeeds. Accept an acknowledgment only for a retained prepared frame in the current live surface epoch, with its exact stamp, and newer than the last acknowledged frame. Duplicate acknowledgment of the current frame may be idempotent; older and unknown frames fail. Hosts must serialize actual presentation order and must not display an older frame after a newer one. Reordered completion notifications cannot roll geometry back.

Hit testing, pointer text selection, hover and IME candidate positioning use the last valid displayed snapshot's transforms, clips and text geometry. The resulting target must still match the live widget generation and relevant text/UI revision. Reject stale geometry-dependent input until a valid frame is acknowledged; never resolve it against a newly prepared invisible frame. Non-geometric keyboard edits may continue through the focused widget and normal revision validation.

Resize, DPI/host-transform changes, surface loss/recreation and viewport attachment changes invalidate displayed geometry and increment the surface epoch before further geometric input. Ordinary scroll/layout changes produce a new geometry epoch; old displayed geometry may remain available only if its target revisions are still valid. A conservative implementation may reject geometric input on any UI revision mismatch. Failed presentation does not promote new geometry. A late acknowledgment from a retired epoch cannot restore it.

Bound retained prepared/in-flight snapshots. A full frame budget yields backpressure or discards only a frame the host guarantees was never submitted; it cannot release resources still used by a displayed/in-flight frame. Retain the displayed snapshot and any referenced atlas/layout resources until a replacement is acknowledged or the surface is explicitly invalidated. Frame resources and widget state have separate lifetimes.

The equivalent DOM boundary is a host-confirmed layout commit: after applying the shared snapshot, update/measure current element bounds, scroll positions, clips and transforms together and acknowledge that exact revision. A pending DOM patch is not current hit geometry. Native element event targets may identify the element directly, but ID/generation/revision checks still apply. Do not claim an animation callback proves literal monitor scanout; the contract is the presenter’s successful commit boundary.

The example DOM presenter retires its native input node and listeners when a
composition session is explicitly cancelled. Late events for the detached node
cannot acquire the new node's target identity. Pointer, key or paste activity is
not evidence that old composition callbacks have drained.

## Neutral draw and text model

Start with an ordered bounded paint list: clipped colored rectangles, text runs with stable IDs/font role/style, and a viewport placeholder. Store logical-coordinate viewport dimensions and scale explicitly. Give hit regions separate semantic widget IDs; do not infer them from colored pixels. The renderer adapter compiles this output with existing renderer typography/shapes and paints the world beneath the designated UI region. It does not give the UI renderer GPU objects or world authority.

For renderer-painted text, caret/selection geometry comes from the same immutable shaped layout used to paint that run, carrying widget generation, text revision and presentation stamp. UTF-8 byte positions must be valid checked grapheme boundaries; visual affinity is explicit when available. Native SDL preedit offsets and browser UTF-16 selection offsets are translated and validated in their adapters. Preedit is separate from committed text and cannot submit a message. Enter while composing must not leak to submit or world controls.

A DOM presenter uses the browser’s text layout and maps native edit/selection results into the same checked model. It must not combine renderer caret geometry with browser-painted glyphs or apply one edit twice through both `keydown` and text-input events. Browser DOM versus GPU presentation is a capability choice; implementation must not claim unqualified parity. Preserve a semantic accessibility tree and keyboard behavior in either case.

## C boundary

Expose ABI version negotiation, sized structs, fixed-width fields, status codes, bounded UTF-8 spans and opaque context handles. No Rust allocation/container, backend object, SDL ownership or callback unwinding crosses the ABI. Input spans are copied during the call unless explicitly documented otherwise; library allocations are freed by the library. Validate pointer nullability and semantic sizes, but valid accessible C pointers remain a caller obligation.

The host owns the SDL window and event loop. Calls occur on the owning thread; reject reentrancy. Rust unwinding cannot cross C; where unwind recovery is supported, poison the context after an internal panic and return a typed failure. Abort/OOM is not made recoverable by this mechanism. A destroyed opaque pointer cannot be reused or destroyed twice. Repeated create/use/destroy cycles, rather than use-after-free, are the required lifecycle test.

The C facade preserves peek/ack output and control-epoch semantics. Copying output into a caller buffer must not dequeue it; `BufferTooSmall` preserves it. Frame/geometry stamps cross the boundary intact. The small C host proves linkage and real event/render behavior, while integration with an existing application's compositor is a separate acceptance step.

## Required regression cases

- Fill the action queue; submit with Enter and button; verify unchanged composer/revision, repeat peek, one acknowledgment, and exactly one accepted retry.
- With the queue full, enter/exit a modal while a world key is held; sample only the final control snapshot and verify epoch-based cancellation still happens. Destroy focused/captured widgets without stuck controls.
- Inject malformed/oversized text and saturated history; verify atomic rejection or documented atomic retention. Test combining marks, emoji sequences, RTL text and composition cancellation.
- Fail and reorder presentation around scrolling, editing, resize and surface recreation. Click before acknowledgment; verify no invisible geometry receives input and retired stamps cannot revive it.
- Verify IME positioning uses the current acknowledged transform; exercise a pending DOM patch and a stale native text-layout snapshot.
- Saturate prepared-frame/output-buffer capacity and verify bounded retention without loss. Run C17 header/link and repeated lifecycle tests, plus the supported host-specific checks.
