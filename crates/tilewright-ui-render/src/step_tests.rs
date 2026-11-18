// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! `TextGeometry::step_caret`'s loop, driven by synthetic layouts. The cases
//! with real shaped text are in `tilewright-ui-ffi` (`twui_move_visual`).
use super::step_visual;
use tilewright_ui::{TextAffinity, TextCursor, TextInput, TextSelection, UiError};

const LIMIT: usize = 4096;

fn open(text: &str) -> TextInput {
    TextInput::new(text.into(), LIMIT).unwrap()
}

fn caret(input: &mut TextInput, byte: usize) {
    input
        .set_selection(
            TextSelection::caret(TextCursor::new(byte)),
            input.revision(),
        )
        .unwrap();
}

/// A step closure over fixed display boundaries that counts its calls.
fn stepper<'a>(
    stops: &'static [usize],
    affinity: TextAffinity,
    calls: &'a mut usize,
) -> impl FnMut(TextCursor) -> TextCursor + 'a {
    move |cursor| {
        *calls += 1;
        let byte = stops
            .iter()
            .rev()
            .find(|stop| **stop < cursor.byte)
            .copied()
            .unwrap_or(0);
        TextCursor { byte, affinity }
    }
}

#[test]
fn step_visual_repeats_a_step_that_only_crosses_a_guard() {
    // "a⏎", guard, U+0301: display stops 0, 1, 4 (inside the guard), 7, 9.
    let mut input = open("a\n\u{301}");
    caret(&mut input, 2);
    let snapshot = input.snapshot();
    assert_eq!(snapshot.display_selection.focus.byte, 7);
    let mut calls = 0;
    // Before affinity: a whole-cursor comparison would stop at the guard.
    let cursor = step_visual(
        &snapshot,
        stepper(&[0, 1, 4, 7, 9], TextAffinity::Before, &mut calls),
    )
    .unwrap();
    assert_eq!(cursor.byte, 1);
    assert_eq!(calls, 2);
}

#[test]
fn step_visual_returns_an_affinity_only_move_at_once() {
    // A direction boundary: same byte, other affinity, a real visual stop.
    let mut input = open("ab");
    caret(&mut input, 1);
    let snapshot = input.snapshot();
    let mut calls = 0;
    let flip = |cursor: TextCursor| {
        calls += 1;
        TextCursor {
            affinity: match cursor.affinity {
                TextAffinity::Before => TextAffinity::After,
                TextAffinity::After => TextAffinity::Before,
            },
            ..cursor
        }
    };
    let cursor = step_visual(&snapshot, flip).unwrap();
    assert_eq!(cursor.byte, 1);
    assert_eq!(cursor.affinity, TextAffinity::Before);
    assert_eq!(calls, 1);
}

#[test]
fn step_visual_stops_at_a_fixed_point_and_refuses_composition() {
    let input = open("a\nb");
    let snapshot = input.snapshot();
    let mut calls = 0;
    let cursor = step_visual(&snapshot, |cursor| {
        calls += 1;
        cursor
    })
    .unwrap();
    assert_eq!(cursor, snapshot.selection.focus);
    assert_eq!(calls, 1);
    let mut input = open("a\nb");
    input.update_preedit("x", 1, 0).unwrap();
    let mut calls = 0;
    assert_eq!(
        step_visual(&input.snapshot(), |cursor| {
            calls += 1;
            cursor
        }),
        Err(UiError::InvalidInput)
    );
    assert_eq!(calls, 0);
}
