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

Who owns the text system. A host with no text engine of its own uses `UiRenderer`, which owns one `TextSystem` built from the verified `FontPack` it is given. A host that already owns a `TextSystem` (the renderer crates' text engine) keeps owning it and lends it to `UiCompiler::compile` as `&mut TextSystem` for the length of each call. The compiler holds only its namespace, budgets and the last atlas revision it was issued: it reads, parses and caches no font, keeps no reference past the call, and needs no lock, `unsafe` code or global state. The shared widgets are then shaped, measured and rasterized by the host's own engine, fonts and caches, so their caret positions and widths are exactly those the host's own text gets for the same string and size. The lent system's fonts and text budgets apply. Frames, layouts and atlas bytes are immutable and outlive the call, the compiler and the text system alike.

Host rules for a lent text system:

- Reserve the compiler's atlas ID as one the host can never compute for its own atlases. A host that derives its own IDs from its content (for example the largest world ID plus a small offset) reserves the shared IDs far above any derived ID and away from sentinel values. `CompiledFrame::compose` rejects a host scene that uses the namespace's IDs with `IdentityCollision`.
- Lend the same text system on every call to a given compiler. Atlas revisions rise per text system, so a second system could reissue an old revision of the compiler's atlas ID; the compiler refuses a revision lower than one this compiler was issued before, or the same revision with different bytes (`TextSystemChanged`), and a refused compile changes nothing. Given a reserved atlas ID, a compiler therefore never returns two different atlases under one (ID, revision). To switch text systems on purpose, for example after reloading fonts, retire every frame carrying this compiler's atlas ID and create a new `UiCompiler`.
- The host's own text and the compiler may alternate on one system. The renderer keeps an unchanged atlas's exact identity per atlas ID, whatever other IDs were compiled in between, so alternating compiles re-upload nothing. One condition: the text system remembers only a small fixed number of recently used atlas IDs. An ID compiled again after more other atlas IDs than that, or any ID after `TextSystem::clear_cache`, gets a new revision for unchanged bytes. That costs a re-upload, never correctness.

A DOM presenter uses the browser’s text layout and maps native edit/selection results into the same checked model. It must not combine renderer caret geometry with browser-painted glyphs or apply one edit twice through both `keydown` and text-input events. Browser DOM versus GPU presentation is a capability choice; implementation must not claim unqualified parity. Preserve a semantic accessibility tree and keyboard behavior in either case.

## C boundary

Expose ABI version negotiation, sized structs, fixed-width fields, status codes, bounded UTF-8 spans and opaque context handles. No Rust allocation/container, backend object, SDL ownership or callback unwinding crosses the ABI. Input spans are copied during the call unless explicitly documented otherwise; library allocations are freed by the library. Validate pointer nullability and semantic sizes, but valid accessible C pointers remain a caller obligation.

The host owns the SDL window and event loop. Calls occur on the owning thread; reject reentrancy. Rust unwinding cannot cross C; where unwind recovery is supported, poison the context after an internal panic and return a typed failure. Abort/OOM is not made recoverable by this mechanism. A destroyed opaque pointer cannot be reused or destroyed twice. Repeated create/use/destroy cycles, rather than use-after-free, are the required lifecycle test.

The C facade preserves peek/ack output and control-epoch semantics. Copying output into a caller buffer must not dequeue it; `BufferTooSmall` preserves it. Frame/geometry stamps cross the boundary intact. The small C host proves linkage and real event/render behavior, while integration with an existing application's compositor is a separate acceptance step.

## Embedded panels and field binding

Each context has an immutable `Composition`: `Demo`, `ChatPanel`, or
`SingleField`. Existing constructors retain `Demo`. Embedded contexts contain
one editable slot; hosts use separate contexts for independently bound fields.
`ChatPanel` draws history, composer and Send. `SingleField` draws the value and
Apply, while the host owns its caption, validation and review interface. Neither
embedded mode emits a world hit or paints a modal over host windows. The host
still arbitrates events outside the panel and other focused controls.

A Rust host that draws its own submit control lays the panel out with
`layout_in_with` and `LayoutOptions` whose `submit_control` is `false` (since
0.1.1): the composer then takes the whole inner width, and the layout holds no
Apply or Send paint and no submit hit region. Enter and `Input::Activate(SUBMIT)`
still submit. `layout_in` and every existing caller keep the control; `Demo`
refuses the option with `InvalidInput`. The C facade and the web adapter always
lay out the control.

`LayoutOptions::field_style` (since 0.1.2) sizes and colours the single field.
Its default reproduces earlier versions exactly; each field and its default:

| `FieldStyle` field | Default | Effect |
| --- | --- | --- |
| `text_size` | `20.0` | Composer text size in logical px, for shaping, measurement, hit-testing, the caret, the IME area and `step_caret` |
| `height` | `36.0` | Composer box height, at most the attachment height less twice the inset; the box sits at the bottom |
| `inset` | `8.0` | Space between the attachment's edges and the box (and the submit control), at most half the attachment's width and height |
| `padding_left`, `padding_top` | `0.0` | Offset of the text origin from the box's top-left corner |
| `padding_right` | `0.0` | When the text scrolls to keep the caret visible, the caret's right edge stays this far from the box's right edge |
| `text_color` | `[1.0; 4]` | Composer text |
| `box_color` | `Some([0.18, 0.2, 0.25, 1.0])` | Box fill; `None` paints no box, for a host that draws the background |
| `selection_color` | `[0.23, 0.40, 0.62, 1.0]` | Selection highlight, under the text |
| `caret_color`, `preedit_color` | `None` | Caret and preedit underline, over the text; `None` uses `text_color` |

The layout uses the geometry and the text and box colours, and keeps the
validated style in the `Layout` (`Layout::field_style`, read-only).
`UiCompiler::compile` reads the text size, padding and remaining colours from
there, so a frame is always drawn with the style its layout was made with.
Text, selection, caret and underline stay clipped to the box (intersected with
the clip), padding included. `layout_in_with` refuses with `InvalidInput`, and
lays nothing out, when a length is not finite, is negative or exceeds 8192, when
`height` is zero, when `text_size` is outside 1–256, or when a colour channel is
not finite or outside 0–1. Only `SingleField` takes a style other than the
default: `ChatPanel` and `Demo` refuse one with `InvalidInput`, since their
composer shares its panel with history and fixed rows. The Apply control's
width (80), its gap (8), colours and label, the history rows and the 18 px body
and button text are fixed. The C facade, the web adapter and `App` use the
default style.

Reading one value of the live state does not need a snapshot: `Ui::composer_text`
(borrowed committed text), `Ui::composer_revision`, `Ui::composer_selection` and
`Ui::is_composing` (since 0.1.1) equal the corresponding `snapshot().composer`
values at the moment of the call. Like a snapshot they describe the current
state, not displayed geometry.

A host that composes the shared frame into its own scene under its own limits
reads `CompiledFrame::quad_count` to budget for the shared quads and composes
with `CompiledFrame::compose_with_limits` (since 0.1.1); `compose` uses
`SceneLimits::default()`. `CompiledFrame::raster_sample_count` (since 0.1.2)
gives the frame's raster samples as `SceneLimits::maximum_raster_samples`
counts them (each quad's bounding box, or its two triangles' boxes when it is
not an axis-aligned rectangle, clipped to the frame's physical viewport in
whole pixels, times the glow factor, summed with saturation):
`compose_with_limits` accepts the frame over a world exactly when the world's
samples plus this count are within the limit, and the other limits hold.

Call `set_layout` with the host viewport, panel bounds and clip before preparing
an embedded frame. A changed supplied attachment invalidates old displayed
geometry immediately, including a clip change with an identical effective
intersection. Invalid geometry leaves state unchanged. `prepare_current`
captures the configured attachment; acknowledge it only after the host's whole
frame is successfully presented. External screen-transform changes require
explicit invalidation even if local coordinates remain equal.

`bind_field` atomically loads a bounded value while focused or unfocused.
The value may contain stored line breaks and control characters (see
[Single-line text](#single-line-text)); only its byte length can refuse it. It
changes widget generation, retires focus/preedit and advances cancellation
identity, while retaining pending immutable actions. Rebinding identical text
also changes generation. Invalid text or exhausted identities leave the old
binding untouched. The host maps the returned widget identity to its document,
property and captured source revision.

`SubmitField` accepts an empty value and retains the exact draft and selection.
Its action carries widget generation and source text revision. Acknowledging the
action transfers responsibility to the host's bounded review processing; the
host separately decides validation and document commit. Late review results
must not replace a newly edited or rebound value. Consumers inspect actions
after input even when no visual revision changed.

The C facade adds opt-in functions and sized structs while retaining ABI 2 and
the existing layouts. A legacy-created Demo context continues to emit only
chat actions. New field actions expose their binding identity through
`twui_get_field_action`. See the installed header for exact buffer ownership.

## Single-line text

Every composer is single-line. New input never adds a line break, while a
stored value that already holds one opens, displays and returns unchanged.

| Case | Rule |
| --- | --- |
| Refused new input | Any control character (`char::is_control`: C0, DEL and C1, so LF, CR, tab, VT, FF, NEL, NUL), U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR. `refuses_single_line` is the predicate. |
| Where refused | `TextCommit` (typing, host paste), `Preedit` and an IME commit, `ReplaceText`. The whole text is refused: nothing is stripped or partly inserted. |
| What refusal looks like | `UiError::RefusedText` (C: `TWUI_REFUSED_TEXT`, 14). Text, selection, preedit and revisions are unchanged; a rejected IME commit keeps its composition. An insertion past `composer_bytes` is `LimitExceeded` instead. When text is both over the remaining budget and holds a refused character, `RefusedText` wins: trimming it would not help. Exception in C: the facade bounds a span before reading it, so a span longer than `composer_bytes` (`message_bytes` for RECEIVE) is `TWUI_LIMIT_EXCEEDED` whatever it holds. `UiError` is `#[non_exhaustive]`. |
| Received chat messages | `ReceiveMessage` refuses the same characters with `RefusedText` (one history row is one line), then checks `message_bytes` with `LimitExceeded`. |
| Not refused | Format characters such as ZWJ, ZWNJ, ZWSP and bidirectional marks, and the marker glyphs themselves typed as ordinary text. |
| Opening a stored value | `TextInput::new` and `bind_field` accept refused characters losslessly. Only a value over the byte budget is refused (`LimitExceeded`). |
| Display | `display_text` shows each stored character, or CR LF pair, as one marker grapheme: `⏎` (U+23CE) for LF, CR, CR LF, VT, FF, NEL and U+2028; `¶` (U+00B6) for U+2029; `→` (U+2192) for tab; `⬚` (U+2B1A) for any other control. A zero-width space (U+200B) is added beside a marker only when a prepended or combining neighbour would otherwise join its grapheme; it belongs to the marker and is never stored. A stored marker glyph looks the same as a marker but is not in `markers`. |
| Marker positions | `TextInputSnapshot::markers` lists each marker in order: `source` (bytes in the committed text with any preedit spliced in), `display` (the whole unit in `display_text`, guards included) and `glyph` (the visible glyph). Renderers that style markers use these; the headless crate draws nothing. |
| Caret, selection, deletion | A marker is one grapheme. A caret cannot sit inside a CR LF pair. Backspace or Delete beside a marker removes exactly its stored character (both bytes of CR LF). A selection over a marker covers the stored characters; hosts copy from `committed`. `display_selection` and the composition's `display_range` are in display coordinates; `committed_cursor` maps a display position back, and a position beside a guard maps to the stored character's edge on that side of the glyph. |
| Composition | Preedit is spliced before marking, so markers around it keep their places and a combining preedit next to a marker gets a guard. Preedit can never hold a refused character. |
| What the host receives | `committed`, `SubmitField` and the C copy functions carry the exact stored bytes, markers never. An untouched value round-trips byte for byte; edits elsewhere keep the stored breaks. |
| Limits | `composer_bytes` counts committed bytes, and a preedit is checked against it as if committed. `display_text` can be longer, but never more than 3 bytes per stored byte: a 1-byte control becomes a 3-byte marker, a marker of a 2- or 3-byte stored character is at most 3 bytes, and a 3-byte guard is only added beside a neighbour of at least 2 bytes, which adds at most one guard. So `display_text.len() <= 3 * composer_bytes`, preedit included. A renderer whose text limit is below that fails such a frame with a clean error and keeps the field unchanged. The renderer lays out at most 65,536 bytes of one text, whatever its configured limits, so 21,845 is the ceiling for `composer_bytes` in a single field that may hold stored controls. |
| Visual caret movement | `twui_move_visual` and the native example take one renderer layout step on `display_text` and map it back, through `tilewright-ui-render`'s `TextGeometry::step_caret`, which returns the new focus in committed text. A guard gives one committed position two display positions. A step that moves the display byte but not the committed byte only crossed the guard and is repeated, so crossing a guard no longer stalls the caret (it did in right-to-left text). The stepping rule (since 0.1.1): a layout step that changes only the caret's affinity is a stop if and only if the two caret rectangles differ by more than 1/64 logical pixel in x or y; otherwise it is repeated, like a guard crossing, within at most four layout steps per call. So a move of the affinity alone to a visibly different place, a direction boundary, is a real stop and counts as one step, while two caret stops that the layout places a rounding error apart (in 0.1.0 a press next to a marker, or in plain right-to-left text, at 20 px could do nothing) are one. Since 0.1.3 a layout stop at the inner side of a guard, which maps to the committed position at the marker unit's edge, is not a stop when that edge is drawn at another place than the stop and no further than where the press began in the pressed direction (more than 1/64 logical pixel beyond it is further), which happens in mixed-direction text; the step is repeated. Before, such a stop moved the caret against the press, or left it in place, and could make presses alternate or cycle (for example a right-to-left letter, a Latin letter, then a prepended character before a stored break). A stop whose edge lies further the pressed way is kept, as before. A step can still land on another committed position drawn at the same place, and a press can still pass over a place drawn between two stops in mixed-direction text; Home, End and the pointer still work. |

`mark_stored_breaks` applies the same display to any stored string, for
example a read-only row. A host that describes a stored value ("contains 2
line breaks") counts its markers' glyphs. A browser presenter that edits the
whole value through `ReplaceText` cannot keep a stored break, since that
path is new input; the browser example binds no field.

Known gap, unreachable today: the web adapter's snapshot reports `anchor` and
`focus` as UTF-16 offsets into `display_text`, while the DOM value and paste
use the committed text. They differ once a CR LF pair (two units, one
marker) or a guard (one extra unit) exists; a lone LF, U+2028, U+2029, tab or
NUL is one unit in both. The web adapter must not bind a
field that may hold stored breaks until it maps display offsets to committed
ones, and its DOM must never hold `displayText`, or the marker glyphs would
silently replace the stored breaks.

## Required regression cases

- Fill the action queue; submit with Enter and button; verify unchanged composer/revision, repeat peek, one acknowledgment, and exactly one accepted retry.
- With the queue full, enter/exit a modal while a world key is held; sample only the final control snapshot and verify epoch-based cancellation still happens. Destroy focused/captured widgets without stuck controls.
- Inject malformed/oversized text and saturated history; verify atomic rejection or documented atomic retention. Test combining marks, emoji sequences, RTL text and composition cancellation.
- Fail and reorder presentation around scrolling, editing, resize and surface recreation. Click before acknowledgment; verify no invisible geometry receives input and retired stamps cannot revive it.
- Verify IME positioning uses the current acknowledged transform; exercise a pending DOM patch and a stale native text-layout snapshot.
- Saturate prepared-frame/output-buffer capacity and verify bounded retention without loss. Run C17 header/link and repeated lifecycle tests, plus the supported host-specific checks.
