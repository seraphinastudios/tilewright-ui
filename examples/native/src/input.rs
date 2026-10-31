// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
//! SDL conversion only; all editing and key ownership remain in shared Ui.
use sdl3::{
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod},
    mouse::MouseButton,
};
use tilewright_app::App;
use tilewright_ui::{COMPOSER, Input, Key, KeyEvent, SUBMIT, WORLD};

#[derive(Default)]
pub struct HostInput {
    epoch: u64,
    session_start: u64,
    pub text_active: bool,
    pub world_held: std::collections::BTreeSet<u16>,
    pressed: std::collections::BTreeMap<u16, Key>,
}
impl HostInput {
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
    pub fn event(
        &mut self,
        app: &mut App,
        event: &Event,
        window_id: u32,
    ) -> Result<bool, tilewright_app::AppError> {
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
                let code = scancode.map_or(0, |code| code as u16);
                let key = *self
                    .pressed
                    .entry(code)
                    .or_insert_with(|| semantic_key(*keycode, *keymod, code));
                let disposition = app.apply(Input::Key(KeyEvent {
                    key,
                    pressed: true,
                    repeat: *repeat,
                }))?;
                if !repeat {
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
                let code = scancode.map_or(0, |code| code as u16);
                app.apply(Input::Key(KeyEvent {
                    key: self
                        .pressed
                        .remove(&code)
                        .unwrap_or_else(|| semantic_key(*keycode, *keymod, code)),
                    pressed: false,
                    repeat: false,
                }))?;
                self.world_held.remove(&code);
            }
            Event::MouseButtonDown {
                window_id: id,
                mouse_btn: MouseButton::Left,
                x,
                y,
                ..
            } if *id == window_id => {
                if let Some(stamp) = app.displayed().map(|frame| frame.stamp) {
                    if let Ok(Some(widget)) = app.hit_test(stamp, *x, *y) {
                        match widget.id {
                            COMPOSER => {
                                app.apply(Input::Focus(Some(COMPOSER)))?;
                            }
                            SUBMIT => {
                                app.apply(Input::Activate(SUBMIT))?;
                            }
                            WORLD => {
                                app.apply(Input::Focus(None))?;
                            }
                            _ => (),
                        }
                    }
                }
            }
            Event::MouseWheel {
                window_id: id, y, ..
            } if *id == window_id => {
                app.apply(Input::Scroll(*y as i32))?;
            }
            _ => (),
        }
        Ok(true)
    }
}
fn semantic_key(key: Option<Keycode>, modifiers: Mod, physical: u16) -> Key {
    match key {
        Some(Keycode::Return | Keycode::KpEnter) => Key::Enter,
        Some(Keycode::Backspace) => Key::Backspace,
        Some(Keycode::Delete) => Key::Delete,
        Some(Keycode::Escape) => Key::Escape,
        Some(Keycode::A) if modifiers.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) => Key::SelectAll,
        _ => Key::Other(physical),
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
}
