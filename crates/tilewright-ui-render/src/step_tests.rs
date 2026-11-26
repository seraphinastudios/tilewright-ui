// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! `TextGeometry::step_caret`'s loop, driven by synthetic layouts, and with
//! the `prototype` feature by real shaped text. Further cases with real
//! shaped text are in `tilewright-ui-ffi` (`twui_move_visual`).
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

/// Every display byte drawn at its own place, 10 px apart.
#[allow(clippy::cast_precision_loss, clippy::unnecessary_wraps)] // `step_visual`'s `caret`.
fn distinct(cursor: TextCursor) -> Option<[f32; 2]> {
    Some([cursor.byte as f32 * 10.0, 0.0])
}

fn flip(cursor: TextCursor) -> TextCursor {
    TextCursor {
        affinity: match cursor.affinity {
            TextAffinity::Before => TextAffinity::After,
            TextAffinity::After => TextAffinity::Before,
        },
        ..cursor
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
        distinct,
    )
    .unwrap();
    assert_eq!(cursor.byte, 1);
    assert_eq!(calls, 2);
}

#[test]
fn step_visual_repeats_a_stop_beside_a_guard_that_lands_behind() {
    // U+0600, guard, "⏎": display stops 0, 2, 5 (inside the guard), 8. The
    // layout steps Left from 8 (drawn at x 80) to 5 (x 50), then to 0. 5
    // maps to committed 2, drawn where display 2 is.
    let mut input = open("\u{600}\n");
    caret(&mut input, 3);
    let snapshot = input.snapshot();
    assert_eq!(snapshot.display_selection.focus.byte, 8);
    assert_eq!(snapshot.markers[0].display, 2..8);
    let press = |two: f32| {
        let mut calls = 0;
        let cursor = step_visual(
            &snapshot,
            stepper(&[0, 5, 8], TextAffinity::Before, &mut calls),
            |cursor| {
                Some(if cursor.byte == 2 {
                    [two, 0.0]
                } else {
                    distinct(cursor).unwrap()
                })
            },
        )
        .unwrap();
        (cursor.byte, calls)
    };
    // Display 2 drawn right of where the press began (as in a right-to-left
    // paragraph): stopping at 5 would draw the caret behind it. Not a stop.
    assert_eq!(press(90.0), (0, 2));
    // At the start's own place: not a stop either.
    assert_eq!(press(80.0), (0, 2));
    // Drawn at 5's place (the guard has no width there), or beyond the
    // start the way the press went: a stop, as before.
    assert_eq!(press(50.0), (2, 1));
    assert_eq!(press(20.0), (2, 1));
}

#[test]
fn step_visual_returns_an_affinity_only_move_to_another_place_at_once() {
    // A direction boundary: same byte, other affinity, drawn elsewhere.
    let mut input = open("ab");
    caret(&mut input, 1);
    let snapshot = input.snapshot();
    let mut calls = 0;
    let step = |cursor: TextCursor| {
        calls += 1;
        flip(cursor)
    };
    let apart = |cursor: TextCursor| {
        Some(match cursor.affinity {
            TextAffinity::Before => [10.0, 0.0],
            TextAffinity::After => [17.0, 0.0],
        })
    };
    let cursor = step_visual(&snapshot, step, apart).unwrap();
    assert_eq!(cursor.byte, 1);
    assert_eq!(cursor.affinity, TextAffinity::Before);
    assert_eq!(calls, 1);
}

#[test]
fn step_visual_steps_past_an_affinity_only_move_that_stays_in_place() {
    // Two stops for byte 1 a rounding error apart (97.700005 and 97.7, as
    // the layout gives at 20 px), then byte 0 further left.
    let mut input = open("ab");
    caret(&mut input, 1);
    let snapshot = input.snapshot();
    let mut calls = 0;
    let step = |cursor: TextCursor| {
        calls += 1;
        match (cursor.byte, cursor.affinity) {
            (1, TextAffinity::After) => flip(cursor),
            _ => TextCursor::new(0),
        }
    };
    let noise = |cursor: TextCursor| {
        Some(match (cursor.byte, cursor.affinity) {
            (1, TextAffinity::After) => [97.700_005, 4.0],
            (1, TextAffinity::Before) => [97.7, 4.0],
            _ => [88.0, 4.0],
        })
    };
    let cursor = step_visual(&snapshot, step, noise).unwrap();
    assert_eq!(cursor, TextCursor::new(0));
    assert_eq!(calls, 2);

    // Just over the tolerance is a place of its own, and a stop.
    let mut calls = 0;
    let step = |cursor: TextCursor| {
        calls += 1;
        flip(cursor)
    };
    let apart = |cursor: TextCursor| {
        Some(match cursor.affinity {
            TextAffinity::After => [97.7 + 1.0 / 32.0, 4.0],
            TextAffinity::Before => [97.7, 4.0],
        })
    };
    let cursor = step_visual(&snapshot, step, apart).unwrap();
    assert_eq!((cursor.byte, cursor.affinity), (1, TextAffinity::Before));
    assert_eq!(calls, 1);

    // The same x on another line is another place too: y counts.
    let mut calls = 0;
    let step = |cursor: TextCursor| {
        calls += 1;
        flip(cursor)
    };
    let lines = |cursor: TextCursor| {
        Some(match cursor.affinity {
            TextAffinity::After => [97.7, 4.0],
            TextAffinity::Before => [97.7, 26.0],
        })
    };
    let cursor = step_visual(&snapshot, step, lines).unwrap();
    assert_eq!((cursor.byte, cursor.affinity), (1, TextAffinity::Before));
    assert_eq!(calls, 1);
}

#[test]
fn step_visual_is_bounded_and_stops_at_a_fixed_point() {
    // A layout that only ever flips the affinity in place: four steps, then
    // the caret stays at its byte.
    let mut input = open("ab");
    caret(&mut input, 1);
    let snapshot = input.snapshot();
    let mut calls = 0;
    let cursor = step_visual(
        &snapshot,
        |cursor| {
            calls += 1;
            flip(cursor)
        },
        |_| Some([5.0, 0.0]),
    )
    .unwrap();
    assert_eq!(cursor.byte, 1);
    assert_eq!(calls, 4);
    // A cursor without a caret is never in the same place: a stop.
    let mut calls = 0;
    step_visual(
        &snapshot,
        |cursor| {
            calls += 1;
            flip(cursor)
        },
        |_| None,
    )
    .unwrap();
    assert_eq!(calls, 1);
}

#[test]
fn step_visual_stops_at_a_fixed_point_and_refuses_composition() {
    let input = open("a\nb");
    let snapshot = input.snapshot();
    let mut calls = 0;
    let cursor = step_visual(
        &snapshot,
        |cursor| {
            calls += 1;
            cursor
        },
        distinct,
    )
    .unwrap();
    assert_eq!(cursor, snapshot.selection.focus);
    assert_eq!(calls, 1);
    let mut input = open("a\nb");
    input.update_preedit("x", 1, 0).unwrap();
    let mut calls = 0;
    assert_eq!(
        step_visual(
            &input.snapshot(),
            |cursor| {
                calls += 1;
                cursor
            },
            distinct,
        ),
        Err(UiError::InvalidInput)
    );
    assert_eq!(calls, 0);
}

/// Real shaped text: every press is driven through `step_visual` with the
/// layout `UiCompiler::compile` would build for the field (the display text
/// at the composer's size), and compared with the 0.1.0 loop.
#[cfg(feature = "prototype")]
mod shaped {
    use super::super::{SAME_CARET_POSITION as SAME, neutral_cursor, renderer_cursor, step_visual};
    use super::LIMIT;
    use crate::{TextLimits, TextSystem, UnicodeLayout, VisualMotion};
    use tilewright_ui::{TextCursor, TextInput, TextInputSnapshot, TextSelection, UiError};

    /// `step_visual` as released in 0.1.0: no caret-position rule, at most
    /// three layout steps.
    fn released_step(
        snapshot: &TextInputSnapshot,
        mut step: impl FnMut(TextCursor) -> TextCursor,
    ) -> Result<TextCursor, UiError> {
        if snapshot.composition.is_some() {
            return Err(UiError::InvalidInput);
        }
        let start = snapshot.selection.focus;
        let mut display = snapshot.display_selection.focus;
        let mut result = start;
        for _ in 0..3 {
            let next = step(display);
            if next == display {
                break;
            }
            let moved = next.byte != display.byte;
            display = next;
            result = snapshot.committed_cursor(display)?;
            if result.byte != start.byte || !moved {
                break;
            }
        }
        Ok(result)
    }

    pub(super) struct Field {
        input: TextInput,
        layout: UnicodeLayout,
    }

    impl Field {
        pub(super) fn open(system: &mut TextSystem, value: &str, size: f32) -> Option<Self> {
            let input = TextInput::new(value.into(), LIMIT).ok()?;
            let layout = system
                .layout(&input.snapshot().display_text, size, None)
                .ok()?;
            Some(Self { input, layout })
        }
        /// Committed grapheme boundaries, in byte order.
        pub(super) fn boundaries(&mut self) -> Vec<usize> {
            let len = self.input.text().len();
            (0..=len)
                .filter(|byte| self.place(TextCursor::new(*byte)).is_ok())
                .collect()
        }
        pub(super) fn place(&mut self, cursor: TextCursor) -> Result<(), UiError> {
            self.input
                .set_selection(TextSelection::caret(cursor), self.input.revision())
        }
        pub(super) fn focus(&self) -> TextCursor {
            self.input.selection().focus
        }
        /// Where the committed focus's caret is drawn.
        pub(super) fn drawn(&self) -> Option<[f32; 2]> {
            let focus = self.input.snapshot().display_selection.focus;
            self.layout
                .caret(renderer_cursor(focus))
                .map(|rect| [rect.x, rect.y])
        }
        /// One press with the current loop, or with the 0.1.0 loop.
        pub(super) fn press(&mut self, motion: VisualMotion, released: bool) -> TextCursor {
            self.counted_press(motion, released).0
        }
        /// [`Field::press`], and how many layout steps it took.
        pub(super) fn counted_press(
            &mut self,
            motion: VisualMotion,
            released: bool,
        ) -> (TextCursor, usize) {
            let snapshot = self.input.snapshot();
            let layout = &self.layout;
            let mut steps = 0;
            let step = |cursor| {
                steps += 1;
                neutral_cursor(layout.move_visual(renderer_cursor(cursor), motion))
            };
            let cursor = if released {
                released_step(&snapshot, step)
            } else {
                step_visual(&snapshot, step, |cursor| {
                    layout
                        .caret(renderer_cursor(cursor))
                        .map(|rect| [rect.x, rect.y])
                })
            }
            .unwrap();
            self.place(cursor).unwrap();
            (cursor, steps)
        }
    }

    pub(super) fn same(a: Option<[f32; 2]>, b: Option<[f32; 2]>) -> bool {
        super::super::same_position(a, b)
    }

    fn system() -> TextSystem {
        TextSystem::new(crate::tests::fonts(), TextLimits::default()).unwrap()
    }

    fn bytes(field: &mut Field, start: usize, motion: VisualMotion, released: bool) -> Vec<usize> {
        field.place(TextCursor::new(start)).unwrap();
        (0..8).map(|_| field.press(motion, released).byte).collect()
    }

    /// The consumer's case: a press next to the second marker did nothing at
    /// 20 px. Every committed position is reached in both directions, and no
    /// press leaves the caret where it was drawn until the end is reached.
    #[test]
    fn markers_at_twenty_pixels_have_no_press_that_does_nothing() {
        let value = "Trail\nmar\u{2028}ker\u{2029}";
        let mut system = system();
        let mut field = Field::open(&mut system, value, 20.0).unwrap();
        assert_eq!(
            bytes(&mut field, value.len(), VisualMotion::Left, true),
            [15, 14, 13, 12, 9, 9, 8, 7],
            "the 0.1.0 loop stalled at 9"
        );
        let boundaries = field.boundaries();
        for (motion, start, end) in [
            (VisualMotion::Left, value.len(), 0),
            (VisualMotion::Right, 0, value.len()),
        ] {
            field.place(TextCursor::new(start)).unwrap();
            let mut visited = vec![start];
            while field.focus().byte != end {
                let before = field.drawn();
                field.press(motion, false);
                assert!(
                    !same(before, field.drawn()),
                    "{motion:?} did nothing: {visited:?}"
                );
                visited.push(field.focus().byte);
                assert!(visited.len() <= boundaries.len(), "{motion:?}: {visited:?}");
            }
            let mut sorted = visited.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, boundaries, "{motion:?}: {visited:?}");
        }
        assert_eq!(
            bytes(&mut field, value.len(), VisualMotion::Left, false),
            [15, 14, 13, 12, 9, 8, 7, 6]
        );
        assert_eq!(
            bytes(&mut field, 0, VisualMotion::Right, false),
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    /// Plain right-to-left text had the same one-press stall at 20 px:
    /// two stops at x = 7.48 and 7.4799995.
    #[test]
    fn plain_right_to_left_text_has_no_stall() {
        let value = "\u{5d0}\u{5d1}\u{5d2}";
        let mut system = system();
        let mut field = Field::open(&mut system, value, 20.0).unwrap();
        assert_eq!(
            bytes(&mut field, 0, VisualMotion::Left, true)[..4],
            [2, 4, 4, 6]
        );
        assert_eq!(
            bytes(&mut field, 0, VisualMotion::Left, false)[..3],
            [2, 4, 6]
        );
        assert_eq!(
            bytes(&mut field, value.len(), VisualMotion::Right, false)[..3],
            [4, 2, 0]
        );
    }

    /// A marker's guard beside right-to-left text. In
    /// `RLM กำ b U+0600 U+2028 U+0300 가` (a right-to-left paragraph) End then
    /// Left cycled 12, 10, 9, 12 for ever: from 9 the layout steps to the
    /// guard's inner side, drawn to the left, which maps to committed 12,
    /// drawn at the right end. Every press now moves the caret the pressed
    /// way until it stays at that end.
    #[test]
    fn a_guard_beside_right_to_left_text_does_not_cycle() {
        let value = "\u{200f}\u{e01}\u{e33}b\u{600}\u{2028}\u{300}\u{ac00}";
        let mut system = system();
        for size in [18.0, 20.0] {
            let mut field = Field::open(&mut system, value, size).unwrap();
            field.press(VisualMotion::End, false);
            assert_eq!(field.focus().byte, 12);
            assert_eq!(
                bytes(&mut field, 12, VisualMotion::Left, true)[..4],
                [10, 9, 12, 10],
                "the 0.1.0 loop cycled"
            );
            for (start, motion) in [
                (VisualMotion::End, VisualMotion::Left),
                (VisualMotion::Home, VisualMotion::Right),
            ] {
                field.press(start, false);
                let mut visited = vec![field.focus()];
                loop {
                    let before = field.drawn().unwrap();
                    let cursor = field.press(motion, false);
                    if cursor == *visited.last().unwrap() {
                        break;
                    }
                    let after = field.drawn().unwrap();
                    let moved = if motion == VisualMotion::Left {
                        after[0] < before[0]
                    } else {
                        after[0] > before[0]
                    };
                    assert!(moved, "{size} {motion:?} went the other way: {visited:?}");
                    assert!(!visited.contains(&cursor), "{size} {motion:?}: {visited:?}");
                    visited.push(cursor);
                }
                let walked: Vec<_> = visited.iter().map(|cursor| cursor.byte).collect();
                assert_eq!(
                    walked,
                    if motion == VisualMotion::Left {
                        &[12, 10, 9, 20, 17][..]
                    } else {
                        &[17, 15, 3, 9, 10, 0][..]
                    },
                    "{size} {motion:?}"
                );
            }
        }
    }

    pub(super) const ALPHABET: [&str; 9] = [
        "a",
        "\u{5d0}",
        "\n",
        "\r",
        "\u{301}",
        "\u{600}",
        "\t",
        "\u{2028}",
        "\u{1f469}\u{200d}",
    ];

    /// What both loops do on every value of an enumeration. A state is a
    /// committed caret (every grapheme boundary, both affinities) and a
    /// direction; a press maps a state to a state.
    #[derive(Debug, Default)]
    pub(super) struct Counts {
        pub(super) values: usize,
        pub(super) refused: usize,
        pub(super) states: usize,
        /// Presses whose result differs between the two loops.
        pub(super) differing: usize,
        /// Of those, presses where the 0.1.0 loop changed only the affinity
        /// and left the caret where it was drawn: removed do-nothing presses.
        pub(super) differing_removed_stall: usize,
        /// Of those, presses where the 0.1.0 loop stopped beside a marker's
        /// guard, so the caret stayed where it was drawn or went the other
        /// way, and the new loop moves it the pressed way.
        pub(super) differing_wrong_way: usize,
        /// The rest of `differing`; each is suspect and listed by the report.
        pub(super) differing_other: usize,
        /// Presses that leave the caret where it was drawn although pressing
        /// on in that direction later moves it: with the 0.1.0 loop, with the
        /// new loop, and (i) with the new loop where the 0.1.0 loop moved.
        pub(super) stalls_released: usize,
        pub(super) stalls: usize,
        pub(super) stalls_new_only: usize,
        /// Of `stalls_released`, those where the 0.1.0 press changed only the
        /// affinity (the rounding stall); the rest change the committed byte
        /// to another position drawn at the same place.
        pub(super) stalls_released_affinity_only: usize,
        pub(super) stalls_affinity_only: usize,
        /// Of `stalls`, those whose two caret positions are bit-identical:
        /// two committed positions the layout draws at one place.
        pub(super) stalls_identical: usize,
        /// (ii) States from which the 0.1.0 loop reaches a drawn place that
        /// the new loop, pressed on from the same state, never reaches; only
        /// places reached before a press that went against the pressed way.
        pub(super) lost_places: usize,
        /// States whose repeated presses end in a cycle, not a fixed point.
        pub(super) cycles_released: usize,
        pub(super) cycles: usize,
        /// (iii) The most layout steps one press took, and how many presses
        /// used the new loop's whole bound.
        pub(super) most_steps_released: usize,
        pub(super) most_steps: usize,
        pub(super) presses_at_bound: usize,
        pub(super) suspects: Vec<String>,
    }

    /// Compare both loops on every value over [`ALPHABET`] of 1 to `length`
    /// pieces, at both sizes.
    pub(super) fn enumerate(length: u32) -> Counts {
        let mut system = system();
        let mut counts = Counts::default();
        for len in 1..=length {
            for mut index in 0..ALPHABET.len().pow(len) {
                let mut value = String::new();
                for _ in 0..len {
                    value.push_str(ALPHABET[index % ALPHABET.len()]);
                    index /= ALPHABET.len();
                }
                for size in [18.0, 20.0] {
                    count_value(&mut system, &value, size, &mut counts);
                }
            }
            system.clear_cache();
        }
        counts
    }

    /// The states one state reaches by pressing on, itself first, until
    /// a state repeats; and whether that repeat is a cycle, not a fixed point.
    fn walk(next: &[usize], from: usize) -> (Vec<usize>, bool) {
        let mut seen = vec![from];
        loop {
            let to = next[*seen.last().unwrap()];
            if let Some(at) = seen.iter().position(|state| *state == to) {
                return (seen.clone(), at != seen.len() - 1);
            }
            seen.push(to);
        }
    }

    /// How far a press from state `a` to state `b` moves the drawn caret
    /// the way `motion` points (negative: against it).
    fn shift(drawn: &[Option<[f32; 2]>], motion: VisualMotion, a: usize, b: usize) -> Option<f32> {
        drawn[a].zip(drawn[b]).map(|(a, b)| {
            if motion == VisualMotion::Left {
                a[0] - b[0]
            } else {
                b[0] - a[0]
            }
        })
    }

    fn count_value(system: &mut TextSystem, value: &str, size: f32, counts: &mut Counts) {
        let Some(mut field) = Field::open(system, value, size) else {
            counts.refused += 1;
            return;
        };
        counts.values += 1;
        let mut states = Vec::new();
        for byte in field.boundaries() {
            for affinity in [
                tilewright_ui::TextAffinity::Before,
                tilewright_ui::TextAffinity::After,
            ] {
                states.push(TextCursor { byte, affinity });
            }
        }
        let drawn: Vec<_> = states
            .iter()
            .map(|state| {
                field.place(*state).unwrap();
                field.drawn()
            })
            .collect();
        let index = |cursor: TextCursor| states.iter().position(|state| *state == cursor).unwrap();
        for motion in [VisualMotion::Left, VisualMotion::Right] {
            let mut tables = [Vec::new(), Vec::new()];
            for (table, released) in tables.iter_mut().zip([true, false]) {
                for state in &states {
                    field.place(*state).unwrap();
                    let (cursor, steps) = field.counted_press(motion, released);
                    if released {
                        counts.most_steps_released = counts.most_steps_released.max(steps);
                    } else {
                        counts.most_steps = counts.most_steps.max(steps);
                        counts.presses_at_bound +=
                            usize::from(steps == super::super::MAXIMUM_STEPS);
                    }
                    table.push(index(cursor));
                }
            }
            let [old, new] = tables;
            for from in 0..states.len() {
                counts.states += 1;
                let (old_walk, old_cycle) = walk(&old, from);
                let (new_walk, new_cycle) = walk(&new, from);
                counts.cycles_released += usize::from(old_cycle);
                counts.cycles += usize::from(new_cycle);
                let stalls = |next: &[usize], walk: &[usize]| {
                    same(drawn[from], drawn[next[from]])
                        && walk.iter().any(|state| !same(drawn[from], drawn[*state]))
                };
                let old_stall = stalls(&old, &old_walk);
                let new_stall = stalls(&new, &new_walk);
                let affinity_only = |next: &[usize]| states[next[from]].byte == states[from].byte;
                counts.stalls_released += usize::from(old_stall);
                counts.stalls += usize::from(new_stall);
                counts.stalls_new_only += usize::from(new_stall && !old_stall);
                counts.stalls_released_affinity_only +=
                    usize::from(old_stall && affinity_only(&old));
                counts.stalls_affinity_only += usize::from(new_stall && affinity_only(&new));
                counts.stalls_identical +=
                    usize::from(new_stall && drawn[from] == drawn[new[from]]);
                let shift = |a: usize, b: usize| shift(&drawn, motion, a, b);
                let pressed_way = |to: usize| shift(from, to).is_some_and(|shift| shift > SAME);
                if old[from] != new[from] {
                    counts.differing += 1;
                    if affinity_only(&old) && same(drawn[from], drawn[old[from]]) {
                        counts.differing_removed_stall += 1;
                    } else if !pressed_way(old[from]) && pressed_way(new[from]) {
                        counts.differing_wrong_way += 1;
                    } else {
                        counts.differing_other += 1;
                        counts.suspects.push(format!(
                            "{value:?} {size} {motion:?} {:?}: {:?} -> {:?}",
                            states[from], states[old[from]], states[new[from]]
                        ));
                    }
                }
                // Only places the 0.1.0 loop reaches before a press that
                // went against the pressed way count: one past such a press
                // (beside a guard in mixed-direction text, removed in 0.1.3)
                // is not where pressing on is meant to go.
                let reach = old_walk
                    .windows(2)
                    .position(|pair| shift(pair[0], pair[1]).is_some_and(|shift| shift < -SAME))
                    .map_or(old_walk.len(), |at| at + 1);
                if old_walk[..reach].iter().any(|place| {
                    !new_walk
                        .iter()
                        .any(|reached| same(drawn[*place], drawn[*reached]))
                }) {
                    counts.lost_places += 1;
                    counts.suspects.push(format!(
                        "lost {value:?} {size} {motion:?} {:?}: {old_walk:?} vs {new_walk:?}",
                        states[from]
                    ));
                }
            }
        }
    }

    fn check(counts: &Counts) {
        assert_eq!(counts.refused, 0, "{counts:#?}");
        assert_eq!(counts.stalls_new_only, 0, "{counts:#?}");
        assert_eq!(counts.stalls_affinity_only, 0, "{counts:#?}");
        assert_eq!(counts.differing_other, 0, "{counts:#?}");
        assert_eq!(counts.lost_places, 0, "{counts:#?}");
        assert_eq!(counts.cycles, 0, "{counts:#?}");
        assert!(counts.most_steps <= super::super::MAXIMUM_STEPS);
    }

    /// Every value up to three pieces long: the new loop never does nothing
    /// where the 0.1.0 loop moved, changes no press except to remove a
    /// press that did nothing or went against the pressed way, loses no
    /// drawn place and never cycles.
    #[test]
    fn small_alphabet_enumeration_keeps_every_place() {
        check(&enumerate(3));
    }

    /// The longer enumeration, for review evidence:
    /// `TILEWRIGHT_UI_STEP_ENUMERATION=5 cargo test --release ... -- --ignored`.
    #[test]
    #[ignore = "minutes long; run by hand for review evidence"]
    fn small_alphabet_enumeration_report() {
        let length = std::env::var("TILEWRIGHT_UI_STEP_ENUMERATION")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(4);
        let counts = enumerate(length);
        println!("length {length}: {counts:#?}");
        check(&counts);
    }
}
