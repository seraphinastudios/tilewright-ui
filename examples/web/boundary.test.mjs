// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { exactText, identity, PressOwnership } from './boundary.mjs';

test('text crosses without silent surrogate replacement', () => {
  for (const text of ['hello', 'e\u0301', '👩🏽‍💻', 'مرحبا', 'אבג']) assert.equal(exactText(text), text);
  for (const text of ['\ud800', '\udfff', 'x\ud800a', null, 12]) assert.throws(() => exactText(text));
});
test('identities never round or wrap', () => {
  assert.equal(identity('18446744073709551615'), '18446744073709551615');
  for (const value of [1, '-1', '01', '18446744073709551616', '1e3']) assert.throws(() => identity(value));
});
test('epoch changes quarantine release even when final permission is true', () => {
  const ownership = new PressOwnership();
  ownership.observe({ epoch: '1' });
  assert.equal(ownership.down('KeyW', true), 'world');
  assert.deepEqual(ownership.observe({ epoch: '3' }), ['KeyW']);
  assert.equal(ownership.up('KeyW'), 'cancelled');
  assert.equal(ownership.down('KeyW', true), 'world');
  assert.equal(ownership.up('KeyW'), 'world');
  assert.equal(ownership.down('Enter', false), 'ui');
  assert.equal(ownership.up('Enter'), 'ui');
});

test('browser physical IDs separate Enter sources and native edit semantics', async () => {
  const {physicalCode,browserKey}=await import('./boundary.mjs');
  assert.notEqual(physicalCode('Enter'),physicalCode('NumpadEnter'));
  assert.notEqual(physicalCode('ControlLeft'),physicalCode('ControlRight'));
  assert.throws(()=>physicalCode('Unidentified'));
  const calls=[];const app={key:(...args)=>{calls.push(args);return '{"consumed":true,"keyEffects":false}';}};
  assert.deepEqual(browserKey(app,{code:'Enter',key:'Enter',isComposing:true},true),{consumed:true,keyEffects:false});
  browserKey(app,{code:'NumpadEnter',key:'Enter',repeat:false},true);
  browserKey(app,{code:'Backspace',key:'Backspace'},true);
  assert.deepEqual(calls.map(call=>call[1]),['other','enter','other']);
  assert.notEqual(calls[0][0],calls[1][0]);
});
