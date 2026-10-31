/* Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT */
/* Caller-owned SDL3 host. UI semantics, glyphs and retained frames are Rust. */
#include <SDL3/SDL.h>
#define TWUI_CPU_PROTOTYPE
#include "tilewright_ui.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>

#define FONT_LIMIT (32U * 1024U * 1024U)
#define MAX_FONTS 32U
struct host {
    twui_context *ui;
    SDL_Window *window;
    uint64_t epoch, session_start;
    bool text_active, consumed;
    bool held[SDL_SCANCODE_COUNT];
    uint32_t pressed[SDL_SCANCODE_COUNT];
};
static void check(twui_status status, const char *operation) {
    if (status != TWUI_OK) { fprintf(stderr, "%s: status %" PRIu32 "\n", operation, status); exit(EXIT_FAILURE); }
}
static void sdl_check(bool ok, const char *operation) {
    if (!ok) { fprintf(stderr, "%s: %s\n", operation, SDL_GetError()); exit(EXIT_FAILURE); }
}
static twui_state state(struct host *host) {
    twui_state result = { .struct_size = sizeof(result) };
    check(twui_get_state(host->ui, &result), "state"); return result;
}
static void sync_control(struct host *host) {
    twui_control control = { .struct_size = sizeof(control) };
    check(twui_get_control(host->ui, &control), "control");
    if (host->epoch != control.epoch) {
        host->epoch = control.epoch;
        memset(host->held, 0, sizeof(host->held));
        host->session_start = SDL_GetTicksNS();
        sdl_check(SDL_StopTextInput(host->window), "stop text");
        host->text_active = state(host).focused != 0;
        if (host->text_active) sdl_check(SDL_StartTextInput(host->window), "start text");
    }
}
static twui_status apply(struct host *host, twui_input input) {
    uint32_t consumed = 1;
    input.struct_size = sizeof(input);
    twui_status result = twui_apply(host->ui, &input, &consumed);
    host->consumed = consumed != 0 || result != TWUI_OK;
    sync_control(host);
    if (result != TWUI_OK) fprintf(stderr, "input rejected: %" PRIu32 "\n", result);
    return result;
}
static twui_input text_input(struct host *host, uint32_t kind, const char *text) {
    twui_state current = state(host);
    twui_input result = { .kind = kind, .text = (const uint8_t *)text, .text_len = strlen(text),
        .widget_generation = current.widget_generation, .control_epoch = current.control_epoch,
        .text_revision = current.text_revision };
    if (kind == TWUI_RECEIVE) { result.widget_generation = 0; result.control_epoch = 0; result.text_revision = 0; }
    return result;
}
static uint32_t key_value(SDL_Keycode key, SDL_Keymod modifiers, SDL_Scancode physical) {
    if (key == SDLK_RETURN || key == SDLK_KP_ENTER) return TWUI_KEY_ENTER;
    if (key == SDLK_BACKSPACE) return TWUI_KEY_BACKSPACE;
    if (key == SDLK_DELETE) return TWUI_KEY_DELETE;
    if (key == SDLK_ESCAPE) return TWUI_KEY_ESCAPE;
    if (key == SDLK_A && (modifiers & SDL_KMOD_CTRL)) return TWUI_KEY_SELECT_ALL;
    return UINT32_C(0x10000) + (uint32_t)physical;
}
static void clipboard(struct host *host, const SDL_KeyboardEvent *event) {
    twui_state current = state(host);
    if (event->repeat || !current.focused || !(event->mod & SDL_KMOD_CTRL)) return;
    if (event->key == SDLK_V) {
        char *text = SDL_GetClipboardText();
        if (text) { (void)apply(host, text_input(host, TWUI_COMMIT, text)); SDL_free(text); }
    } else if ((event->key == SDLK_C || event->key == SDLK_X) && current.preedit_bytes == 0) {
        uint8_t text[4097]; uint64_t required = 0;
        check(twui_copy_text(host->ui, TWUI_TEXT_COMPOSER, 0, text, sizeof(text) - 1, &required), "copy composer");
        uint64_t start = current.anchor < current.caret ? current.anchor : current.caret;
        uint64_t end = current.anchor > current.caret ? current.anchor : current.caret;
        if (end > required || start > end) { fputs("invalid shared selection\n", stderr); exit(EXIT_FAILURE); }
        if (start != end) {
            text[end] = 0; sdl_check(SDL_SetClipboardText((const char *)text + start), "clipboard");
            if (event->key == SDLK_X) (void)apply(host, text_input(host, TWUI_COMMIT, ""));
        }
    }
}
static bool event(struct host *host, SDL_Event *event) {
    SDL_WindowID own = SDL_GetWindowID(host->window);
    if (event->type == SDL_EVENT_QUIT) return false;
    if (event->type >= SDL_EVENT_WINDOW_FIRST && event->type <= SDL_EVENT_WINDOW_LAST && event->window.windowID != own) return true;
    switch (event->type) {
    case SDL_EVENT_WINDOW_CLOSE_REQUESTED: return false;
    case SDL_EVENT_WINDOW_FOCUS_LOST: (void)apply(host, (twui_input){ .kind = TWUI_SET_WINDOW_FOCUS, .value = 0 }); break;
    case SDL_EVENT_WINDOW_FOCUS_GAINED: (void)apply(host, (twui_input){ .kind = TWUI_SET_WINDOW_FOCUS, .value = 1 }); break;
    case SDL_EVENT_WINDOW_MINIMIZED:
        (void)apply(host, (twui_input){ .kind = TWUI_SET_SUSPENDED, .value = 1 });
        check(twui_invalidate_surface(host->ui), "invalidate minimized"); break;
    case SDL_EVENT_WINDOW_RESTORED:
        (void)apply(host, (twui_input){ .kind = TWUI_SET_SUSPENDED, .value = 0 });
        check(twui_invalidate_surface(host->ui), "invalidate restored"); break;
    case SDL_EVENT_WINDOW_RESIZED: case SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED:
        check(twui_invalidate_surface(host->ui), "invalidate resize"); break;
    case SDL_EVENT_TEXT_INPUT:
        if (event->text.windowID == own && host->text_active && event->text.timestamp >= host->session_start)
            (void)apply(host, text_input(host, TWUI_COMMIT, event->text.text));
        break;
    case SDL_EVENT_TEXT_EDITING:
        if (event->edit.windowID == own && host->text_active && event->edit.timestamp >= host->session_start) {
            twui_input input = text_input(host, TWUI_PREEDIT, event->edit.text);
            /* SDL counts scalar characters; the shared core converts to bytes. */
            input.anchor = event->edit.start < 0 ? UINT64_MAX : (uint64_t)event->edit.start;
            input.caret = event->edit.length < 0 ? UINT64_MAX : (uint64_t)event->edit.length;
            (void)apply(host, input);
        } break;
    case SDL_EVENT_KEY_DOWN: case SDL_EVENT_KEY_UP:
        if (event->key.windowID == own && event->key.scancode > SDL_SCANCODE_UNKNOWN && event->key.scancode < SDL_SCANCODE_COUNT) {
            size_t physical = (size_t)event->key.scancode;
            bool down = event->type == SDL_EVENT_KEY_DOWN;
            if (down && host->pressed[physical] == 0) host->pressed[physical] = key_value(event->key.key, event->key.mod, event->key.scancode);
            uint32_t value = host->pressed[physical] ? host->pressed[physical] : key_value(event->key.key, event->key.mod, event->key.scancode);
            if (down && state(host).focused) {
                uint32_t motion = event->key.key == SDLK_LEFT ? 1U : event->key.key == SDLK_RIGHT ? 2U : event->key.key == SDLK_HOME ? 3U : event->key.key == SDLK_END ? 4U : 0U;
                twui_stamp stamp = { .struct_size = sizeof(stamp) };
                if (motion && twui_get_displayed(host->ui, &stamp) == TWUI_OK) {
                    twui_status moved = twui_move_visual(host->ui, &stamp, motion, (event->key.mod & SDL_KMOD_SHIFT) ? 1U : 0U);
                    if (moved != TWUI_OK && moved != TWUI_STALE) fprintf(stderr, "cursor rejected: %" PRIu32 "\n", moved);
                }
            }
            if (down) clipboard(host, &event->key);
            (void)apply(host, (twui_input){ .kind = down ? TWUI_KEY_DOWN : TWUI_KEY_UP, .value = value, .key_repeat = down && event->key.repeat });
            bool owned = host->consumed;
            if (down && !event->key.repeat) {
                if (event->key.key == SDLK_TAB && !state(host).modal_open)
                    (void)apply(host, (twui_input){ .kind = TWUI_SET_FOCUS, .value = !state(host).focused });
                if (event->key.key == SDLK_F1)
                    (void)apply(host, (twui_input){ .kind = TWUI_SET_MODAL, .value = !state(host).modal_open });
            }
            twui_control control = { .struct_size = sizeof(control) };
            check(twui_get_control(host->ui, &control), "key control");
            host->held[physical] = down && !owned && control.world_input_allowed;
            if (!down) host->pressed[physical] = 0;
        } break;
    case SDL_EVENT_MOUSE_BUTTON_DOWN:
        if (event->button.windowID == own && event->button.button == SDL_BUTTON_LEFT) {
            twui_stamp stamp = { .struct_size = sizeof(stamp) }; uint32_t consumed = 1;
            if (twui_get_displayed(host->ui, &stamp) == TWUI_OK) {
                twui_status status = twui_pointer(host->ui, &stamp, event->button.x, event->button.y, &consumed);
                if (status != TWUI_OK && status != TWUI_STALE) check(status, "pointer");
                sync_control(host);
            }
        } break;
    case SDL_EVENT_MOUSE_WHEEL:
        if (event->wheel.windowID == own) {
            twui_stamp stamp = { .struct_size = sizeof(stamp) }; uint32_t consumed = 1;
            if (twui_get_displayed(host->ui, &stamp) == TWUI_OK) {
                twui_status status = twui_scroll_at(host->ui, &stamp, event->wheel.mouse_x, event->wheel.mouse_y, (int32_t)event->wheel.y, &consumed);
                if (status != TWUI_OK && status != TWUI_STALE) check(status, "scroll");
                sync_control(host);
            }
        }
        break;
    default: break;
    }
    return true;
}
static void accept_actions(struct host *host) {
    twui_action action = { .struct_size = sizeof(action) };
    while (twui_peek_action(host->ui, &action) == TWUI_OK) {
        uint8_t text[4097]; uint64_t size = 0;
        check(twui_copy_action(host->ui, action.batch_id, text, sizeof(text) - 1, &size), "copy action");
        text[size] = 0;
        /* Local echo is the host effect; acknowledge only after acceptance. */
        check(apply(host, text_input(host, TWUI_RECEIVE, (const char *)text)), "echo");
        check(twui_ack_action(host->ui, action.batch_id), "ack action");
    }
}
static uint8_t nibble(char c) {
    if (c >= '0' && c <= '9') return (uint8_t)(c - '0');
    if (c >= 'a' && c <= 'f') return (uint8_t)(c - 'a' + 10);
    if (c >= 'A' && c <= 'F') return (uint8_t)(c - 'A' + 10);
    fputs("invalid SHA256\n", stderr); exit(EXIT_FAILURE);
}
static void configure_ime(void) {
    /* Composition rendering is provided here; candidate UI stays OS-owned. */
    sdl_check(SDL_SetHintWithPriority(SDL_HINT_IME_IMPLEMENTED_UI, "composition", SDL_HINT_OVERRIDE), "IME capability");
    const char *hint = SDL_GetHint(SDL_HINT_IME_IMPLEMENTED_UI);
    if (!hint || strcmp(hint, "composition") != 0) { fputs("IME capability was not retained\n", stderr); exit(EXIT_FAILURE); }
}
int main(int argc, char **argv) {
    configure_ime();
    if (argc == 2 && strcmp(argv[1], "--check-ime") == 0) { puts("ime_ui=composition"); return EXIT_SUCCESS; }
    twui_font fonts[MAX_FONTS] = {{0}}; uint8_t *font_bytes[MAX_FONTS] = {0}; uint32_t font_count = 0; uint64_t total_font_bytes = 0;
    uint64_t max_frames = 0;
    for (int arg = 1; arg < argc; ++arg) {
        if (strcmp(argv[arg], "--font") == 0 && arg + 3 < argc && font_count < MAX_FONTS) {
            const char *family = argv[++arg], *path = argv[++arg], *hex = argv[++arg];
            if (strlen(hex) != 64) { fputs("SHA256 must contain 64 hex digits\n", stderr); return EXIT_FAILURE; }
            FILE *file = fopen(path, "rb"); if (!file) { perror(path); return EXIT_FAILURE; }
            if (fseek(file, 0, SEEK_END) != 0) return EXIT_FAILURE;
            long length = ftell(file); if (length <= 0 || (unsigned long)length > FONT_LIMIT) return EXIT_FAILURE;
            total_font_bytes += (uint64_t)length; if (total_font_bytes > 64U * 1024U * 1024U) return EXIT_FAILURE;
            rewind(file); uint8_t *bytes = malloc((size_t)length); if (!bytes) return EXIT_FAILURE;
            if (fread(bytes, 1, (size_t)length, file) != (size_t)length || fclose(file) != 0) return EXIT_FAILURE;
            twui_font *font = &fonts[font_count];
            font->struct_size = sizeof(*font); font->family = (const uint8_t *)family; font->family_bytes = strlen(family);
            font->bytes = bytes; font->byte_length = (uint64_t)length;
            for (size_t index = 0; index < 32; ++index) font->sha256[index] = (uint8_t)((nibble(hex[index * 2]) << 4) | nibble(hex[index * 2 + 1]));
            font_bytes[font_count++] = bytes;
        } else if (strcmp(argv[arg], "--frames") == 0 && arg + 1 < argc) {
            char *end = NULL; max_frames = strtoull(argv[++arg], &end, 10); if (!end || *end != 0 || max_frames == 0) return EXIT_FAILURE;
        } else { fputs("usage: tilewright-ui-c-sdl --font FAMILY PATH SHA256 [--font FAMILY PATH SHA256] [--frames N]\n", stderr); return EXIT_FAILURE; }
    }
    if (font_count == 0) { fputs("explicit verified fonts required\n", stderr); return EXIT_FAILURE; }
    sdl_check(SDL_Init(SDL_INIT_VIDEO), "init");
    SDL_Window *window = SDL_CreateWindow("Tilewright shared UI — CPU reference / C SDL3", 800, 600, SDL_WINDOW_RESIZABLE);
    if (!window) { fprintf(stderr, "%s\n", SDL_GetError()); return EXIT_FAILURE; }
    SDL_Renderer *renderer = SDL_CreateRenderer(window, NULL); if (!renderer) return EXIT_FAILURE;
    twui_config config = TWUI_CONFIG_INIT; struct host host = { .window = window };
    check(twui_create(&config, &host.ui), "create"); check(twui_set_fonts(host.ui, fonts, font_count), "set fonts");
    for (uint32_t index = 0; index < font_count; ++index) free(font_bytes[index]);
    (void)apply(&host, text_input(&host, TWUI_RECEIVE, "Shared UI / caller-owned C17 SDL3"));
    (void)apply(&host, text_input(&host, TWUI_RECEIVE, "Tab: chat focus · F1: modal · Ctrl+A/C/V: clipboard"));
    (void)apply(&host, text_input(&host, TWUI_RECEIVE, "Neutral procedural map. CPU reference rendering."));
    (void)apply(&host, (twui_input){ .kind = TWUI_SET_FOCUS, .value = 1 });
    bool running = true; uint64_t frames = 0;
    while (running) {
        SDL_Event current; while (SDL_PollEvent(&current)) if (!event(&host, &current)) running = false;
        if (!running) break;
        accept_actions(&host);
        int width = 0, height = 0; sdl_check(SDL_GetWindowSize(window, &width, &height), "size");
        if (width <= 0 || height <= 0) { SDL_Delay(20); continue; }
        if (width > 1600 || height > 1200) { fputs("reference viewport exceeds 1600x1200\n", stderr); return EXIT_FAILURE; }
        twui_viewport viewport = { .struct_size = sizeof(viewport), .width = (float)width, .height = (float)height, .scale = 1 };
        twui_image image = { .struct_size = sizeof(image) };
        check(twui_render_prepare(host.ui, &viewport, &image), "render prepare");
        uint8_t *pixels = malloc((size_t)image.byte_length); if (!pixels) return EXIT_FAILURE;
        uint64_t required = 0; check(twui_copy_rgba(host.ui, &image.stamp, pixels, image.byte_length, &required), "copy RGBA");
        /* Premultiplied shared pixels over independently authored host map. */
        for (uint64_t index = 0; index < required / 4; ++index) {
            uint32_t x = (uint32_t)(index % image.width), y = (uint32_t)(index / image.width);
            const uint32_t a[3] = {39,73,66}, b[3] = {31,60,56}; const uint32_t *color = (x / 40 + y / 32) % 3 == 0 ? a : b;
            uint8_t *pixel = pixels + index * 4; uint32_t inverse = 255U - pixel[3];
            for (size_t channel = 0; channel < 3; ++channel) { uint32_t value = pixel[channel] + (color[channel] * inverse + 127U) / 255U; pixel[channel] = (uint8_t)(value > 255U ? 255U : value); }
            pixel[3] = 255;
        }
        SDL_Texture *texture = SDL_CreateTexture(renderer, SDL_PIXELFORMAT_RGBA32, SDL_TEXTUREACCESS_STREAMING, (int)image.width, (int)image.height);
        if (!texture) return EXIT_FAILURE;
        sdl_check(SDL_SetTextureBlendMode(texture, SDL_BLENDMODE_NONE), "blend");
        sdl_check(SDL_UpdateTexture(texture, NULL, pixels, (int)image.stride), "upload"); free(pixels);
        sdl_check(SDL_RenderTexture(renderer, texture, NULL, NULL), "draw");
        if (!SDL_RenderPresent(renderer)) { check(twui_discard_prepared(host.ui, &image.stamp), "discard failed frame"); sdl_check(false, "present"); }
        check(twui_ack_presented(host.ui, &image.stamp), "ack present"); SDL_DestroyTexture(texture);
        twui_rect caret = { .struct_size = sizeof(caret) };
        if (host.text_active && twui_get_ime_rect(host.ui, &image.stamp, &caret) == TWUI_OK) {
            SDL_Rect area = { (int)caret.x, (int)caret.y, (int)SDL_ceilf(caret.width), (int)SDL_ceilf(caret.height) };
            if (area.w < 1) area.w = 1;
            if (area.h < 1) area.h = 1;
            sdl_check(SDL_SetTextInputArea(window, &area, 0), "IME area");
        }
        ++frames; if (max_frames && frames >= max_frames) break;
        SDL_Delay(16);
    }
    sdl_check(SDL_StopTextInput(window), "stop text"); check(twui_destroy(host.ui), "destroy");
    SDL_DestroyRenderer(renderer); SDL_DestroyWindow(window); SDL_Quit();
    printf("presented_frames=%" PRIu64 "\n", frames); return EXIT_SUCCESS;
}
