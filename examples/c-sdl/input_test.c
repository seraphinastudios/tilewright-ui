/* Copyright 2026 Seraphina Studios. SPDX-License-Identifier: MIT */
/* Exercise the actual C host adapter against SDL's private dummy driver. */
#define main example_main
#include "main.c"
#undef main
#include <assert.h>

static void send_key(struct host *host, SDL_Keycode key, SDL_Scancode code, bool down, bool repeat) {
    SDL_Event e = {0}; e.type = down ? SDL_EVENT_KEY_DOWN : SDL_EVENT_KEY_UP;
    e.key.windowID = SDL_GetWindowID(host->window); e.key.key = key; e.key.scancode = code;
    e.key.repeat = repeat; assert(event(host,&e));
}
static uint32_t allowed(struct host *host, SDL_Scancode code) {
    uint32_t value = 0; check(twui_key_effect_allowed(host->ui,(uint32_t)code,&value),"test effects"); return value;
}
static void reset(struct host *host) {
    if (host->ui) check(twui_destroy(host->ui),"test destroy");
    SDL_Window *window = host->window; memset(host,0,sizeof(*host)); host->window = window;
    twui_config config = TWUI_CONFIG_INIT; check(twui_create(&config,&host->ui),"test create"); sync_control(host);
}
static void present_model(struct host *host, float width) {
    twui_viewport viewport = { .struct_size = sizeof(viewport), .width = width, .height = 600, .scale = 1 };
    twui_stamp stamp = { .struct_size = sizeof(stamp) };
    check(twui_prepare(host->ui,&viewport,&stamp),"test prepare"); sync_control(host);
    check(twui_ack_presented(host->ui,&stamp),"test model commit");
}
int main(void) {
    configure_ime(); sdl_check(SDL_Init(SDL_INIT_VIDEO),"test SDL");
    SDL_Window *window = SDL_CreateWindow("private adapter test",800,600,SDL_WINDOW_HIDDEN); assert(window);
    struct host host = { .window = window }; reset(&host);
    for (int reverse = 0; reverse < 2; ++reverse) {
        reset(&host); check(apply(&host,(twui_input){.kind=TWUI_SET_FOCUS,.value=1}),"focus");
        send_key(&host,SDLK_RETURN,SDL_SCANCODE_RETURN,true,false);
        send_key(&host,SDLK_KP_ENTER,SDL_SCANCODE_KP_ENTER,true,false);
        assert(allowed(&host,SDL_SCANCODE_RETURN) && allowed(&host,SDL_SCANCODE_KP_ENTER));
        SDL_Scancode first = reverse ? SDL_SCANCODE_KP_ENTER : SDL_SCANCODE_RETURN;
        SDL_Scancode second = reverse ? SDL_SCANCODE_RETURN : SDL_SCANCODE_KP_ENTER;
        send_key(&host,reverse ? SDLK_KP_ENTER : SDLK_RETURN,first,false,false);
        assert(!allowed(&host,first) && allowed(&host,second));
        send_key(&host,reverse ? SDLK_RETURN : SDLK_KP_ENTER,second,false,false); assert(!allowed(&host,second));
    }
    for (int commit_first = 0; commit_first < 2; ++commit_first) {
        reset(&host); check(apply(&host,(twui_input){.kind=TWUI_SET_FOCUS,.value=1}),"focus");
        SDL_Event enter={0}; enter.type=SDL_EVENT_KEY_DOWN; enter.key.windowID=SDL_GetWindowID(window); enter.key.key=SDLK_RETURN; enter.key.scancode=SDL_SCANCODE_RETURN;
        SDL_Event text={0}; text.type=SDL_EVENT_TEXT_INPUT; text.text.windowID=SDL_GetWindowID(window); text.text.timestamp=SDL_GetTicksNS(); text.text.text="confirmed";
        SDL_Event batch[2]={commit_first ? text : enter,commit_first ? enter : text};
        begin_batch(&host,batch,2); assert(event(&host,&batch[0])); assert(event(&host,&batch[1]));
        twui_action action={.struct_size=sizeof(action)}; assert(twui_peek_action(host.ui,&action)==TWUI_EMPTY);
        assert(state(&host).composer_bytes==9);
        send_key(&host,SDLK_RETURN,SDL_SCANCODE_RETURN,false,false);
        begin_batch(&host,NULL,0); send_key(&host,SDLK_RETURN,SDL_SCANCODE_RETURN,true,false);
        check(twui_peek_action(host.ui,&action),"confirmed next Enter"); assert(action.text_bytes==9);
    }
    reset(&host); send_key(&host,SDLK_LEFT,SDL_SCANCODE_LEFT,true,false);
    check(apply(&host,(twui_input){.kind=TWUI_SET_FOCUS,.value=1}),"focus");
    check(apply(&host,text_input(&host,TWUI_COMMIT,"abc")),"text"); present_model(&host,800);
    send_key(&host,SDLK_LEFT,SDL_SCANCODE_LEFT,true,true);
    assert(!allowed(&host,SDL_SCANCODE_LEFT) && state(&host).caret == 3);
    uint64_t old_time = host.session_start;
    SDL_Event resized = {0}; resized.type=SDL_EVENT_WINDOW_RESIZED; resized.window.windowID=SDL_GetWindowID(window);
    assert(event(&host,&resized)); assert(host.session_start >= old_time);
    SDL_Event stale = {0}; stale.type=SDL_EVENT_TEXT_INPUT; stale.text.windowID=SDL_GetWindowID(window);
    stale.text.timestamp = old_time; stale.text.text="stale";
    assert(event(&host,&stale)); assert(state(&host).composer_bytes==3);
    uint64_t epoch=host.epoch; present_model(&host,801); assert(host.epoch>epoch);
    twui_input preedit=text_input(&host,TWUI_PREEDIT,"compose"); check(apply(&host,preedit),"preedit"); present_model(&host,801);
    SDL_Event click = {0}; click.type=SDL_EVENT_MOUSE_BUTTON_DOWN; click.button.windowID=SDL_GetWindowID(window);
    click.button.button=SDL_BUTTON_LEFT; click.button.x=10; click.button.y=10;
    assert(event(&host,&click)); assert(!state(&host).focused && state(&host).preedit_bytes==0 && !host.text_active);
    check(twui_destroy(host.ui),"test destroy"); SDL_DestroyWindow(window); SDL_Quit();
    puts("C host physical ownership, epoch synchronization and focus parity passed"); return 0;
}
