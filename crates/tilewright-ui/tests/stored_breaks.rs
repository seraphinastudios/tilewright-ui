// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Single-line rules: new line breaks and controls are refused; stored ones
//! open unchanged, show as one marker each and round-trip byte for byte.
use std::collections::BTreeSet;

use tilewright_ui::*;
use unicode_segmentation::UnicodeSegmentation;

const LIMIT: usize = 4096;

fn open(text: &str) -> TextInput {
    TextInput::new(text.into(), LIMIT).unwrap()
}

fn caret(input: &mut TextInput, byte: usize) {
    select(input, byte, byte);
}

fn select(input: &mut TextInput, anchor: usize, focus: usize) {
    input
        .set_selection(
            TextSelection {
                anchor: TextCursor::new(anchor),
                focus: TextCursor::new(focus),
            },
            input.revision(),
        )
        .unwrap();
}

fn boundaries(text: &str) -> BTreeSet<usize> {
    text.grapheme_indices(true)
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .collect()
}

fn committed(snapshot: &TextInputSnapshot, shown: usize) -> Result<usize, UiError> {
    snapshot
        .committed_cursor(TextCursor::new(shown))
        .map(|cursor| cursor.byte)
}

/// A focused single field bound to `stored`.
fn field(stored: &str) -> Ui {
    let mut ui = Ui::with_composition(Limits::default(), Composition::SingleField).unwrap();
    ui.bind_field(stored).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui
}

fn target(ui: &Ui) -> TextTarget {
    ui.text_target().unwrap()
}

fn submitted(ui: &mut Ui) -> String {
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    let batch = ui.peek_actions().unwrap().clone();
    ui.ack_actions(batch.id).unwrap();
    match &batch.actions[0] {
        Action::SubmitField { text, .. } => text.clone(),
        Action::SubmitChat { .. } => panic!("chat action from a field"),
    }
}

/// Every marked class and its marker, a CR LF pair as one.
const CLASSES: [(&str, char); 12] = [
    ("\n", LINE_BREAK_MARKER),
    ("\r", LINE_BREAK_MARKER),
    ("\r\n", LINE_BREAK_MARKER),
    ("\u{b}", LINE_BREAK_MARKER),
    ("\u{c}", LINE_BREAK_MARKER),
    ("\u{85}", LINE_BREAK_MARKER),
    ("\u{2028}", LINE_BREAK_MARKER),
    ("\u{2029}", PARAGRAPH_MARKER),
    ("\t", TAB_MARKER),
    ("\u{0}", CONTROL_MARKER),
    ("\u{1b}", CONTROL_MARKER),
    ("\u{7f}", CONTROL_MARKER),
];

#[test]
fn stored_breaks_open_unchanged_as_one_marker_each_anywhere() {
    for (stored, marker) in CLASSES {
        for (value, shown) in [
            (format!("{stored}A"), format!("{marker}A")),
            (format!("A{stored}B"), format!("A{marker}B")),
            (format!("A{stored}"), format!("A{marker}")),
            (format!("{stored}{stored}"), format!("{marker}{marker}")),
            (stored.to_owned(), marker.to_string()),
        ] {
            let input = open(&value);
            let snapshot = input.snapshot();
            assert_eq!(snapshot.display_text, shown, "{value:?}");
            assert_eq!(snapshot.committed, value, "showing never changes the draft");
            assert_eq!(input.text(), value);
            assert_eq!(mark_stored_breaks(&value).0, shown);
            for unit in &snapshot.markers {
                assert_eq!(&value[unit.source.clone()], stored);
                assert_eq!(
                    unit.display, unit.glyph,
                    "no guard without a joining neighbour"
                );
            }
        }
    }
    // Several breaks of different kinds side by side keep their own markers.
    let (shown, markers) = mark_stored_breaks("\r\n\n\u{2029}\t\u{7}");
    assert_eq!(
        shown,
        format!(
            "{LINE_BREAK_MARKER}{LINE_BREAK_MARKER}{PARAGRAPH_MARKER}{TAB_MARKER}{CONTROL_MARKER}"
        )
    );
    let sources: Vec<_> = markers.iter().map(|unit| unit.source.clone()).collect();
    assert_eq!(sources, [0..2, 2..3, 3..6, 6..7, 7..8]);
    // Plain text and format characters that are not controls (bidi marks,
    // joiners) are shown unchanged and never refused.
    for plain in [
        "",
        "Trail marker",
        "e\u{301}",
        "\u{200f}עברית\u{200d}",
        "👨‍👩‍👧",
    ] {
        let (shown, markers) = mark_stored_breaks(plain);
        assert_eq!(shown, plain);
        assert!(markers.is_empty());
        assert!(!plain.chars().any(refuses_single_line));
    }
}

#[test]
fn a_bound_field_paints_markers_and_submits_the_exact_stored_bytes() {
    let stored = "Trail\nmar\u{2028}ker\r\n\u{2029}";
    let mut ui = field(stored);
    let snapshot = ui.snapshot();
    let shown = format!(
        "Trail{LINE_BREAK_MARKER}mar{LINE_BREAK_MARKER}ker{LINE_BREAK_MARKER}{PARAGRAPH_MARKER}"
    );
    assert_eq!(snapshot.composer.display_text, shown);
    assert_eq!(snapshot.composer.markers.len(), 4);
    let viewport = Viewport::new(400.0, 300.0, 1.0).unwrap();
    let painted = layout(&snapshot, viewport)
        .unwrap()
        .paint
        .into_iter()
        .any(|paint| matches!(paint, Paint::Text { widget, text, .. } if widget.id == COMPOSER && text == shown));
    assert!(painted, "the composer paints the marked display text");
    // Untouched: the action carries the stored bytes, markers never.
    assert_eq!(submitted(&mut ui).as_bytes(), stored.as_bytes());
    // Editing elsewhere keeps every stored break byte for byte.
    let target = target(&ui);
    ui.apply(Input::Selection {
        target,
        selection: TextSelection::caret(TextCursor::new(0)),
    })
    .unwrap();
    let target = self::target(&ui);
    ui.apply(Input::TextCommit {
        target,
        text: "<".into(),
    })
    .unwrap();
    assert_eq!(submitted(&mut ui), format!("<{stored}"));
    // Guarded values: the zero-width guard is display only, never stored.
    for stored in ["a\n\u{301}", "\u{600}\n", "\u{600}\r\n\u{301}"] {
        let mut ui = field(stored);
        let snapshot = ui.snapshot().composer;
        assert!(snapshot.display_text.contains('\u{200b}'), "{stored:?}");
        assert_eq!(snapshot.committed, stored);
        assert_eq!(submitted(&mut ui).as_bytes(), stored.as_bytes());
    }
}

#[test]
fn opening_and_binding_obey_only_the_byte_budget() {
    let value = "\n".repeat(8);
    assert!(TextInput::new(value.clone(), 8).is_ok());
    assert_eq!(
        TextInput::new(value.clone(), 7),
        Err(UiError::LimitExceeded)
    );
    let mut ui = Ui::with_composition(
        Limits {
            composer_bytes: 8,
            ..Limits::default()
        },
        Composition::SingleField,
    )
    .unwrap();
    ui.bind_field(value.clone()).unwrap();
    // The display is longer than the budget; the budget counts stored bytes.
    let snapshot = ui.snapshot();
    assert_eq!(
        snapshot.composer.display_text.len(),
        8 * LINE_BREAK_MARKER.len_utf8()
    );
    let before = ui.snapshot();
    assert_eq!(
        ui.bind_field(format!("{value}x")),
        Err(UiError::LimitExceeded)
    );
    assert_eq!(ui.snapshot(), before);
    // A full stored value still accepts a same-size replacement.
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    let target = target(&ui);
    ui.apply(Input::Selection {
        target,
        selection: TextSelection {
            anchor: TextCursor::new(0),
            focus: TextCursor::new(1),
        },
    })
    .unwrap();
    let target = self::target(&ui);
    assert_eq!(
        ui.apply(Input::TextCommit {
            target,
            text: "ab".into()
        }),
        Err(UiError::LimitExceeded)
    );
    ui.apply(Input::TextCommit {
        target,
        text: "a".into(),
    })
    .unwrap();
    assert_eq!(
        ui.snapshot().composer.committed,
        format!("a{}", "\n".repeat(7))
    );
}

/// Pieces that exercise every grapheme rule a marker could disturb: stored
/// breaks and controls, prepended, combining, spacing and joining marks,
/// Hangul jamo, an Indic conjunct, emoji modifiers and joiners, a regional
/// indicator, a stored zero-width space and the marker glyphs as plain text.
const PARTS: [&str; 30] = [
    "\n",
    "\r\n",
    "\r",
    "\u{2028}",
    "\u{2029}",
    "\t",
    "\u{85}",
    "\u{1}",
    "\u{9f}",
    "a",
    "\u{301}",
    "\u{600}",
    "\u{110bd}",
    "\u{94d}",
    "\u{903}",
    "\u{915}\u{94d}",
    "\u{1100}",
    "\u{1161}",
    "\u{ac00}",
    "👍",
    "\u{1f3fd}",
    "\u{fe0f}",
    "\u{200d}",
    "🇺",
    "\u{200b}",
    "⏎",
    "¶",
    "→",
    "⬚",
    "e\u{301}",
];

/// A tiny deterministic generator, so every run checks the same sample.
struct Seeded(u64);
impl Seeded {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap() % bound
    }
    fn value(&mut self, pieces: usize) -> String {
        (0..pieces)
            .map(|_| PARTS[self.below(PARTS.len())])
            .collect()
    }
}

/// Each committed caret is shown on a display grapheme boundary and maps
/// back exactly; the only other display boundaries lie inside a marker unit
/// and map to the stored character's edge on the glyph's side.
fn check_caret_round_trip(value: &str) {
    let input = open(value);
    let snapshot = input.snapshot();
    let display = boundaries(&snapshot.display_text);
    let mut mapped = BTreeSet::new();
    for byte in boundaries(value) {
        let mut probe = input.clone();
        caret(&mut probe, byte);
        let shown = probe.snapshot().display_selection.focus.byte;
        assert!(display.contains(&shown), "{value:?} {byte} -> {shown}");
        assert_eq!(committed(&snapshot, shown), Ok(byte), "{value:?}");
        mapped.insert(shown);
    }
    for &byte in display.difference(&mapped) {
        let unit = snapshot
            .markers
            .iter()
            .find(|unit| unit.display.start < byte && byte < unit.display.end)
            .unwrap_or_else(|| panic!("{value:?}: stray boundary {byte}"));
        let edge = if byte <= unit.glyph.start {
            unit.source.start
        } else {
            unit.source.end
        };
        assert_eq!(committed(&snapshot, byte), Ok(edge), "{value:?}");
    }
    for unit in &snapshot.markers {
        assert!(display.contains(&unit.glyph.start), "{value:?}");
        assert!(display.contains(&unit.glyph.end), "{value:?}");
        assert!(refuses_single_line(
            value[unit.source.clone()].chars().next().unwrap()
        ));
    }
}

#[test]
fn display_boundaries_match_committed_boundaries_exactly() {
    let mut values = vec![String::new()];
    for _ in 0..3 {
        values = values
            .iter()
            .flat_map(|value| PARTS.iter().map(move |part| format!("{value}{part}")))
            .collect();
        for value in &values {
            check_caret_round_trip(value);
        }
    }
    let mut random = Seeded(0x5eed);
    for _ in 0..5_000 {
        let pieces = 4 + random.below(5);
        check_caret_round_trip(&random.value(pieces));
    }
}

#[test]
fn a_guard_appears_only_beside_a_joining_neighbour() {
    let (shown, markers) = mark_stored_breaks("\n\u{301}");
    assert_eq!(shown, format!("{LINE_BREAK_MARKER}\u{200b}\u{301}"));
    assert_eq!(
        (markers[0].display.clone(), markers[0].glyph.clone()),
        (0..6, 0..3)
    );
    let (shown, markers) = mark_stored_breaks("\u{600}\n");
    assert_eq!(shown, format!("\u{600}\u{200b}{LINE_BREAK_MARKER}"));
    assert_eq!(
        (markers[0].display.clone(), markers[0].glyph.clone()),
        (2..8, 5..8)
    );
}

#[test]
fn backspace_and_delete_beside_a_marker_remove_exactly_one_stored_character() {
    for (stored, _) in CLASSES {
        let value = format!("Ab{stored}{stored}c");
        let second = 2 + stored.len();
        let expected = format!("Ab{stored}c");
        let mut input = open(&value);
        caret(&mut input, second + stored.len());
        input.backspace().unwrap();
        assert_eq!(input.text().as_bytes(), expected.as_bytes(), "{stored:?}");
        assert_eq!(
            input.selection(),
            TextSelection::caret(TextCursor::new(second))
        );
        let mut input = open(&value);
        caret(&mut input, 2);
        input.delete().unwrap();
        assert_eq!(input.text().as_bytes(), expected.as_bytes(), "{stored:?}");
        assert_eq!(input.snapshot().markers.len(), 1);
    }
    // Through the field's own keys too: one Backspace removes a CR LF pair.
    let mut ui = field("a\r\nb");
    let target = target(&ui);
    ui.apply(Input::Selection {
        target,
        selection: TextSelection::caret(TextCursor::new(3)),
    })
    .unwrap();
    ui.apply(Input::Key(KeyEvent {
        physical: 42,
        key: Key::Backspace,
        pressed: true,
        repeat: false,
        platform_composing: false,
    }))
    .unwrap();
    assert_eq!(ui.snapshot().composer.committed, "ab");
}

#[test]
fn a_selection_over_a_marker_covers_the_stored_characters() {
    let value = "A\r\nB\u{2028}C";
    let mut input = open(value);
    select(&mut input, 1, 7);
    let snapshot = input.snapshot();
    // What a host copies: committed bytes, never the markers.
    assert_eq!(
        &snapshot.committed[snapshot.selection.range()],
        "\r\nB\u{2028}"
    );
    let shown = snapshot.display_selection.range();
    assert_eq!(
        &snapshot.display_text[shown],
        format!("{LINE_BREAK_MARKER}B{LINE_BREAK_MARKER}")
    );
    // Replacing the selection removes both breaks.
    input.input("-").unwrap();
    assert_eq!(input.text(), "A-C");
    // A caret can never sit inside a CR LF pair.
    let mut input = open(value);
    let before = input.clone();
    let stale = input.revision();
    assert_eq!(
        input.set_selection(TextSelection::caret(TextCursor::new(2)), stale),
        Err(UiError::InvalidInput)
    );
    assert_eq!(input, before);
}

#[test]
fn preedit_beside_markers_maps_both_ways() {
    // Four markers before a preedit at a caret.
    let mut input = open("\n\n\n\nbc");
    caret(&mut input, 4);
    input.update_preedit("a", 1, 0).unwrap();
    let snapshot = input.snapshot();
    assert_eq!(
        snapshot.display_text,
        format!("{}abc", LINE_BREAK_MARKER.to_string().repeat(4))
    );
    assert_eq!(snapshot.composition.as_ref().unwrap().display_range, 12..13);
    assert_eq!(snapshot.display_selection.focus.byte, 13);
    for (shown, expected) in [(0, 0), (3, 1), (12, 4), (13, 4), (14, 5), (15, 6)] {
        assert_eq!(committed(&snapshot, shown), Ok(expected), "display {shown}");
    }
    // A replaced selection with markers before and after it.
    let mut input = open("x\ny\u{2029}z\tw");
    select(&mut input, 2, 6);
    input.update_preedit("QR", 2, 0).unwrap();
    let snapshot = input.snapshot();
    assert_eq!(
        snapshot.display_text,
        format!("x{LINE_BREAK_MARKER}QRz{TAB_MARKER}w")
    );
    for (shown, expected) in [(1, 1), (4, 2), (6, 6), (7, 7), (10, 8), (11, 9)] {
        assert_eq!(committed(&snapshot, shown), Ok(expected), "display {shown}");
    }
    assert_eq!(committed(&snapshot, 5), Err(UiError::InvalidInput));
    // A combining preedit right after a marker gets a guard, then commits.
    let mut input = open("A\nB");
    caret(&mut input, 2);
    input.update_preedit("\u{301}x", 2, 0).unwrap();
    let snapshot = input.snapshot();
    assert_eq!(
        snapshot.display_text,
        format!("A{LINE_BREAK_MARKER}\u{200b}\u{301}xB")
    );
    let range = snapshot.composition.as_ref().unwrap().display_range.clone();
    assert_eq!(&snapshot.display_text[range.clone()], "\u{301}x");
    assert_eq!(snapshot.display_selection.focus.byte, range.end);
    assert_eq!(committed(&snapshot, range.start), Ok(2));
    assert_eq!(committed(&snapshot, 1), Ok(1));
    input.input("\u{301}x").unwrap();
    assert_eq!(input.text(), "A\n\u{301}xB");
    assert!(input.snapshot().markers[0].display.len() > LINE_BREAK_MARKER.len_utf8());
}

/// The committed position of a display boundary outside the preedit,
/// computed independently of `committed_cursor`.
fn expected_committed(snapshot: &TextInputSnapshot, shown: usize) -> usize {
    let composition = snapshot.composition.as_ref().unwrap();
    let raw = snapshot
        .markers
        .iter()
        .rev()
        .find(|unit| unit.display.end <= shown)
        .map_or(shown, |unit| unit.source.end + (shown - unit.display.end));
    let start = composition.replacement.start;
    let preedit = start..start + composition.text.len();
    if raw <= preedit.start {
        raw
    } else {
        assert!(raw >= preedit.end);
        composition.replacement.end + (raw - preedit.end)
    }
}

#[test]
fn seeded_preedits_among_markers_map_back_exactly() {
    let mut random = Seeded(0xc0de);
    for _ in 0..3_000 {
        let pieces = 1 + random.below(6);
        let value = random.value(pieces);
        let edges: Vec<_> = boundaries(&value).into_iter().collect();
        let (a, b) = (
            edges[random.below(edges.len())],
            edges[random.below(edges.len())],
        );
        let mut input = open(&value);
        select(&mut input, a, b);
        let preedit = ["a", "青", "QR"][random.below(3)];
        input.update_preedit(preedit, 0, 0).unwrap();
        let snapshot = input.snapshot();
        let composition = snapshot.composition.as_ref().unwrap();
        let range = composition.display_range.clone();
        assert_eq!(&snapshot.display_text[range.clone()], preedit);
        for shown in boundaries(&snapshot.display_text) {
            let in_marker = snapshot
                .markers
                .iter()
                .any(|unit| unit.display.start < shown && shown < unit.display.end);
            if (range.start < shown && shown < range.end) || in_marker {
                continue;
            }
            let expected = if shown == range.start {
                composition.replacement.start
            } else {
                expected_committed(&snapshot, shown)
            };
            assert_eq!(
                committed(&snapshot, shown),
                Ok(expected),
                "{value:?} {a}..{b} {preedit:?} display {shown}"
            );
        }
    }
}

#[test]
fn typed_pasted_replaced_or_composed_breaks_and_controls_are_refused_whole() {
    const REFUSED: [&str; 9] = [
        "\n",
        "\r\n",
        "\r",
        "\t",
        "\u{2028}",
        "\u{2029}",
        "\u{85}",
        "x\u{1b}",
        "ok\u{2028}ok",
    ];
    let mut ui = field("A\nB");
    let target = target(&ui);
    ui.apply(Input::Selection {
        target,
        selection: TextSelection::caret(TextCursor::new(1)),
    })
    .unwrap();
    let before = ui.snapshot();
    for refused in REFUSED {
        let target = self::target(&ui);
        // A typed character or a host paste arrives as a commit.
        let commit = Input::TextCommit {
            target,
            text: refused.into(),
        };
        assert_eq!(ui.apply(commit), Err(UiError::RefusedText), "{refused:?}");
        // A browser-style whole-value replacement, even keeping the stored break.
        let replace = Input::ReplaceText {
            target,
            text: format!("A{refused}\nB"),
            selection: TextSelection::default(),
        };
        assert_eq!(ui.apply(replace), Err(UiError::RefusedText), "{refused:?}");
        let preedit = Input::Preedit {
            target,
            text: refused.into(),
            start: 0,
            length: 0,
        };
        assert_eq!(ui.apply(preedit), Err(UiError::RefusedText), "{refused:?}");
        assert_eq!(ui.snapshot(), before, "{refused:?}");
    }
    // An IME commit that carries one is refused and keeps the composition.
    let target = self::target(&ui);
    ui.apply(Input::Preedit {
        target,
        text: "é".into(),
        start: 1,
        length: 0,
    })
    .unwrap();
    let composing = ui.snapshot();
    for refused in REFUSED {
        let target = self::target(&ui);
        assert_eq!(
            ui.apply(Input::Preedit {
                target,
                text: format!("é{refused}"),
                start: 1,
                length: 0
            }),
            Err(UiError::RefusedText)
        );
        assert_eq!(
            ui.apply(Input::TextCommit {
                target,
                text: format!("é{refused}")
            }),
            Err(UiError::RefusedText)
        );
        assert_eq!(ui.snapshot(), composing, "{refused:?}");
    }
    let target = self::target(&ui);
    ui.apply(Input::TextCommit {
        target,
        text: "é".into(),
    })
    .unwrap();
    assert_eq!(ui.snapshot().composer.committed, "Aé\nB");
    // Marker glyphs typed as text are ordinary text, accepted and unmarked.
    let target = self::target(&ui);
    ui.apply(Input::TextCommit {
        target,
        text: LINE_BREAK_MARKER.into(),
    })
    .unwrap();
    let snapshot = ui.snapshot();
    assert_eq!(
        snapshot.composer.committed,
        format!("Aé{LINE_BREAK_MARKER}\nB")
    );
    assert_eq!(snapshot.composer.markers.len(), 1);
}

#[test]
fn chat_composers_refuse_separators_as_well() {
    let mut ui = Ui::new(Limits::default()).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    for separator in ["\u{2028}", "\u{2029}"] {
        let target = target(&ui);
        assert_eq!(
            ui.apply(Input::TextCommit {
                target,
                text: format!("a{separator}b")
            }),
            Err(UiError::RefusedText)
        );
    }
    assert_eq!(ui.snapshot().composer.committed, "");
}

#[test]
fn a_refused_edit_leaves_a_text_input_exactly_as_it_was() {
    let mut input = open("A\nB");
    caret(&mut input, 1);
    let before = input.clone();
    for refused in ["\n", "\t", "x\u{2028}", "\u{2029}"] {
        assert_eq!(input.input(refused), Err(UiError::RefusedText));
        assert_eq!(
            input.update_preedit(refused, 0, 0),
            Err(UiError::RefusedText)
        );
        assert_eq!(input, before, "{refused:?}");
    }
    input.update_preedit("é", 1, 0).unwrap();
    let composing = input.clone();
    // An IME commit with a refused character keeps the composition.
    assert_eq!(input.input("é\n"), Err(UiError::RefusedText));
    assert_eq!(input.update_preedit("é\n", 1, 0), Err(UiError::RefusedText));
    assert_eq!(input, composing);
}

#[test]
fn a_refused_character_wins_over_the_byte_budget() {
    let mut input = TextInput::new("abc".into(), 4).unwrap();
    let before = input.clone();
    assert_eq!(input.input("xy"), Err(UiError::LimitExceeded));
    assert_eq!(input.input("x\ny"), Err(UiError::RefusedText));
    assert_eq!(
        input.update_preedit("x\ny", 0, 0),
        Err(UiError::RefusedText)
    );
    assert_eq!(input, before);
    input.input("x").unwrap();
    assert_eq!(input.text(), "abcx");
}

#[test]
fn received_chat_messages_refuse_breaks_and_separators() {
    let mut ui = Ui::new(Limits {
        message_bytes: 8,
        ..Limits::default()
    })
    .unwrap();
    ui.apply(Input::ReceiveMessage("kept".into())).unwrap();
    let before = ui.snapshot();
    for refused in [
        "a\u{2028}b",
        "a\u{2029}",
        "a\nb",
        "\t",
        "long\u{2028}message",
    ] {
        assert_eq!(
            ui.apply(Input::ReceiveMessage(refused.into())),
            Err(UiError::RefusedText),
            "{refused:?}"
        );
        assert_eq!(ui.snapshot(), before);
    }
    assert_eq!(
        ui.apply(Input::ReceiveMessage("too long!".into())),
        Err(UiError::LimitExceeded)
    );
    assert_eq!(ui.snapshot().messages, ["kept"]);
}
