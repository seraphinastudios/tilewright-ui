// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { ClipboardRead } from './clipboard.mjs';

const snapshot = JSON.stringify({ text:'abc', anchor:1, focus:2, generation:'1', textRevision:'3', composing:false, control:{epoch:'2',focused:'1'} });
test('clipboard completion carries captured fencing tokens and exact selection', async () => {
  const calls=[];
  const app={snapshot:()=>snapshot,replace:(...args)=>calls.push(args)};
  await new ClipboardRead().paste(app,{readText:async()=>'👩🏽‍💻'});
  assert.deepEqual(calls,[['a👩🏽‍💻c',8,8,'1','2','3']]);
});
test('permission failure, cancellation, and malformed text never edit', async () => {
  let edits=0;
  const app={snapshot:()=>snapshot,replace:()=>{edits+=1;}};
  const clipboard=new ClipboardRead();
  await assert.rejects(clipboard.paste(app,{readText:async()=>{throw new Error('denied');}}));
  await assert.rejects(clipboard.paste(app,{readText:async()=>'\ud800'}));
  let resolve;
  const pending=clipboard.paste(app,{readText:()=>new Promise(r=>{resolve=r;})});
  await assert.rejects(clipboard.paste(app,{readText:async()=>'second'}));
  clipboard.cancel(); resolve('first'); await assert.rejects(pending);
  assert.equal(edits,0);
});


test('keyboard clipboard requests require current unquarantined ownership', async () => {
  let reads=0;
  const app={snapshot:()=>snapshot,replace:()=>assert.fail('must not edit'),key_effects:()=>false};
  await assert.rejects(new ClipboardRead().pasteKey(app,12,{readText:async()=>{reads+=1;return 'text';}}));
  assert.equal(reads,0);
});
