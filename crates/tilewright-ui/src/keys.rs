// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Infallible ownership tracking for the complete finite physical-key domain.
use crate::Key;

const NAMED_KEYS: usize = 5;
const KEY_COUNT: usize = NAMED_KEYS + (u16::MAX as usize) + 1;
const KEY_WORDS: usize = KEY_COUNT.div_ceil(64);

#[derive(Clone, Debug)]
pub(crate) struct KeyLedger {
    pressed: [u64; KEY_WORDS],
    owned: [u64; KEY_WORDS],
    quarantined: [u64; KEY_WORDS],
}
impl KeyLedger {
    pub(crate) const fn new() -> Self {
        Self {
            pressed: [0; KEY_WORDS],
            owned: [0; KEY_WORDS],
            quarantined: [0; KEY_WORDS],
        }
    }
    fn location(key: Key) -> (usize, u64) {
        let index = match key {
            Key::Enter => 0,
            Key::Backspace => 1,
            Key::Delete => 2,
            Key::SelectAll => 3,
            Key::Escape => 4,
            Key::Other(code) => NAMED_KEYS + usize::from(code),
        };
        (index / 64, 1_u64 << (index % 64))
    }
    pub(crate) fn state(&self, key: Key) -> Option<(bool, bool)> {
        let (word, mask) = Self::location(key);
        (self.pressed[word] & mask != 0).then_some((
            self.owned[word] & mask != 0,
            self.quarantined[word] & mask != 0,
        ))
    }
    pub(crate) fn press(&mut self, key: Key, owned: bool) {
        let (word, mask) = Self::location(key);
        self.pressed[word] |= mask;
        self.owned[word] = (self.owned[word] & !mask) | if owned { mask } else { 0 };
        self.quarantined[word] &= !mask;
    }
    pub(crate) fn release(&mut self, key: Key) -> Option<bool> {
        let owned = self.state(key).map(|(owned, _)| owned);
        let (word, mask) = Self::location(key);
        self.pressed[word] &= !mask;
        self.owned[word] &= !mask;
        self.quarantined[word] &= !mask;
        owned
    }
    pub(crate) fn cancel(&mut self) {
        self.owned = self.pressed;
        self.quarantined = self.pressed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn all_keys() -> impl Iterator<Item = Key> {
        [
            Key::Enter,
            Key::Backspace,
            Key::Delete,
            Key::SelectAll,
            Key::Escape,
        ]
        .into_iter()
        .chain((0..=u16::MAX).map(Key::Other))
    }
    #[test]
    fn complete_domain_is_distinct_and_cancellable_without_capacity_failure() {
        let mut ledger = KeyLedger::new();
        for (index, key) in all_keys().enumerate() {
            assert_eq!(ledger.state(key), None);
            ledger.press(key, index % 2 == 0);
        }
        for (index, key) in all_keys().enumerate() {
            assert_eq!(ledger.state(key), Some((index % 2 == 0, false)));
        }
        ledger.cancel();
        for key in all_keys() {
            assert_eq!(ledger.state(key), Some((true, true)));
            assert_eq!(ledger.release(key), Some(true));
            assert_eq!(ledger.release(key), None);
        }
        assert_eq!(ledger.pressed, [0; KEY_WORDS]);
        assert_eq!(ledger.owned, [0; KEY_WORDS]);
        assert_eq!(ledger.quarantined, [0; KEY_WORDS]);
    }
}
