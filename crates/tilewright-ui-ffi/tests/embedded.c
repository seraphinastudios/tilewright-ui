/* Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT */
/* Reuse the existing C boundary helpers and execute its regression suite too. */
#define main legacy_abi_main
#include "../../../tests/c/abi.c"
#undef main

static twui_layout attachment(void) {
    twui_layout out = {0};
    out.struct_size = sizeof out;
    out.viewport_width = 640; out.viewport_height = 480; out.viewport_scale = 1;
    out.bounds_x = 100; out.bounds_y = 80;
    out.bounds_width = 300; out.bounds_height = 120;
    out.clip_x = 100; out.clip_y = 80;
    out.clip_width = 300; out.clip_height = 120;
    return out;
}

static void embedded_field(void) {
    twui_config cfg = TWUI_CONFIG_INIT; cfg.queued_batches = 1;
    twui_context *ctx = NULL;
    assert(twui_create_with_composition(&cfg, 99, &ctx) == TWUI_INVALID_ARGUMENT);
    assert(ctx == NULL);
    assert(twui_create_with_composition(&cfg, TWUI_COMPOSITION_SINGLE_FIELD, &ctx) == TWUI_OK);
    twui_stamp frame = {0}; frame.struct_size = sizeof frame;
    assert(twui_prepare_current(ctx, &frame) == TWUI_STALE);
    twui_layout layout = attachment();
    assert(twui_set_layout(ctx, &layout) == TWUI_OK);
    twui_widget_ref widget = {0}; widget.struct_size = sizeof widget;
    assert(twui_bind_field(ctx, NULL, 0, &widget) == TWUI_OK);
    twui_state bound = state(ctx);
    assert(widget.widget_generation == bound.widget_generation);
    toggle(ctx, TWUI_SET_FOCUS, 1);
    twui_state before = state(ctx);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    unchanged(before, state(ctx)); /* successful field submission retains draft */
    twui_action action = {0}; action.struct_size = sizeof action;
    assert(twui_peek_action(ctx, &action) == TWUI_OK);
    assert(action.kind == TWUI_ACTION_SUBMIT_FIELD && action.text_bytes == 0);
    twui_field_action field = {0}; field.struct_size = sizeof field;
    assert(twui_get_field_action(ctx, action.batch_id + 1, &field) == TWUI_STALE);
    assert(twui_get_field_action(ctx, action.batch_id, &field) == TWUI_OK);
    assert(field.widget_id == widget.widget_id);
    assert(field.widget_generation == widget.widget_generation);
    assert(field.text_revision == before.text_revision && field.text_bytes == 0);
    uint64_t required = 99;
    assert(twui_copy_action(ctx, action.batch_id, NULL, 0, &required) == TWUI_OK);
    assert(required == 0);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_QUEUE_FULL);
    unchanged(before, state(ctx));

    twui_input old = input(ctx, TWUI_COMMIT);
    old.text = (const uint8_t *)"late"; old.text_len = 4;
    twui_input key = input(ctx, TWUI_KEY_DOWN);
    key.value = TWUI_KEY_OTHER; key.physical_key = 30;
    assert(apply(ctx, key) == TWUI_OK);
    twui_input preedit = input(ctx, TWUI_PREEDIT);
    preedit.text = (const uint8_t *)"x"; preedit.text_len = 1;
    assert(apply(ctx, preedit) == TWUI_OK);
    assert(twui_bind_field(ctx, (const uint8_t *)"new", 3, &widget) == TWUI_OK);
    twui_state rebound = state(ctx);
    assert(rebound.widget_generation > bound.widget_generation);
    assert(rebound.text_revision > before.text_revision);
    assert(rebound.control_epoch > before.control_epoch);
    assert(rebound.focused == 0 && rebound.preedit_bytes == 0);
    expect_text(ctx, "new");
    assert(twui_get_field_action(ctx, action.batch_id, &field) == TWUI_OK);
    assert(field.widget_generation == bound.widget_generation); /* old queued identity retained */
    toggle(ctx, TWUI_SET_FOCUS, 1);
    assert(apply(ctx, old) == TWUI_STALE);
    uint32_t allowed = 99;
    assert(twui_key_effect_allowed(ctx, 30, &allowed) == TWUI_OK && allowed == 0);
    key.kind = TWUI_KEY_UP;
    uint32_t consumed = 99;
    assert(twui_apply(ctx, &key, &consumed) == TWUI_OK && consumed == 1);
    assert(twui_ack_action(ctx, action.batch_id) == TWUI_OK);
    before = state(ctx);
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    expect_text(ctx, "new");
    assert(twui_peek_action(ctx, &action) == TWUI_OK);
    assert(twui_get_field_action(ctx, action.batch_id, &field) == TWUI_OK);
    assert(field.widget_generation == widget.widget_generation && field.text_revision == before.text_revision);
    uint8_t copy[3] = {0};
    assert(twui_copy_action(ctx, action.batch_id, copy, 2, &required) == TWUI_BUFFER_TOO_SMALL);
    assert(copy[0] == 0 && required == 3);
    assert(twui_copy_action(ctx, action.batch_id, copy, 3, &required) == TWUI_OK);
    assert(memcmp(copy, "new", 3) == 0);

    before = state(ctx);
    widget.struct_size = sizeof(uint32_t);
    assert(twui_bind_field(ctx, NULL, 0, &widget) == TWUI_ABI_MISMATCH);
    widget.struct_size = sizeof widget;
    assert(twui_bind_field(ctx, NULL, 1, &widget) == TWUI_INVALID_ARGUMENT);
    assert(twui_bind_field(ctx, (const uint8_t *)"\xff", 1, &widget) == TWUI_INVALID_ARGUMENT);
    assert(twui_bind_field(ctx, (const uint8_t *)"a\nb", 3, &widget) == TWUI_INVALID_ARGUMENT);
    unchanged(before, state(ctx));
    assert(twui_prepare_current(ctx, &frame) == TWUI_OK);
    assert(twui_ack_presented(ctx, &frame) == TWUI_OK);
    twui_layout bad = layout; bad.struct_size = sizeof(uint32_t);
    assert(twui_set_layout(ctx, &bad) == TWUI_ABI_MISMATCH);
    bad = layout; bad.reserved = 1;
    assert(twui_set_layout(ctx, &bad) == TWUI_INVALID_ARGUMENT);
    bad = layout; bad.clip_width = NAN;
    assert(twui_set_layout(ctx, &bad) == TWUI_INVALID_ARGUMENT);
    bad = layout; bad.bounds_width = -1;
    assert(twui_set_layout(ctx, &bad) == TWUI_INVALID_ARGUMENT);
    bad = layout; bad.viewport_scale = 0;
    assert(twui_set_layout(ctx, &bad) == TWUI_INVALID_ARGUMENT);
    assert(twui_pointer(ctx, &frame, 1, 1, &consumed) == TWUI_OK && consumed == 1);
    unchanged(before, state(ctx));
    assert(twui_set_layout(ctx, &layout) == TWUI_OK); /* same attachment keeps displayed */
    assert(twui_pointer(ctx, &frame, 639, 479, &consumed) == TWUI_OK && consumed == 1);
    layout.bounds_x += 10; layout.clip_x += 10;
    assert(twui_set_layout(ctx, &layout) == TWUI_OK);
    assert(twui_pointer(ctx, &frame, 120, 90, &consumed) == TWUI_STALE);
    assert(twui_ack_presented(ctx, &frame) == TWUI_STALE);
    twui_stamp current = {0}; current.struct_size = sizeof current;
    assert(twui_get_displayed(ctx, &current) == TWUI_EMPTY);
    assert(twui_destroy(ctx) == TWUI_OK);
}

static void embedded_chat(void) {
    twui_config cfg = TWUI_CONFIG_INIT;
    twui_context *ctx = NULL;
    assert(twui_create_with_composition(&cfg, TWUI_COMPOSITION_CHAT_PANEL, &ctx) == TWUI_OK);
    twui_layout layout = attachment();
    assert(twui_set_layout(ctx, &layout) == TWUI_OK);
    toggle(ctx, TWUI_SET_FOCUS, 1); commit(ctx, "chat");
    assert(apply(ctx, input(ctx, TWUI_SUBMIT)) == TWUI_OK);
    expect_text(ctx, "");
    twui_action action = {0}; action.struct_size = sizeof action;
    assert(twui_peek_action(ctx, &action) == TWUI_OK && action.kind == TWUI_ACTION_SUBMIT_CHAT);
    twui_field_action field = {0}; field.struct_size = sizeof field;
    assert(twui_get_field_action(ctx, action.batch_id, &field) == TWUI_INVALID_ARGUMENT);
    twui_widget_ref widget = {0}; widget.struct_size = sizeof widget;
    twui_state before = state(ctx);
    assert(twui_bind_field(ctx, NULL, 0, &widget) == TWUI_INVALID_ARGUMENT);
    unchanged(before, state(ctx));
    toggle(ctx, TWUI_SET_FOCUS, 0);
    twui_stamp frame = {0}; frame.struct_size = sizeof frame;
    assert(twui_prepare_current(ctx, &frame) == TWUI_OK);
    assert(twui_ack_presented(ctx, &frame) == TWUI_OK);
    uint32_t consumed = 99;
    assert(twui_pointer(ctx, &frame, 1, 1, &consumed) == TWUI_OK && consumed == 1);
    assert(twui_scroll_at(ctx, &frame, 639, 479, 1, &consumed) == TWUI_OK && consumed == 1);
    assert(twui_destroy(ctx) == TWUI_OK);
}

int main(void) {
    assert(legacy_abi_main() == 0);
    embedded_field(); embedded_chat();
    puts("embedded C ABI passed");
    return 0;
}
