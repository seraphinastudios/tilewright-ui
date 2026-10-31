// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
//! SDL conversion only; all editing and key ownership remain in shared Ui.
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod},
    mouse::MouseButton,
};
use tilewright_app::App;
use tilewright_ui::{COMPOSER, HISTORY, Input, Key, KeyEvent, SUBMIT, WORLD};

#[derive(Default)]
pub struct HostInput {
    epoch: u64,
    session_start: u64,
    pub text_active: bool,
    pub last_consumed: bool,
    batch_ime_activity: bool,
    pub world_held: std::collections::BTreeSet<u16>,
    pressed: std::collections::BTreeMap<u16, Key>,
}
impl HostInput {
    /// Inspect the bounded drain before dispatch, as in the MIT editor's
    /// submission fence. A full drain conservatively blocks Enter as well.
    pub fn begin_batch(&mut self, app: &App, events: &[Event], window_id: u32) {
        self.batch_ime_activity = app.ui().snapshot().composer.composition.is_some()
            || events.len() == 1024
            || events.iter().any(|event| {
                matches!(event,
                Event::TextEditing { window_id: id, .. } | Event::TextInput { window_id: id, .. }
                if *id == window_id)
            });
    }
    /// Every control epoch is a cancellation barrier, even if final allowed=true.
    pub fn synchronize(&mut self, app: &App, now_ns: u64) -> bool {
        let control = app.ui().control();
        let changed = self.epoch != control.epoch.get();
        if changed {
            self.epoch = control.epoch.get();
            self.world_held.clear();
            self.session_start = now_ns;
        }
        self.text_active = app.ui().text_target().is_some();
        changed
    }
    // Keep the SDL event mapping visible in one exhaustive dispatch. Wheel
    // deltas deliberately truncate fractional ticks and saturate at i32 bounds.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn event(
        &mut self,
        app: &mut App,
        event: &Event,
        window_id: u32,
    ) -> Result<bool, tilewright_app::AppError> {
        self.last_consumed = true;
        match event {
            Event::Quit { .. } => return Ok(false),
            Event::Window {
                window_id: id,
                win_event,
                ..
            } if *id == window_id => match win_event {
                WindowEvent::CloseRequested => return Ok(false),
                WindowEvent::FocusLost => {
                    app.apply(Input::WindowFocused(false))?;
                }
                WindowEvent::FocusGained => {
                    app.apply(Input::WindowFocused(true))?;
                }
                WindowEvent::Minimized => {
                    app.apply(Input::Suspend(true))?;
                    app.invalidate_surface()?;
                }
                WindowEvent::Restored => {
                    app.apply(Input::Suspend(false))?;
                    app.invalidate_surface()?;
                }
                WindowEvent::Resized(..) | WindowEvent::PixelSizeChanged(..) => {
                    app.invalidate_surface()?;
                }
                _ => (),
            },
            Event::TextInput {
                window_id: id,
                timestamp,
                text,
            } if *id == window_id && *timestamp >= self.session_start && self.text_active => {
                if let Some(target) = app.ui().text_target() {
                    app.apply(Input::TextCommit {
                        target,
                        text: text.clone(),
                    })?;
                }
            }
            Event::TextEditing {
                window_id: id,
                timestamp,
                text,
                start,
                length,
            } if *id == window_id && *timestamp >= self.session_start && self.text_active => {
                // SDL start/length count UTF-8 scalar characters. Shared UI checks
                // and converts these to byte offsets; -1 means unknown.
                if let Some(target) = app.ui().text_target() {
                    app.apply(Input::Preedit {
                        target,
                        text: text.clone(),
                        start: *start,
                        length: *length,
                    })?;
                }
            }
            Event::KeyDown {
                window_id: id,
                keycode,
                scancode,
                keymod,
                repeat,
                ..
            } if *id == window_id => {
                // Tab and F1 are explicit host focus/modal commands. Send the
                // physical key first so its release remains UI-owned/quarantined.
                let Some(code) = physical(*scancode) else {
                    return Ok(true);
                };
                let new_press = !self.pressed.contains_key(&code);
                let key = *self
                    .pressed
                    .entry(code)
                    .or_insert_with(|| semantic_key(*keycode, *keymod));
                let disposition = app.apply(Input::Key(KeyEvent {
                    key,
                    physical: code,
                    platform_composing: key == Key::Enter && self.batch_ime_activity,
                    pressed: true,
                    repeat: *repeat,
                }))?;
                self.last_consumed = disposition == tilewright_ui::Disposition::Consumed;
                if new_press && !repeat {
                    if *keycode == Some(Keycode::Tab) && !app.ui().control().modal_open {
                        app.apply(Input::Focus(if app.ui().text_target().is_some() {
                            None
                        } else {
                            Some(COMPOSER)
                        }))?;
                    }
                    if *keycode == Some(Keycode::F1) {
                        app.apply(Input::Modal(!app.ui().control().modal_open))?;
                    }
                }
                if disposition == tilewright_ui::Disposition::Unhandled
                    && app.ui().control().world_input_allowed
                {
                    self.world_held.insert(code);
                }
            }
            Event::KeyUp {
                window_id: id,
                keycode,
                scancode,
                keymod,
                ..
            } if *id == window_id => {
                let Some(code) = physical(*scancode) else {
                    return Ok(true);
                };
                let disposition = app.apply(Input::Key(KeyEvent {
                    physical: code,
                    platform_composing: false,
                    key: self
                        .pressed
                        .remove(&code)
                        .unwrap_or_else(|| semantic_key(*keycode, *keymod)),
                    pressed: false,
                    repeat: false,
                }))?;
                self.last_consumed = disposition == tilewright_ui::Disposition::Consumed;
                self.world_held.remove(&code);
            }
            Event::MouseButtonDown {
                window_id: id,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if *id == window_id => {
                if let Some(stamp) = app.displayed().map(|frame| frame.stamp)
                    && let Ok(Some(widget)) = app.hit_test(stamp, *x, *y)
                {
                    match widget.id {
                        COMPOSER => {
                            app.apply(Input::Focus(Some(COMPOSER)))?;
                        }
                        SUBMIT => {
                            app.apply(Input::Activate(SUBMIT))?;
                        }
                        WORLD => {
                            app.apply(Input::Focus(None))?;
                            self.last_consumed = !app.ui().control().world_input_allowed;
                        }
                        _ => (),
                    }
                }
            }
            Event::MouseWheel {
                window_id: id,
                y,
                mouse_x,
                mouse_y,
                ..
            } if *id == window_id => {
                if let Some(stamp) = app.displayed().map(|frame| frame.stamp)
                    && let Ok(Some(widget)) = app.hit_test(stamp, *mouse_x, *mouse_y)
                    && widget.id == HISTORY
                {
                    app.apply(Input::Scroll(*y as i32))?;
                }
            }
            _ => (),
        }
        Ok(true)
    }
}
pub fn physical(scancode: Option<sdl3::keyboard::Scancode>) -> Option<u16> {
    scancode
        .filter(|code| *code != sdl3::keyboard::Scancode::Unknown)
        .map(|code| code as u16)
}
fn semantic_key(key: Option<Keycode>, modifiers: Mod) -> Key {
    match key {
        Some(Keycode::Return | Keycode::KpEnter) => Key::Enter,
        Some(Keycode::Backspace) => Key::Backspace,
        Some(Keycode::Delete) => Key::Delete,
        Some(Keycode::Escape) => Key::Escape,
        Some(Keycode::A) if modifiers.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => Key::SelectAll,
        _ => Key::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilewright_ui::{Limits, TextSelection};

    fn key(
        code: Keycode,
        scancode: sdl3::keyboard::Scancode,
        modifiers: Mod,
        pressed: bool,
    ) -> Event {
        if pressed {
            Event::KeyDown {
                timestamp: 10,
                window_id: 1,
                keycode: Some(code),
                scancode: Some(scancode),
                keymod: modifiers,
                repeat: false,
                which: 0,
                raw: 0,
            }
        } else {
            Event::KeyUp {
                timestamp: 11,
                window_id: 1,
                keycode: Some(code),
                scancode: Some(scancode),
                keymod: modifiers,
                repeat: false,
                which: 0,
                raw: 0,
            }
        }
    }
    #[test]
    fn stale_window_and_session_text_never_edits_current_composer() {
        let mut app = App::new(Limits::default()).unwrap();
        app.apply(Input::Focus(Some(COMPOSER))).unwrap();
        let mut host = HostInput::default();
        host.synchronize(&app, 100);
        for (window_id, timestamp) in [(2, 101), (1, 99)] {
            host.event(
                &mut app,
                &Event::TextInput {
                    window_id,
                    timestamp,
                    text: "stale".into(),
                },
                1,
            )
            .unwrap();
        }
        assert_eq!(app.ui().snapshot().composer.committed, "");
        host.event(
            &mut app,
            &Event::TextInput {
                window_id: 1,
                timestamp: 101,
                text: "accepted 🦀".into(),
            },
            1,
        )
        .unwrap();
        assert_eq!(app.ui().snapshot().composer.committed, "accepted 🦀");
    }
    #[test]
    fn modifier_release_preserves_the_original_press_identity() {
        let mut app = App::new(Limits::default()).unwrap();
        app.apply(Input::Focus(Some(COMPOSER))).unwrap();
        let mut host = HostInput::default();
        host.synchronize(&app, 0);
        host.event(
            &mut app,
            &key(Keycode::A, sdl3::keyboard::Scancode::A, Mod::LCTRLMOD, true),
            1,
        )
        .unwrap();
        host.event(
            &mut app,
            &key(Keycode::A, sdl3::keyboard::Scancode::A, Mod::NOMOD, false),
            1,
        )
        .unwrap();
        assert!(host.pressed.is_empty());
        let target = app.ui().text_target().unwrap();
        app.apply(Input::Selection {
            target,
            selection: TextSelection::default(),
        })
        .unwrap();
    }
    #[test]
    fn epoch_cancels_world_even_when_final_permission_is_restored() {
        let mut app = App::new(Limits::default()).unwrap();
        let mut host = HostInput::default();
        host.synchronize(&app, 0);
        host.event(
            &mut app,
            &key(Keycode::W, sdl3::keyboard::Scancode::W, Mod::NOMOD, true),
            1,
        )
        .unwrap();
        assert!(!host.world_held.is_empty());
        app.apply(Input::Modal(true)).unwrap();
        app.apply(Input::Modal(false)).unwrap();
        assert!(app.ui().control().world_input_allowed);
        host.synchronize(&app, 20);
        assert!(host.world_held.is_empty());
        host.event(
            &mut app,
            &key(Keycode::W, sdl3::keyboard::Scancode::W, Mod::NOMOD, false),
            1,
        )
        .unwrap();
        assert!(host.world_held.is_empty());
    }
    #[test]
    fn panel_background_is_not_a_world_click_and_wheel_uses_history_target() {
        let mut app = App::new(Limits::default()).unwrap();
        app.apply(Input::Focus(Some(COMPOSER))).unwrap();
        for message in ["one", "two", "three"] {
            app.apply(Input::ReceiveMessage(message.into())).unwrap();
        }
        let frame = app
            .prepare(tilewright_ui::Viewport::new(800.0, 600.0, 1.0).unwrap())
            .unwrap();
        app.ack_presented(frame.stamp).unwrap();
        let mut host = HostInput::default();
        host.synchronize(&app, 0);
        let click = Event::MouseButtonDown {
            timestamp: 1,
            window_id: 1,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 10.0,
            y: 350.0,
        };
        host.event(&mut app, &click, 1).unwrap();
        assert!(app.ui().text_target().is_some());
        assert!(host.world_held.is_empty());
        let wheel = |mouse_y| Event::MouseWheel {
            timestamp: 2,
            window_id: 1,
            which: 0,
            x: 0.0,
            y: 1.0,
            direction: sdl3::mouse::MouseWheelDirection::Normal,
            mouse_x: 10.0,
            mouse_y,
            integer_x: 0,
            integer_y: 1,
        };
        host.event(&mut app, &wheel(100.0), 1).unwrap();
        assert_eq!(app.ui().snapshot().scroll, 0);
        host.event(&mut app, &wheel(350.0), 1).unwrap();
        assert_eq!(app.ui().snapshot().scroll, 1);
    }
    #[test]
    fn return_and_keypad_enter_have_independent_release_ownership() {
        use sdl3::keyboard::Scancode;
        for reverse in [false, true] {
            let mut app = App::new(Limits::default()).unwrap();
            app.apply(Input::Focus(Some(COMPOSER))).unwrap();
            let mut host = HostInput::default();
            host.synchronize(&app, 0);
            host.event(
                &mut app,
                &key(Keycode::Return, Scancode::Return, Mod::NOMOD, true),
                1,
            )
            .unwrap();
            host.event(
                &mut app,
                &key(Keycode::KpEnter, Scancode::KpEnter, Mod::NOMOD, true),
                1,
            )
            .unwrap();
            assert!(app.ui().can_apply_key_effect(Scancode::Return as u16));
            assert!(app.ui().can_apply_key_effect(Scancode::KpEnter as u16));
            let (first, second) = if reverse {
                (
                    (Keycode::KpEnter, Scancode::KpEnter),
                    (Keycode::Return, Scancode::Return),
                )
            } else {
                (
                    (Keycode::Return, Scancode::Return),
                    (Keycode::KpEnter, Scancode::KpEnter),
                )
            };
            host.event(&mut app, &key(first.0, first.1, Mod::NOMOD, false), 1)
                .unwrap();
            assert!(!app.ui().can_apply_key_effect(first.1 as u16));
            assert!(app.ui().can_apply_key_effect(second.1 as u16));
            host.event(&mut app, &key(second.0, second.1, Mod::NOMOD, false), 1)
                .unwrap();
            assert!(!app.ui().can_apply_key_effect(second.1 as u16));
        }
    }
    #[test]
    fn a_world_arrow_repeated_after_focus_cannot_request_editor_effects() {
        use sdl3::keyboard::Scancode;
        let mut app = App::new(Limits::default()).unwrap();
        let mut host = HostInput::default();
        host.synchronize(&app, 0);
        host.event(
            &mut app,
            &key(Keycode::Left, Scancode::Left, Mod::NOMOD, true),
            1,
        )
        .unwrap();
        app.apply(Input::Focus(Some(COMPOSER))).unwrap();
        host.synchronize(&app, 1);
        let mut repeated = key(Keycode::Left, Scancode::Left, Mod::NOMOD, true);
        if let Event::KeyDown { repeat, .. } = &mut repeated {
            *repeat = true;
        }
        host.event(&mut app, &repeated, 1).unwrap();
        assert!(host.last_consumed);
        assert!(!app.ui().can_apply_key_effect(Scancode::Left as u16));
        assert!(host.world_held.is_empty());
    }
    #[test]
    fn commit_and_enter_in_either_batch_order_confirm_without_submitting() {
        use sdl3::keyboard::Scancode;
        for commit_first in [true, false] {
            let mut app = App::new(Limits::default()).unwrap();
            app.apply(Input::Focus(Some(COMPOSER))).unwrap();
            let mut host = HostInput::default();
            host.synchronize(&app, 0);
            let commit = Event::TextInput {
                timestamp: 100,
                window_id: 1,
                text: "confirmed".into(),
            };
            let enter = key(Keycode::Return, Scancode::Return, Mod::NOMOD, true);
            let batch = if commit_first {
                vec![commit, enter]
            } else {
                vec![enter, commit]
            };
            host.begin_batch(&app, &batch, 1);
            for e in &batch {
                host.event(&mut app, e, 1).unwrap();
            }
            assert!(app.ui().peek_actions().is_none());
            assert_eq!(app.ui().snapshot().composer.committed, "confirmed");
            host.event(
                &mut app,
                &key(Keycode::Return, Scancode::Return, Mod::NOMOD, false),
                1,
            )
            .unwrap();
            host.begin_batch(&app, &[], 1);
            host.event(
                &mut app,
                &key(Keycode::Return, Scancode::Return, Mod::NOMOD, true),
                1,
            )
            .unwrap();
            assert_eq!(app.ui().peek_actions().unwrap().actions.len(), 1);
            assert_eq!(app.ui().snapshot().composer.committed, "");
        }
    }
}
