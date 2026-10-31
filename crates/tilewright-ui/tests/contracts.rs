use tilewright_ui::*;
fn ui() -> Ui {
    let mut ui = Ui::new(Limits {
        queued_batches: 1,
        ..Limits::default()
    })
    .unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui
}
fn commit(ui: &mut Ui, text: &str) {
    ui.apply(Input::TextCommit {
        target: ui.text_target().unwrap(),
        text: text.into(),
    })
    .unwrap();
}
fn key(ui: &mut Ui, key: Key, pressed: bool) -> Result<Disposition, UiError> {
    ui.apply(Input::Key(KeyEvent {
        key,
        pressed,
        repeat: false,
    }))
}
#[test]
fn full_queue_preserves_entire_composer_and_peek_ack_is_stable() {
    let mut ui = ui();
    commit(&mut ui, "first");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    let first = ui.peek_actions().unwrap().clone();
    assert_eq!(ui.peek_actions(), Some(&first));
    commit(&mut ui, "second");
    let before = ui.snapshot();
    assert_eq!(key(&mut ui, Key::Enter, true), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    assert_eq!(ui.apply(Input::Activate(SUBMIT)), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    assert_eq!(
        ui.ack_actions(BatchId::new(first.id.get() + 1).unwrap()),
        Err(UiError::Stale)
    );
    ui.ack_actions(first.id).unwrap();
    ui.ack_actions(first.id).unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert_eq!(
        ui.peek_actions().unwrap().actions,
        vec![Action::SubmitChat {
            text: "second".into()
        }]
    );
}
#[test]
fn cancellation_survives_full_queue_and_roundtrip_modal() {
    let mut ui = ui();
    commit(&mut ui, "full");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    ui.apply(Input::Focus(None)).unwrap();
    assert_eq!(
        key(&mut ui, Key::Other(12), true).unwrap(),
        Disposition::Unhandled
    );
    let before = ui.control().epoch;
    ui.apply(Input::Modal(true)).unwrap();
    ui.apply(Input::Modal(false)).unwrap();
    assert!(ui.control().epoch > before);
    assert!(ui.control().world_input_allowed);
    assert_eq!(
        key(&mut ui, Key::Other(12), false).unwrap(),
        Disposition::Consumed
    );
    assert!(ui.peek_actions().is_some());
}
#[test]
fn cancelled_world_press_cannot_repeat_into_focused_widget() {
    let mut ui = ui();
    commit(&mut ui, "abc");
    ui.apply(Input::Focus(None)).unwrap();
    key(&mut ui, Key::Backspace, true).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui.apply(Input::Key(KeyEvent {
        key: Key::Backspace,
        pressed: true,
        repeat: true,
    }))
    .unwrap();
    assert_eq!(ui.snapshot().composer.committed, "abc");
    assert_eq!(
        key(&mut ui, Key::Backspace, false).unwrap(),
        Disposition::Consumed
    );
}
#[test]
fn stale_focus_tokens_and_closed_widget_generation_are_rejected() {
    let mut ui = ui();
    let old = ui.text_target().unwrap();
    ui.apply(Input::Focus(None)).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    assert_eq!(
        ui.apply(Input::TextCommit {
            target: old,
            text: "late".into()
        }),
        Err(UiError::Stale)
    );
    let generation = ui.composer_widget().generation;
    ui.apply(Input::ComposerVisible(false)).unwrap();
    ui.apply(Input::ComposerVisible(true)).unwrap();
    assert!(ui.composer_widget().generation > generation);
}
#[test]
fn enter_composition_commit_does_not_submit_until_release_and_next_press() {
    let mut ui = ui();
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "候補".into(),
        start: 0,
        length: 2,
    })
    .unwrap();
    key(&mut ui, Key::Enter, true).unwrap();
    commit(&mut ui, "確定");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert!(ui.peek_actions().is_none());
    key(&mut ui, Key::Enter, false).unwrap();
    key(&mut ui, Key::Enter, true).unwrap();
    assert!(ui.peek_actions().is_some());
}
#[test]
fn exact_utf8_joined_graphemes_and_atomic_invalid_edits() {
    let mut ui = ui();
    commit(&mut ui, "a");
    commit(&mut ui, "\u{301}");
    assert_eq!(ui.snapshot().composer.committed, "a\u{301}");
    let before = ui.snapshot();
    let bad = TextSelection::caret(TextCursor::new(1));
    assert_eq!(
        ui.apply(Input::Selection {
            target: ui.text_target().unwrap(),
            selection: bad
        }),
        Err(UiError::InvalidInput)
    );
    assert_eq!(ui.snapshot(), before);
    key(&mut ui, Key::Backspace, true).unwrap();
    assert_eq!(ui.snapshot().composer.committed, "");
    key(&mut ui, Key::Backspace, false).unwrap();
    commit(&mut ui, "👩‍👩‍👧‍👦אבג");
    assert!(ui.snapshot().composer.committed.starts_with("👩‍👩‍👧‍👦"));
}
#[test]
fn preedit_is_transient_and_validates_offsets_before_cancellation() {
    let mut ui = ui();
    commit(&mut ui, "base");
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "😀".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    let before = ui.snapshot();
    assert_eq!(before.composer.committed, "base");
    assert_eq!(before.composer.display_text, "base😀");
    assert_eq!(
        ui.apply(Input::Preedit {
            target: ui.text_target().unwrap(),
            text: String::new(),
            start: 2,
            length: 0
        }),
        Err(UiError::InvalidInput)
    );
    assert_eq!(ui.snapshot(), before);
    ui.apply(Input::CancelComposition {
        target: ui.text_target().unwrap(),
    })
    .unwrap();
    assert_eq!(ui.snapshot().composer.display_text, "base");
}
#[test]
fn replacement_and_retention_are_atomic_and_bounded() {
    let mut ui = Ui::new(Limits {
        message_bytes: 4,
        history_bytes: 5,
        history_messages: 2,
        composer_bytes: 4,
        ..Limits::default()
    })
    .unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    commit(&mut ui, "ok");
    let before = ui.snapshot();
    assert_eq!(
        ui.apply(Input::ReplaceText {
            target: ui.text_target().unwrap(),
            text: "valid".into(),
            selection: TextSelection::default()
        }),
        Err(UiError::LimitExceeded)
    );
    assert_eq!(ui.snapshot(), before);
    for text in ["ab", "cd", "efg"] {
        ui.apply(Input::ReceiveMessage(text.into())).unwrap();
    }
    assert_eq!(ui.snapshot().messages, vec!["cd", "efg"]);
    let before = ui.snapshot();
    assert_eq!(
        ui.apply(Input::ReceiveMessage("long!".into())),
        Err(UiError::LimitExceeded)
    );
    assert_eq!(ui.snapshot(), before);
}

#[test]
fn native_whole_value_composition_commit_is_atomic() {
    let mut ui = ui();
    commit(&mut ui, "before");
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "候補".into(),
        start: 0,
        length: 2,
    })
    .unwrap();
    let before = ui.snapshot();
    assert_eq!(
        ui.apply(Input::ReplaceText {
            target: ui.text_target().unwrap(),
            text: "done".into(),
            selection: TextSelection::caret(TextCursor::new(99))
        }),
        Err(UiError::InvalidInput)
    );
    assert_eq!(ui.snapshot(), before);
    ui.apply(Input::ReplaceText {
        target: ui.text_target().unwrap(),
        text: "done".into(),
        selection: TextSelection::caret(TextCursor::new(4)),
    })
    .unwrap();
    assert_eq!(ui.snapshot().composer.committed, "done");
    assert!(ui.snapshot().composer.composition.is_none());
}
