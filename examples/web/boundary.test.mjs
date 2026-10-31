// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import assert from 'node:assert/strict';
import test from 'node:test';
import { exactText, identity, PressOwnership } from './boundary.mjs';

test('text crosses without silent surrogate replacement', () => {
  for (const text of ['hello', 'e\u0301', '👩🏽‍💻', 'مرحبا', 'אבג']) assert.equal(exactText(text), text);
  for (const text of ['\ud800', '\udfff', 'x\ud800a']) assert.throws(() => exactText(text));
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
