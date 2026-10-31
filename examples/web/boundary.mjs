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

// Adapter-local physical IDs. They are separate from the semantic command and
// remain stable across keyboard layouts, modifiers and composition sessions.
// Unknown browser codes cannot acquire world controls or custom key effects.
const physicalCodes = [
  'Backquote','Backslash','BracketLeft','BracketRight','Comma','Equal','IntlBackslash',
  'IntlRo','IntlYen','Minus','Period','Quote','Semicolon','Slash',
  ...Array.from({length:10},(_,i)=>`Digit${i}`),
  ...Array.from({length:26},(_,i)=>`Key${String.fromCharCode(65+i)}`),
  'AltLeft','AltRight','Backspace','CapsLock','ContextMenu','ControlLeft','ControlRight',
  'Enter','MetaLeft','MetaRight','ShiftLeft','ShiftRight','Space','Tab',
  'Convert','KanaMode','Lang1','Lang2','Lang3','Lang4','Lang5','NonConvert',
  'Delete','End','Help','Home','Insert','PageDown','PageUp',
  'ArrowDown','ArrowLeft','ArrowRight','ArrowUp','NumLock',
  ...Array.from({length:10},(_,i)=>`Numpad${i}`),
  'NumpadAdd','NumpadBackspace','NumpadClear','NumpadClearEntry','NumpadComma',
  'NumpadDecimal','NumpadDivide','NumpadEnter','NumpadEqual','NumpadHash',
  'NumpadMemoryAdd','NumpadMemoryClear','NumpadMemoryRecall','NumpadMemoryStore',
  'NumpadMemorySubtract','NumpadMultiply','NumpadParenLeft','NumpadParenRight',
  'NumpadStar','NumpadSubtract','Escape','Fn','FnLock','PrintScreen','ScrollLock','Pause',
  ...Array.from({length:24},(_,i)=>`F${i+1}`),
  'BrowserBack','BrowserFavorites','BrowserForward','BrowserHome','BrowserRefresh',
  'BrowserSearch','BrowserStop','Eject','LaunchApp1','LaunchApp2','LaunchMail',
  'MediaPlayPause','MediaSelect','MediaStop','MediaTrackNext','MediaTrackPrevious',
  'Power','Sleep','AudioVolumeDown','AudioVolumeMute','AudioVolumeUp','WakeUp',
];
const physicalIds = new Map(physicalCodes.map((code,index)=>[code,index+1]));
export function physicalCode(code) {
  const id=physicalIds.get(code);
  if (id === undefined) throw new TypeError('Unsupported physical key code');
  return id;
}

// Non-text commands only. Native input/composition performs edits once through
// replace/preedit; composing Enter retains its semantic and an explicit platform
// composition flag so Rust fences that physical key until release.
// Backspace, Delete, selection and clipboard are never applied
// a second time by this key bridge. Custom key effects require keyEffects=true
// after a successful call, independently of consumed=true.
export function browserKey(app,event,pressed) {
  const physical=physicalCode(event.code);
  const composing=event.isComposing || event.keyCode === 229;
  const enter=event.key==='Enter'||(composing&&['Enter','NumpadEnter'].includes(event.code));
  const command=enter?'enter':event.key==='Escape'?'escape':'other';
  return JSON.parse(app.key(physical,command,pressed,Boolean(event.repeat),Boolean(composing)));
}
