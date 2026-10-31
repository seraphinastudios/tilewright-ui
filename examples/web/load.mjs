// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import init, { BrowserUi, WorldView } from './pkg/tilewright_ui_web.js';
import { exactText, identity } from './boundary.mjs';

let initialized;
const ready = () => initialized ??= init();

// Public JS boundary: check strings before wasm-bindgen can replace malformed
// UTF-16. This object owns no widget state; each method delegates to shared Rust.
export async function createUi() {
  await ready();
  const raw = new BrowserUi();
  let alive = true;
  const call = (method,...args) => {
    if (!alive) throw new Error('UI disposed');
    return raw[method](...args);
  };
  const token = (generation,epoch,revision) => [identity(generation),identity(epoch),identity(revision)];
  return Object.freeze({
    snapshot:()=>call('snapshot'),
    focus:value=>call('focus',value),
    focus_widget:value=>call('focus_widget',identity(value)),
    modal:value=>call('modal',value),
    window_focused:value=>call('window_focused',value),
    suspend:value=>call('suspend',value),
    receive:text=>call('receive',exactText(text)),
    scroll:delta=>call('scroll',delta),
    submit:()=>call('submit'),
    key_effects:physical=>call('key_effects',physical),
    key:(physical,command,pressed,repeat,platformComposing=false)=>call('key',physical,exactText(command),pressed,repeat,platformComposing),
    replace:(text,anchor,focus,generation,epoch,revision)=>call('replace',exactText(text),anchor,focus,...token(generation,epoch,revision)),
    preedit:(text,generation,epoch,revision)=>call('preedit',exactText(text),...token(generation,epoch,revision)),
    cancel_composition:(generation,epoch,revision)=>call('cancel_composition',...token(generation,epoch,revision)),
    select:(stamp,anchor,focus,generation,epoch,revision)=>call('select',stamp,anchor,focus,...token(generation,epoch,revision)),
    peek_actions:()=>call('peek_actions'),
    ack_actions:id=>call('ack_actions',identity(id)),
    prepare:(width,height,scale)=>call('prepare',width,height,scale),
    ack_presented:stamp=>call('ack_presented',stamp),
    discard_prepared:stamp=>call('discard_prepared',stamp),
    invalidate_surface:()=>call('invalidate_surface'),
    validate_displayed:stamp=>call('validate_displayed',stamp),
    hit:(stamp,x,y)=>call('hit',stamp,x,y),
    free:()=>{if(alive){alive=false;raw.free();}},
  });
}

export async function createWorld(canvas,width,height,backend='auto') {
  await ready();
  return WorldView.create(canvas,width,height,backend);
}
