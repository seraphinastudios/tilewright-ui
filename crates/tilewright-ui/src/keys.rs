// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Infallible ownership tracking for the complete finite physical-key domain.
const KEY_COUNT: usize = (u16::MAX as usize) + 1;
const KEY_WORDS: usize = KEY_COUNT.div_ceil(64);

#[derive(Clone, Debug)]
pub(crate) struct KeyLedger {
    pressed: [u64; KEY_WORDS],
    owned: [u64; KEY_WORDS],
    quarantined: [u64; KEY_WORDS],
    enter_fences: [u64; KEY_WORDS],
}
impl KeyLedger {
    pub(crate) const fn new() -> Self {
        Self {
            pressed: [0; KEY_WORDS],
            owned: [0; KEY_WORDS],
            quarantined: [0; KEY_WORDS],
            enter_fences: [0; KEY_WORDS],
        }
    }
    fn location(physical: u16) -> (usize, u64) {
        let index = usize::from(physical);
        (index / 64, 1_u64 << (index % 64))
    }
    pub(crate) fn state(&self, physical: u16) -> Option<(bool, bool)> {
        let (word, mask) = Self::location(physical);
        (self.pressed[word] & mask != 0).then_some((
            self.owned[word] & mask != 0,
            self.quarantined[word] & mask != 0,
        ))
    }
    pub(crate) fn press(&mut self, physical: u16, owned: bool) {
        let (word, mask) = Self::location(physical);
        self.pressed[word] |= mask;
        self.owned[word] = (self.owned[word] & !mask) | if owned { mask } else { 0 };
        self.quarantined[word] &= !mask;
    }
    pub(crate) fn release(&mut self, physical: u16) -> Option<bool> {
        let owned = self.state(physical).map(|(owned, _)| owned);
        let (word, mask) = Self::location(physical);
        self.pressed[word] &= !mask;
        self.owned[word] &= !mask;
        self.quarantined[word] &= !mask;
        self.enter_fences[word] &= !mask;
        owned
    }
    pub(crate) fn quarantine(&mut self, physical: u16) {
        let (word, mask) = Self::location(physical);
        if self.pressed[word] & mask != 0 {
            self.owned[word] |= mask;
            self.quarantined[word] |= mask;
        }
    }
    pub(crate) fn fence_enter(&mut self, physical: u16) {
        let (word, mask) = Self::location(physical);
        self.enter_fences[word] |= mask;
    }
    pub(crate) fn enter_fenced(&self) -> bool {
        self.enter_fences.iter().any(|word| *word != 0)
    }
    pub(crate) fn cancel(&mut self) {
        self.owned = self.pressed;
        self.quarantined = self.pressed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_domain_is_distinct_and_cancellable_without_capacity_failure() {
        let mut ledger = KeyLedger::new();
        for (index, key) in (0..=u16::MAX).enumerate() {
            assert_eq!(ledger.state(key), None);
            ledger.press(key, index % 2 == 0);
        }
        for (index, key) in (0..=u16::MAX).enumerate() {
            assert_eq!(ledger.state(key), Some((index % 2 == 0, false)));
        }
        ledger.cancel();
        for key in 0..=u16::MAX {
            assert_eq!(ledger.state(key), Some((true, true)));
            assert_eq!(ledger.release(key), Some(true));
            assert_eq!(ledger.release(key), None);
        }
        assert_eq!(ledger.pressed, [0; KEY_WORDS]);
        assert_eq!(ledger.owned, [0; KEY_WORDS]);
        assert_eq!(ledger.quarantined, [0; KEY_WORDS]);
        assert_eq!(ledger.enter_fences, [0; KEY_WORDS]);
    }
}
