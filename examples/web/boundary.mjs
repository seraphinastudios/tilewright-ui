// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

// wasm-bindgen converts unpaired UTF-16 surrogates to replacement characters.
// Refuse them before crossing the boundary, so accepted text is byte-exact.
export function exactText(value) {
  if (typeof value !== 'string') throw new TypeError('Expected string');
  for (let i = 0; i < value.length; i += 1) {
    const unit = value.charCodeAt(i);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(++i);
      if (!(next >= 0xdc00 && next <= 0xdfff)) throw new TypeError('Unpaired surrogate');
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      throw new TypeError('Unpaired surrogate');
    }
  }
  return value;
}

export function identity(value) {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value) ||
      BigInt(value) > 18446744073709551615n) {
    throw new TypeError('Invalid identity');
  }
  return value;
}

// Presses are owned until release. A new control epoch quarantines all held
// world presses even if world permission became true again between samples.
export class PressOwnership {
  #epoch = null;
  #held = new Map();
  observe(control) {
    const epoch = identity(control.epoch);
    const cancelled = [];
    if (this.#epoch !== null && this.#epoch !== epoch) {
      for (const [key, owner] of this.#held) {
        if (owner === 'world') cancelled.push(key);
        this.#held.set(key, 'cancelled');
      }
    }
    this.#epoch = epoch;
    return cancelled;
  }
  down(key, worldAllowed) {
    if (!this.#held.has(key)) this.#held.set(key, worldAllowed ? 'world' : 'ui');
    return this.#held.get(key);
  }
  up(key) {
    const owner = this.#held.get(key) ?? 'cancelled';
    this.#held.delete(key);
    return owner;
  }
}
