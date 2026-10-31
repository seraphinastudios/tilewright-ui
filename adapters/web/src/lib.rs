// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Browser boundary conversions. The shared Rust UI remains authoritative.
#![forbid(unsafe_code)]

/// A browser offset splits a surrogate pair or exceeds the input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidOffset;

/// Convert a UTF-16 DOM offset into an exact UTF-8 byte offset.
/// Grapheme validation belongs to the shared UI after this conversion.
pub fn utf16_to_utf8(text: &str, offset: usize) -> Result<usize, InvalidOffset> {
    let mut units = 0;
    for (byte, character) in text.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += character.len_utf16();
        if units > offset {
            return Err(InvalidOffset);
        }
    }
    if units == offset {
        Ok(text.len())
    } else {
        Err(InvalidOffset)
    }
}

/// Convert an exact UTF-8 character boundary into a UTF-16 DOM offset.
pub fn utf8_to_utf16(text: &str, offset: usize) -> Result<usize, InvalidOffset> {
    text.get(..offset)
        .map(|prefix| prefix.encode_utf16().count())
        .ok_or(InvalidOffset)
}

mod browser;
pub use browser::BrowserUi;

#[cfg(target_arch = "wasm32")]
mod world;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_unicode_character_boundaries_without_splitting_surrogates() {
        let text = "a👩🏽\u{200d}💻e\u{301}אב";
        for byte in text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain([text.len()])
        {
            let units = utf8_to_utf16(text, byte).unwrap();
            assert_eq!(utf16_to_utf8(text, units), Ok(byte));
        }
        assert_eq!(utf16_to_utf8(text, 2), Err(InvalidOffset));
        assert_eq!(utf16_to_utf8(text, 100), Err(InvalidOffset));
        assert_eq!(utf8_to_utf16(text, 2), Err(InvalidOffset));
        assert_eq!(utf8_to_utf16(text, 100), Err(InvalidOffset));
    }
}
