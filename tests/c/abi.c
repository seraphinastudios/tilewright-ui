/* Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT */
#include "tilewright_ui.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <threads.h>

static twui_state state(twui_context *ctx) {
    twui_state out = {0}; out.struct_size = sizeof out;
    assert(twui_get_state(ctx, &out) == TWUI_OK); return out;
}
static twui_control control(twui_context *ctx) {
    twui_control out = {0}; out.struct_size = sizeof out;
    assert(twui_get_control(ctx, &out) == TWUI_OK); return out;
}
static twui_input input(twui_context *ctx, uint32_t kind) {
    twui_input in = {0}; in.struct_size = sizeof in; in.kind = kind;
    if (kind >= TWUI_COMMIT && kind <= TWUI_DELETE) {
        twui_state s = state(ctx);
        in.widget_generation = s.widget_generation;
        in.control_epoch = s.control_epoch;
        in.text_revision = s.text_revision;
    }
    return in;
}
static twui_status apply(twui_context *ctx, twui_input in) {
    uint32_t consumed = 99;
    return twui_apply(ctx, &in, &consumed);
}
static void toggle(twui_context *ctx, uint32_t kind, uint32_t value) {
    twui_input in = input(ctx, kind); in.value = value;
    assert(apply(ctx, in) == TWUI_OK);
}
static void commit(twui_context *ctx, const char *text) {
    twui_input in = input(ctx, TWUI_COMMIT);
    in.text = (const uint8_t *)text; in.text_len = strlen(text);
    assert(apply(ctx, in) == TWUI_OK);
}
static void unchanged(twui_state a, twui_state b) {
    assert(a.ui_revision == b.ui_revision && a.text_revision == b.text_revision);
    assert(a.anchor == b.anchor && a.caret == b.caret);
    assert(a.composer_bytes == b.composer_bytes && a.preedit_bytes == b.preedit_bytes);
    assert(a.control_epoch == b.control_epoch);
}
static void expect_text(twui_context *ctx, const char *text) {
    uint8_t bytes[128]; uint64_t required = 999;
    memset(bytes, 0x5a, sizeof bytes);
    assert(twui_copy_text(ctx, TWUI_TEXT_COMPOSER, 0, bytes, sizeof bytes, &required) == TWUI_OK);
    assert(required == strlen(text) && memcmp(bytes, text, (size_t)required) == 0);
    assert(bytes[required] == 0x5a); /* no implicit NUL write */
}
static int wrong_thread(void *opaque) {
    twui_context *ctx = opaque;
    twui_state out = {0}; out.struct_size = sizeof out;
    assert(twui_get_state(ctx, &out) == TWUI_WRONG_THREAD);
    assert(twui_destroy(ctx) == TWUI_WRONG_THREAD);
    return 0;
}
int main(void) {
    assert(twui_abi_version() == TWUI_ABI_VERSION);
    assert(twui_destroy(NULL) == TWUI_INVALID_ARGUMENT);
    twui_config cfg = TWUI_CONFIG_INIT;
    twui_context *ctx = NULL;
    assert(twui_create(NULL, &ctx) == TWUI_INVALID_ARGUMENT && ctx == NULL);
    cfg.abi_version++;
    assert(twui_create(&cfg, &ctx) == TWUI_ABI_MISMATCH && ctx == NULL);
    cfg.abi_version = TWUI_ABI_VERSION;
    cfg.struct_size = 4;
    assert(twui_create(&cfg, &ctx) == TWUI_ABI_MISMATCH);
    cfg.struct_size = sizeof cfg;
    cfg.queued_batches = 1; cfg.prepared_frames = 2;
    assert(twui_create(&cfg, &ctx) == TWUI_OK && ctx != NULL);
    thrd_t other;
    assert(thrd_create(&other, wrong_thread, ctx) == thrd_success);
    assert(thrd_join(other, NULL) == thrd_success);
    toggle(ctx, TWUI_SET_FOCUS, 1);
    commit(ctx, "e\xcc\x81 \xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x92\xbb");
    expect_text(ctx, "e\xcc\x81 \xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x92\xbb");
    twui_state before = state(ctx);
    twui_input bad = input(ctx, TWUI_SELECT); bad.anchor = 1; bad.caret = 1;
    assert(apply(ctx, bad) == TWUI_INVALID_ARGUMENT); unchanged(before, state(ctx));
    bad = input(ctx, TWUI_COMMIT); const uint8_t invalid[] = {0xc0, 0x80};
    bad.text = invalid; bad.text_len = sizeof invalid;
    assert(apply(ctx, bad) == TWUI_INVALID_ARGUMENT); unchanged(before, state(ctx));
    bad.text = NULL; bad.text_len = UINT64_MAX;
    assert(apply(ctx, bad) == TWUI_LIMIT_EXCEEDED); unchanged(before, state(ctx));
    bad = input(ctx, TWUI_COMMIT); bad.text_revision++;
    assert(apply(ctx, bad) == TWUI_STALE); unchanged(before, state(ctx));
    bad = input(ctx, TWUI_SET_FOCUS); bad.value = 2;
    assert(apply(ctx, bad) == TWUI_INVALID_ARGUMENT); unchanged(before, state(ctx));
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    commit(ctx, "second"); before = state(ctx);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_QUEUE_FULL); unchanged(before, state(ctx));
    twui_input enter = input(ctx, TWUI_KEY_DOWN); enter.value = TWUI_KEY_ENTER;
    assert(apply(ctx, enter) == TWUI_QUEUE_FULL); unchanged(before, state(ctx));
    twui_action a = {0}; a.struct_size = sizeof a;
    assert(twui_peek_action(ctx, &a) == TWUI_OK && a.kind == 1);
    twui_action repeated = {0}; repeated.struct_size = sizeof repeated;
    assert(twui_peek_action(ctx, &repeated) == TWUI_OK && repeated.batch_id == a.batch_id);
    uint8_t buffer[64]; memset(buffer, 0x5a, sizeof buffer); uint64_t need = 0;
    assert(twui_copy_action(ctx, a.batch_id, NULL, 0, &need) == TWUI_BUFFER_TOO_SMALL && need == a.text_bytes);
    assert(twui_copy_action(ctx, a.batch_id, buffer, need - 1, &need) == TWUI_BUFFER_TOO_SMALL && buffer[0] == 0x5a);
    assert(twui_copy_action(ctx, a.batch_id, buffer, sizeof buffer, &need) == TWUI_OK);
    assert(twui_ack_action(ctx, a.batch_id + 1) == TWUI_STALE);
    toggle(ctx, TWUI_SET_FOCUS, 0);
    uint64_t epoch = control(ctx).epoch;
    toggle(ctx, TWUI_SET_MODAL, 1); toggle(ctx, TWUI_SET_MODAL, 0);
    assert(control(ctx).epoch > epoch && control(ctx).world_input_allowed == 1);
    assert(twui_peek_action(ctx, &repeated) == TWUI_OK && repeated.batch_id == a.batch_id);
    assert(twui_ack_action(ctx, a.batch_id) == TWUI_OK);
    assert(twui_ack_action(ctx, a.batch_id) == TWUI_OK);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    assert(twui_peek_action(ctx, &repeated) == TWUI_OK && repeated.batch_id > a.batch_id);
    assert(twui_ack_action(ctx, repeated.batch_id) == TWUI_OK);
    assert(twui_peek_action(ctx, &repeated) == TWUI_EMPTY);
    toggle(ctx, TWUI_SET_FOCUS, 1);
    twui_input preedit = input(ctx, TWUI_PREEDIT); preedit.text = (const uint8_t *)"\xe6\xbc\xa2"; preedit.text_len = 3; preedit.caret = 1;
    assert(apply(ctx, preedit) == TWUI_OK && state(ctx).preedit_bytes == 3);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    assert(twui_peek_action(ctx, &repeated) == TWUI_EMPTY);
    assert(apply(ctx, input(ctx, TWUI_CANCEL_PREEDIT)) == TWUI_OK && state(ctx).preedit_bytes == 0);
    twui_viewport vp = {0}; vp.struct_size = sizeof vp; vp.width = 800; vp.height = 600; vp.scale = 1;
    twui_stamp first = {0}, second = {0}, extra = {0};
    first.struct_size = sizeof first; second.struct_size = sizeof second; extra.struct_size = sizeof extra;
    assert(twui_prepare(ctx, &vp, &first) == TWUI_OK);
    uint32_t consumed = 99;
    assert(twui_pointer(ctx, &first, 10, 10, &consumed) == TWUI_STALE);
    assert(twui_prepare(ctx, &vp, &second) == TWUI_OK);
    assert(twui_prepare(ctx, &vp, &extra) == TWUI_QUEUE_FULL);
    assert(twui_ack_presented(ctx, &second) == TWUI_OK);
    assert(twui_ack_presented(ctx, &first) == TWUI_STALE);
    assert(twui_ack_presented(ctx, &second) == TWUI_OK);
    assert(twui_discard_prepared(ctx, &first) == TWUI_OK);
    assert(twui_pointer(ctx, &second, NAN, 0, &consumed) == TWUI_INVALID_ARGUMENT);
    commit(ctx, "new revision");
    assert(twui_pointer(ctx, &second, 10, 10, &consumed) == TWUI_STALE);
    assert(twui_invalidate_surface(ctx) == TWUI_OK);
    assert(twui_get_displayed(ctx, &extra) == TWUI_EMPTY);
    assert(twui_ack_presented(ctx, &second) == TWUI_STALE);
    assert(twui_prepare(ctx, &vp, &extra) == TWUI_OK && extra.surface > second.surface);
    assert(twui_destroy(ctx) == TWUI_OK);
    for (int i = 0; i != 256; ++i) {
        ctx = NULL; assert(twui_create(&cfg, &ctx) == TWUI_OK);
        assert(twui_destroy(ctx) == TWUI_OK);
    }
    puts("C17 ABI: validation, UTF-8, atomic backpressure, control, stamps, thread and 256 lifecycle checks passed");
    return 0;
}
