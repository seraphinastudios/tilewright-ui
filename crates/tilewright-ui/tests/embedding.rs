use tilewright_ui::*;

fn field() -> Ui {
    Ui::with_composition(
        Limits {
            message_bytes: 4,
            queued_batches: 1,
            ..Limits::default()
        },
        Composition::SingleField,
    )
    .unwrap()
}
fn enter(ui: &mut Ui, pressed: bool) -> Result<Disposition, UiError> {
    ui.apply(Input::Key(KeyEvent {
        physical: 1,
        key: Key::Enter,
        pressed,
        repeat: false,
        platform_composing: false,
    }))
}

#[test]
fn field_enter_and_apply_capture_empty_or_full_draft_without_editing() {
    for text in ["", "longer than a chat message 👩‍🚀"] {
        let mut enter_ui = field();
        enter_ui.bind_field(text).unwrap();
        enter_ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
        if !text.is_empty() {
            enter_ui
                .apply(Input::Selection {
                    target: enter_ui.text_target().unwrap(),
                    selection: TextSelection::caret(TextCursor::new(0)),
                })
                .unwrap();
        }
        let mut apply_ui = enter_ui.clone();
        let before = enter_ui.snapshot();
        enter(&mut enter_ui, true).unwrap();
        apply_ui.apply(Input::Activate(SUBMIT)).unwrap();
        assert_eq!(enter_ui.snapshot(), before);
        assert_eq!(apply_ui.snapshot(), before);
        assert_eq!(enter_ui.peek_actions(), apply_ui.peek_actions());
        assert_eq!(
            enter_ui.peek_actions().unwrap().actions,
            vec![Action::SubmitField {
                widget: before.composer_widget,
                revision: before.composer.revision,
                text: text.into(),
            }]
        );
    }
}

#[test]
fn field_queue_full_preserves_state_and_quarantines_the_triggering_key() {
    let mut ui = field();
    ui.bind_field("draft").unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    let batch = ui.peek_actions().unwrap().clone();
    let before = ui.snapshot();
    assert_eq!(enter(&mut ui, true), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    assert_eq!(ui.apply(Input::Activate(SUBMIT)), Err(UiError::QueueFull));
    assert_eq!(ui.snapshot(), before);
    assert_eq!(ui.peek_actions(), Some(&batch));
    ui.ack_actions(batch.id).unwrap();
    ui.apply(Input::Focus(None)).unwrap();
    assert_eq!(enter(&mut ui, false), Ok(Disposition::Consumed));
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    enter(&mut ui, true).unwrap();
    assert!(ui.peek_actions().unwrap().id > batch.id);
}

#[test]
fn rebind_advances_all_identities_and_preserves_pending_action() {
    let mut ui = field();
    ui.bind_field("first").unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    let old_target = ui.text_target().unwrap();
    ui.apply(Input::Activate(SUBMIT)).unwrap();
    let old_action = ui.peek_actions().cloned();
    let before = ui.snapshot();
    assert_eq!(ui.bind_field("second"), Err(UiError::Stale));
    assert_eq!(ui.snapshot(), before);
    ui.apply(Input::Preedit {
        target: old_target,
        text: "候補".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    ui.apply(Input::Focus(None)).unwrap();
    let before = ui.snapshot();
    ui.bind_field("first").unwrap();
    let after = ui.snapshot();
    assert!(after.composer_widget.generation > before.composer_widget.generation);
    assert!(after.composer.revision > before.composer.revision);
    assert!(after.revision > before.revision);
    assert!(after.control.epoch > before.control.epoch);
    assert!(after.composer.composition.is_none());
    assert_eq!(after.composer.selection.focus.byte, "first".len());
    assert_eq!(ui.peek_actions().cloned(), old_action);
    let after_bind = after.clone();
    assert_eq!(ui.bind_field("bad\ntext"), Err(UiError::LimitExceeded));
    assert_eq!(ui.snapshot(), after_bind);
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    assert_eq!(
        ui.apply(Input::TextCommit {
            target: old_target,
            text: "late".into()
        }),
        Err(UiError::Stale)
    );
    ui.apply(Input::Focus(None)).unwrap();
    ui.bind_field("first").unwrap();
    assert!(ui.composer_widget().generation > after.composer_widget.generation);
    assert!(ui.snapshot().composer.revision > after.composer.revision);
}

#[test]
fn embedded_control_only_inhibits_world_when_its_focus_or_modal_requires_it() {
    for composition in [Composition::ChatPanel, Composition::SingleField] {
        let mut ui = Ui::with_composition(Limits::default(), composition).unwrap();
        assert!(ui.control().world_input_allowed);
        ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
        assert!(!ui.control().world_input_allowed);
        ui.apply(Input::Focus(None)).unwrap();
        assert!(ui.control().world_input_allowed);
        let epoch = ui.control().epoch;
        ui.apply(Input::Modal(true)).unwrap();
        assert!(!ui.control().world_input_allowed);
        ui.apply(Input::Modal(false)).unwrap();
        assert!(ui.control().world_input_allowed);
        assert!(ui.control().epoch > epoch);
    }
}

#[test]
fn embedded_layouts_are_bounded_and_have_no_world_or_host_modal_output() {
    let viewport = Viewport::new(800.0, 600.0, 1.5).unwrap();
    let bounds = Rect {
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 150.0,
    };
    let clip = Rect {
        width: 200.0,
        ..bounds
    };
    for composition in [Composition::ChatPanel, Composition::SingleField] {
        let mut ui = Ui::with_composition(Limits::default(), composition).unwrap();
        if composition == Composition::ChatPanel {
            ui.apply(Input::ReceiveMessage("history".into())).unwrap();
        } else {
            assert_eq!(
                ui.apply(Input::ReceiveMessage("history".into())),
                Err(UiError::InvalidInput)
            );
            assert_eq!(ui.apply(Input::Scroll(1)), Err(UiError::InvalidInput));
        }
        let layout = layout_in(&ui.snapshot(), viewport, bounds, clip).unwrap();
        assert!(!layout.hits.iter().any(|h| h.widget.id == WORLD));
        for paint in &layout.paint {
            let paint_clip = match paint {
                Paint::Rectangle { clip, .. } | Paint::Text { clip, .. } => *clip,
                Paint::World { .. } => panic!("embedded world output"),
            };
            assert_eq!(paint_clip.intersection(clip).unwrap(), paint_clip);
            if let Paint::Text { widget, text, .. } = paint {
                assert_ne!(widget.id, MODAL);
                if composition == Composition::SingleField {
                    assert_ne!(widget.id, HISTORY);
                    if widget.id == SUBMIT {
                        assert_eq!(text, "Apply");
                    }
                }
            }
        }
        ui.apply(Input::Modal(true)).unwrap();
        let modal = layout_in(&ui.snapshot(), viewport, bounds, clip).unwrap();
        assert_eq!(modal.paint, layout.paint);
        assert!(modal.hits.is_empty());
    }
}
