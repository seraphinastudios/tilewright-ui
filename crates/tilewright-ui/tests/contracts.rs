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
    physical_key(
        ui,
        key,
        match key {
            Key::Enter => 1,
            Key::Backspace => 2,
            Key::Delete => 3,
            Key::SelectAll => 4,
            Key::Escape => 5,
            Key::Other => 6,
        },
        pressed,
    )
}
fn physical_key(
    ui: &mut Ui,
    key: Key,
    physical: u16,
    pressed: bool,
) -> Result<Disposition, UiError> {
    ui.apply(Input::Key(KeyEvent {
        physical,
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
        key(&mut ui, Key::Other, true).unwrap(),
        Disposition::Unhandled
    );
    let before = ui.control().epoch;
    ui.apply(Input::Modal(true)).unwrap();
    ui.apply(Input::Modal(false)).unwrap();
    assert!(ui.control().epoch > before);
    assert!(ui.control().world_input_allowed);
    assert_eq!(
        key(&mut ui, Key::Other, false).unwrap(),
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
        physical: 2,
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

#[test]
fn repeated_down_keeps_cancelled_press_ownership_until_release() {
    for started_in_widget in [false, true] {
        let mut ui = ui();
        if !started_in_widget {
            ui.apply(Input::Focus(None)).unwrap();
        }
        let key_code = Key::Other;
        let expected = if started_in_widget {
            Disposition::Consumed
        } else {
            Disposition::Unhandled
        };
        assert_eq!(key(&mut ui, key_code, true).unwrap(), expected);
        if started_in_widget {
            ui.apply(Input::Focus(None)).unwrap();
        } else {
            ui.apply(Input::Modal(true)).unwrap();
            ui.apply(Input::Modal(false)).unwrap();
        }
        assert!(ui.control().world_input_allowed);
        assert_eq!(
            ui.apply(Input::Key(KeyEvent {
                physical: 6,
                key: key_code,
                pressed: true,
                repeat: true
            }))
            .unwrap(),
            Disposition::Consumed
        );
        assert_eq!(
            key(&mut ui, key_code, false).unwrap(),
            Disposition::Consumed
        );
        assert_eq!(
            key(&mut ui, key_code, true).unwrap(),
            Disposition::Unhandled
        );
    }
}

#[test]
fn unavailable_composer_and_button_cannot_focus_or_submit_through_keys() {
    for transition in [
        Input::Modal(true),
        Input::Suspend(true),
        Input::WindowFocused(false),
        Input::ComposerVisible(false),
    ] {
        let mut ui = ui();
        commit(&mut ui, "retained");
        ui.apply(Input::Focus(Some(SUBMIT))).unwrap();
        ui.apply(transition).unwrap();
        let before = ui.snapshot();
        for widget in [COMPOSER, SUBMIT] {
            assert_eq!(
                ui.apply(Input::Focus(Some(widget))),
                Err(UiError::InvalidInput)
            );
            assert_eq!(ui.snapshot(), before);
        }
        assert!(ui.apply(Input::Activate(SUBMIT)).is_err());
        let _ = key(&mut ui, Key::Enter, true).unwrap();
        assert!(ui.peek_actions().is_none());
        assert_eq!(ui.snapshot().composer.committed, "retained");
    }
}

#[test]
fn enabled_focused_button_submits_once_on_enter() {
    let mut ui = ui();
    commit(&mut ui, "hello");
    ui.apply(Input::Focus(Some(SUBMIT))).unwrap();
    assert_eq!(
        key(&mut ui, Key::Enter, true).unwrap(),
        Disposition::Consumed
    );
    assert_eq!(
        ui.peek_actions().unwrap().actions,
        vec![Action::SubmitChat {
            text: "hello".into()
        }]
    );
}

#[test]
fn rejected_submit_still_owns_repeats_and_release_after_focus_loss() {
    let mut ui = ui();
    commit(&mut ui, "first");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    commit(&mut ui, "second");
    let before = ui.snapshot();
    assert_eq!(key(&mut ui, Key::Enter, true), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    ui.apply(Input::Focus(None)).unwrap();
    assert_eq!(
        ui.apply(Input::Key(KeyEvent {
            physical: 1,
            key: Key::Enter,
            pressed: true,
            repeat: true
        }))
        .unwrap(),
        Disposition::Consumed
    );
    assert_eq!(
        key(&mut ui, Key::Enter, false).unwrap(),
        Disposition::Consumed
    );
    assert_eq!(
        key(&mut ui, Key::Enter, true).unwrap(),
        Disposition::Unhandled
    );
    assert_eq!(ui.snapshot().composer.committed, "second");
}

#[test]
fn more_than_256_presses_and_domain_endpoints_keep_ownership_after_blur() {
    let mut ui = ui();
    commit(&mut ui, "first");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    commit(&mut ui, "second");
    for physical in 10..267 {
        assert_eq!(
            physical_key(&mut ui, Key::Other, physical, true).unwrap(),
            Disposition::Consumed
        );
    }
    assert_eq!(
        physical_key(&mut ui, Key::Other, u16::MAX, true).unwrap(),
        Disposition::Consumed
    );
    let before = ui.snapshot();
    assert_eq!(key(&mut ui, Key::Enter, true), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    ui.apply(Input::Focus(None)).unwrap();
    for physical in [10, 266, u16::MAX, 1] {
        assert_eq!(
            ui.apply(Input::Key(KeyEvent {
                physical,
                key: Key::Other,
                pressed: true,
                repeat: true
            }))
            .unwrap(),
            Disposition::Consumed
        );
        assert_eq!(
            physical_key(&mut ui, Key::Other, physical, false).unwrap(),
            Disposition::Consumed
        );
        assert_eq!(
            physical_key(&mut ui, Key::Other, physical, true).unwrap(),
            Disposition::Unhandled
        );
    }
}

#[test]
fn simultaneous_return_and_keypad_enter_keep_distinct_ownership_and_fences() {
    let mut ui = ui();
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "候補".into(),
        start: 0,
        length: 2,
    })
    .unwrap();
    physical_key(&mut ui, Key::Enter, 13, true).unwrap();
    physical_key(&mut ui, Key::Enter, 88, true).unwrap();
    commit(&mut ui, "confirmed");
    physical_key(&mut ui, Key::Enter, 13, false).unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert!(ui.peek_actions().is_none());
    ui.apply(Input::Focus(None)).unwrap();
    assert_eq!(
        ui.apply(Input::Key(KeyEvent {
            physical: 88,
            key: Key::Enter,
            pressed: true,
            repeat: true
        }))
        .unwrap(),
        Disposition::Consumed
    );
    assert!(!ui.can_apply_key_effect(88));
    assert_eq!(
        physical_key(&mut ui, Key::Enter, 88, false).unwrap(),
        Disposition::Consumed
    );
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert!(ui.peek_actions().is_some());
}

#[test]
fn host_key_effects_require_current_owned_unquarantined_press() {
    let mut ui = ui();
    assert!(!ui.can_apply_key_effect(224));
    physical_key(&mut ui, Key::Other, 224, true).unwrap(); // Control.
    physical_key(&mut ui, Key::Other, 57, true).unwrap(); // Caps Lock.
    assert!(ui.can_apply_key_effect(224));
    assert!(ui.can_apply_key_effect(57));
    physical_key(&mut ui, Key::Other, 224, false).unwrap();
    assert!(!ui.can_apply_key_effect(224));
    assert!(ui.can_apply_key_effect(57));
    ui.apply(Input::Focus(None)).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    assert!(!ui.can_apply_key_effect(57));
    assert_eq!(
        ui.apply(Input::Key(KeyEvent {
            physical: 57,
            key: Key::Other,
            pressed: true,
            repeat: true
        }))
        .unwrap(),
        Disposition::Consumed
    );
    assert!(!ui.can_apply_key_effect(57));
    physical_key(&mut ui, Key::Other, 225, true).unwrap();
    assert!(ui.can_apply_key_effect(225));
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "x".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    assert!(!ui.can_apply_key_effect(225));
}

#[test]
fn rejected_key_action_cannot_authorize_host_effects() {
    let mut ui = ui();
    commit(&mut ui, "full");
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    commit(&mut ui, "next");
    assert_eq!(
        physical_key(&mut ui, Key::Enter, 13, true),
        Err(UiError::QueueFull)
    );
    assert!(!ui.can_apply_key_effect(13));
}

#[test]
fn explicit_ime_cancel_retires_session_preserving_focus_and_selection() {
    for escape in [false, true] {
        let mut ui = ui();
        commit(&mut ui, "before");
        let selection = ui.snapshot().composer.selection;
        let focus = ui.control().focused_widget;
        ui.apply(Input::Preedit {
            target: ui.text_target().unwrap(),
            text: "candidate".into(),
            start: 0,
            length: 1,
        })
        .unwrap();
        let retired = ui.text_target().unwrap();
        let epoch = ui.control().epoch;
        if escape {
            physical_key(&mut ui, Key::Escape, 9, true).unwrap();
        } else {
            ui.apply(Input::CancelComposition { target: retired })
                .unwrap();
        }
        assert!(ui.control().epoch > epoch);
        assert_eq!(ui.control().focused_widget, focus);
        assert_eq!(ui.snapshot().composer.selection, selection);
        assert_eq!(ui.snapshot().composer.committed, "before");
        assert_eq!(
            ui.apply(Input::TextCommit {
                target: retired,
                text: "late".into()
            }),
            Err(UiError::Stale)
        );
        ui.apply(Input::Preedit {
            target: ui.text_target().unwrap(),
            text: "fresh".into(),
            start: 0,
            length: 1,
        })
        .unwrap();
        assert_eq!(ui.snapshot().composer.composition.unwrap().text, "fresh");
    }
}

#[test]
fn natural_preedit_end_and_commit_do_not_retire_the_platform_session() {
    let mut ui = ui();
    let epoch = ui.control().epoch;
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "候補".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: String::new(),
        start: 0,
        length: 0,
    })
    .unwrap();
    assert_eq!(ui.control().epoch, epoch);
    commit(&mut ui, "confirmed");
    assert_eq!(ui.control().epoch, epoch);
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "next".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    ui.apply(Input::ReplaceText {
        target: ui.text_target().unwrap(),
        text: "whole".into(),
        selection: TextSelection::caret(TextCursor::new(5)),
    })
    .unwrap();
    assert_eq!(ui.control().epoch, epoch);
    ui.apply(Input::CancelComposition {
        target: ui.text_target().unwrap(),
    })
    .unwrap();
    assert_eq!(ui.control().epoch, epoch);
}

#[test]
fn explicit_ime_cancel_preserves_each_held_enter_fence() {
    let mut ui = ui();
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "candidate".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    physical_key(&mut ui, Key::Enter, 13, true).unwrap();
    physical_key(&mut ui, Key::Enter, 88, true).unwrap();
    ui.apply(Input::CancelComposition {
        target: ui.text_target().unwrap(),
    })
    .unwrap();
    commit(&mut ui, "fresh");
    physical_key(&mut ui, Key::Enter, 13, false).unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert!(ui.peek_actions().is_none());
    physical_key(&mut ui, Key::Enter, 88, false).unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    assert!(ui.peek_actions().is_some());
}
