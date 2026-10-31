// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
import { exactText } from './boundary.mjs';

// A single outstanding read is bounded. Capture the authoritative selection and
// revision before awaiting permission; completion cannot modify a newer draft,
// focus owner, widget generation, or lifecycle epoch.
export class ClipboardRead {
  #pending = false;
  #generation = 0;
  cancel() { this.#generation += 1; }
  async paste(app, clipboard = globalThis.navigator?.clipboard) {
    if (this.#pending) throw new Error('Clipboard read already pending');
    if (!clipboard?.readText) throw new Error('Clipboard read unavailable');
    const state = JSON.parse(app.snapshot());
    if (state.composing || state.control.focused !== '1') throw new Error('Composer unavailable');
    const generation = this.#generation;
    this.#pending = true;
    try {
      const text = exactText(await clipboard.readText());
      if (generation !== this.#generation) throw new Error('Clipboard read cancelled');
      const start = Math.min(state.anchor, state.focus);
      const end = Math.max(state.anchor, state.focus);
      const result = state.text.slice(0, start) + text + state.text.slice(end);
      const caret = start + text.length;
      app.replace(result, caret, caret, state.generation, state.control.epoch, state.textRevision);
    } finally {
      this.#pending = false;
    }
  }
}
