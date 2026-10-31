/* Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT */
#ifndef TILEWRIGHT_UI_H
#define TILEWRIGHT_UI_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* Prototype ABI 1. Not a published ABI stability promise.
 * The host owns windows, event loops, rendering/presentation and world effects.
 * All context calls, including destroy, must run on the creating thread. Calls
 * may not overlap or reenter. Each pointer must reference valid, appropriately
 * aligned accessible storage for its documented extent; pointers and spans must
 * not alias other parameters or the context. Null/length checks cannot validate
 * arbitrary addresses. A destroyed handle must never be used or destroyed again.
 * All input text is copied during the call. No allocation or borrowed Rust view
 * escapes; the library alone frees its context. Buffers remain caller-owned.
 * No C callback runs inside this library. Internal unwinding poisons a context;
 * only destroy is accepted thereafter. Abort and OOM are not recoverable.
 * Initialize every POD struct_size to sizeof(the struct). Extra trailing bytes
 * are ignored. Reserved fields must be zero. Flags are exact 0/1 unless noted.
 */
#define TWUI_ABI_VERSION UINT32_C(1)
typedef struct twui_context twui_context;
typedef uint32_t twui_status;
#define TWUI_OK UINT32_C(0)
#define TWUI_INVALID_ARGUMENT UINT32_C(1)
#define TWUI_ABI_MISMATCH UINT32_C(2)
#define TWUI_LIMIT_EXCEEDED UINT32_C(3)
#define TWUI_QUEUE_FULL UINT32_C(4)
#define TWUI_STALE UINT32_C(5)
#define TWUI_EXHAUSTED UINT32_C(6)
#define TWUI_BUFFER_TOO_SMALL UINT32_C(7)
#define TWUI_EMPTY UINT32_C(8)
#define TWUI_WRONG_THREAD UINT32_C(9)
#define TWUI_REENTRANT UINT32_C(10)
#define TWUI_POISONED UINT32_C(11)
#define TWUI_INTERNAL_PANIC UINT32_C(12)
#define TWUI_RENDER_ERROR UINT32_C(13)

typedef struct twui_config {
    uint32_t struct_size, abi_version;
    uint32_t message_bytes, history_messages, history_bytes, composer_bytes;
    uint32_t queued_batches, prepared_frames;
} twui_config;
#define TWUI_CONFIG_INIT { sizeof(twui_config), TWUI_ABI_VERSION, 4096, 128, 65536, 4096, 16, 3 }

/* UTF-8 lengths are bytes, never NUL-terminated strings. Embedded NUL, newline
 * and other disallowed single-line controls are rejected by shared semantics.
 * COMMIT/PREEDIT/RECEIVE use text+text_len. SELECT uses UTF-8 anchor/caret bytes. PREEDIT uses anchor/caret as
 * scalar start/length (UINT64_MAX means SDL unknown -1). Text/edit/preedit calls carry the target generation,
 * control epoch and text revision obtained from state; stale targets fail.
 * SET_FOCUS, SET_MODAL, SET_SUSPENDED, SET_WINDOW_FOCUS use value (0 or 1).
 * KEY_DOWN/KEY_UP use value from TWUI_KEY_* and a stable physical_key in
 * 0..65535 (for example an SDL scancode). Different physical keys MUST keep
 * distinct identities even when both mean Enter. SCROLL uses signed delta.
 * Unused fields must be zero. A physical key is distinct from committed text.
 */
#define TWUI_COMMIT UINT32_C(1)
#define TWUI_PREEDIT UINT32_C(2)
#define TWUI_CANCEL_PREEDIT UINT32_C(3)
#define TWUI_SELECT UINT32_C(4)
#define TWUI_BACKSPACE UINT32_C(5)
#define TWUI_DELETE UINT32_C(6)
#define TWUI_SUBMIT UINT32_C(7)
#define TWUI_SET_FOCUS UINT32_C(8)
#define TWUI_SET_MODAL UINT32_C(9)
#define TWUI_SET_SUSPENDED UINT32_C(10)
#define TWUI_SET_WINDOW_FOCUS UINT32_C(11)
#define TWUI_RECEIVE UINT32_C(12)
#define TWUI_SCROLL UINT32_C(13)
#define TWUI_KEY_DOWN UINT32_C(14)
#define TWUI_KEY_UP UINT32_C(15)
#define TWUI_KEY_ENTER UINT32_C(1)
#define TWUI_KEY_BACKSPACE UINT32_C(2)
#define TWUI_KEY_DELETE UINT32_C(3)
#define TWUI_KEY_SELECT_ALL UINT32_C(4)
#define TWUI_KEY_ESCAPE UINT32_C(5)
#define TWUI_KEY_OTHER UINT32_C(6)
#define TWUI_KEY_WORLD TWUI_KEY_OTHER
typedef struct twui_input {
    uint32_t struct_size, kind, value, reserved;
    const uint8_t *text;
    uint64_t text_len, anchor, caret;
    uint64_t widget_generation, control_epoch, text_revision;
    int32_t delta;
    uint32_t key_repeat; /* 0/1, only KEY_DOWN; other kinds require zero */
    uint32_t physical_key; /* 0..65535 for key events; zero for other kinds */
} twui_input;

typedef struct twui_state {
    uint32_t struct_size, focused, modal_open, suspended;
    uint64_t ui_revision, text_revision, anchor, caret;
    uint32_t history_messages, scroll;
    uint64_t composer_bytes, preedit_bytes;
    uint64_t widget_generation, control_epoch;
} twui_state;
typedef struct twui_control {
    uint32_t struct_size, world_input_allowed, modal_open, suspended;
    uint64_t epoch, widget_id, widget_generation;
} twui_control;
typedef struct twui_action {
    uint32_t struct_size, kind; /* kind 1 = submit chat */
    uint64_t batch_id, text_bytes;
} twui_action;
typedef struct twui_stamp {
    uint32_t struct_size, reserved;
    uint64_t surface, frame, geometry, ui_revision;
} twui_stamp;
typedef struct twui_viewport {
    uint32_t struct_size, reserved;
    float width, height, scale;
    uint32_t reserved2;
} twui_viewport;

uint32_t twui_abi_version(void);
twui_status twui_create(const twui_config *config, twui_context **out_context);
twui_status twui_destroy(twui_context *context);
/* consumed is required; 1 means UI-owned, 0 means unhandled. Errors do not
 * permit world routing. Re-sample control after every input/lifecycle call. */
twui_status twui_apply(twui_context *context, const twui_input *input, uint32_t *consumed);
/* After a successful key apply, gate clipboard/navigation/host edit effects
 * with this query. Consumed alone also includes quarantined presses. */
twui_status twui_key_effect_allowed(twui_context *context, uint32_t physical_key, uint32_t *allowed);
twui_status twui_get_state(twui_context *context, twui_state *out_state);
twui_status twui_get_control(twui_context *context, twui_control *out_control);
/* Copy protocol: required reports exact bytes (no NUL). null buffer with zero
 * capacity queries size; returns BUFFER_TOO_SMALL for nonempty data. Short
 * buffers are not modified. Repeating copies never changes state or acknowledges
 * actions. No library-owned pointer is returned. */
#define TWUI_TEXT_COMPOSER UINT32_C(1)
#define TWUI_TEXT_PREEDIT UINT32_C(2)
#define TWUI_TEXT_HISTORY UINT32_C(3)
twui_status twui_copy_text(twui_context *context, uint32_t source, uint32_t index,
    uint8_t *buffer, uint64_t capacity, uint64_t *required);
twui_status twui_peek_action(twui_context *context, twui_action *out_action);
twui_status twui_copy_action(twui_context *context, uint64_t batch_id,
    uint8_t *buffer, uint64_t capacity, uint64_t *required);
/* Accept/deduplicate by context identity + batch_id before acknowledging. */
twui_status twui_ack_action(twui_context *context, uint64_t batch_id);
/* Preparing is not presenting. Acknowledge only after successful host present;
 * retain serialized presentation order. Invalidate on resize/transform/loss.
 * Geometric input must carry the exact acknowledged stamp. */
twui_status twui_prepare(twui_context *context, const twui_viewport *viewport, twui_stamp *out_stamp);
twui_status twui_ack_presented(twui_context *context, const twui_stamp *stamp);
/* Only discard once host confirms the frame will never be presented. */
twui_status twui_discard_prepared(twui_context *context, const twui_stamp *stamp);
twui_status twui_get_displayed(twui_context *context, twui_stamp *out_stamp);
twui_status twui_invalidate_surface(twui_context *context);
/* Only an explicit current WORLD hit can be unhandled; it clears text focus
 * and composition first. Re-sample the control epoch before world handoff.
 * Panel gaps and points outside the viewport are consumed. */
twui_status twui_pointer(twui_context *context, const twui_stamp *stamp, float x, float y, uint32_t *consumed);
/* Scroll only a positive shared HISTORY hit. Only explicit WORLD may be
 * unhandled; other UI regions/outside points consume without scrolling. */
twui_status twui_scroll_at(twui_context *context, const twui_stamp *stamp, float x, float y, int32_t delta, uint32_t *consumed);

#ifdef TWUI_CPU_PROTOTYPE
/* Available only when built with Cargo feature cpu-prototype. This is the CPU
 * reference renderer, not a GPU performance claim. Fonts come from the host's
 * explicitly verified manifest; no files/system fonts/network are consulted.
 * Font array elements must have exactly sizeof(twui_font) as struct_size.
 * Each font is <=32MiB, the entire pack <=64MiB, count 1..32, family <=128 bytes.
 * sha256 is the expected digest from that manifest. Install once per context.
 */
typedef struct twui_font {
    uint32_t struct_size, reserved;
    const uint8_t *family;
    uint64_t family_bytes;
    const uint8_t *bytes;
    uint64_t byte_length;
    uint8_t sha256[32];
} twui_font;
typedef struct twui_image {
    uint32_t struct_size, width, height, stride;
    uint64_t byte_length;
    twui_stamp stamp;
} twui_image;
typedef struct twui_rect {
    uint32_t struct_size, reserved;
    float x, y, width, height;
} twui_rect;
twui_status twui_set_fonts(twui_context *context, const twui_font *fonts, uint32_t count);
/* Physical size = logical viewport size * scale, integral and <=4096 each,
 * at most 4 million pixels. Output is tightly packed premultiplied RGBA8. Prepare
 * retains pixels and exact shaped geometry until discarded or replaced after
 * acknowledgment. Copy does not acknowledge. Alpha integration is prototype.
 */
twui_status twui_render_prepare(twui_context *context, const twui_viewport *viewport, twui_image *out_image);
twui_status twui_copy_rgba(twui_context *context, const twui_stamp *stamp,
    uint8_t *buffer, uint64_t capacity, uint64_t *required);
/* Returns EMPTY if no visible caret; requires exact currently displayed/live
 * stamp. Rectangle is in logical coordinates; host applies its screen transform.
 */
twui_status twui_get_ime_rect(twui_context *context, const twui_stamp *stamp, twui_rect *out_rect);
/* Uses the exact displayed shaped layout. motion: 1 left, 2 right, 3 home,
 * 4 end. extend is 0/1. Apply the physical key successfully first; this call
 * requires its active UI-owned, nonquarantined press. Composition must be
 * committed/cancelled first. An ownership failure returns STALE. */
twui_status twui_move_visual(twui_context *context, const twui_stamp *stamp, uint32_t physical_key, uint32_t motion, uint32_t extend);
#endif

#ifdef __cplusplus
}
#endif
#endif
