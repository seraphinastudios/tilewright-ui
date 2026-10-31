// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

use crate::{utf8_to_utf16, utf16_to_utf8};
use serde_json::{Value, json};
use tilewright_app::{App, FrameId, GeometryEpoch, PresentationStamp, SurfaceEpoch};
use tilewright_ui::{
    Action, BatchId, COMPOSER, ControlEpoch, Disposition, Input, Key, KeyEvent, Limits, MODAL,
    SUBMIT, TextCursor, TextRevision, TextSelection, TextTarget, UiRevision, Viewport,
    WidgetGeneration, WidgetRef,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

type Result<T> = std::result::Result<T, String>;
fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}
fn number(value: &str) -> Result<u64> {
    if value.len() > 20
        || value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("InvalidInput".into());
    }
    value.parse().map_err(error)
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn offset(value: f64) -> Result<usize> {
    if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(u32::MAX) {
        return Err("InvalidInput".into());
    }
    Ok(value as usize)
}
fn target(generation: &str, epoch: &str, revision: &str) -> Result<TextTarget> {
    Ok(TextTarget {
        widget: WidgetRef {
            id: COMPOSER,
            generation: WidgetGeneration::new(number(generation)?).map_err(error)?,
        },
        control_epoch: ControlEpoch::new(number(epoch)?).map_err(error)?,
        revision: TextRevision::new(number(revision)?).map_err(error)?,
    })
}
fn stamp(value: &str) -> Result<PresentationStamp> {
    if value.len() > 256 {
        return Err("InvalidInput".into());
    }
    let value: Value = serde_json::from_str(value).map_err(error)?;
    let field = |key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| "InvalidInput".to_owned())
            .and_then(number)
    };
    Ok(PresentationStamp {
        surface: SurfaceEpoch::new(field("surface")?).map_err(error)?,
        frame: FrameId::new(field("frame")?).map_err(error)?,
        geometry: GeometryEpoch::new(field("geometry")?).map_err(error)?,
        ui_revision: UiRevision::new(field("revision")?).map_err(error)?,
    })
}
fn stamp_json(stamp: PresentationStamp) -> Value {
    json!({"surface": stamp.surface.get().to_string(), "frame": stamp.frame.get().to_string(), "geometry": stamp.geometry.get().to_string(), "revision": stamp.ui_revision.get().to_string()})
}

/// One browser-owned app. JavaScript holds presentation/event bookkeeping only.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct BrowserUi {
    app: App,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl BrowserUi {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Result<BrowserUi> {
        Ok(Self {
            app: App::new(Limits::default()).map_err(error)?,
        })
    }
    pub fn snapshot(&self) -> Result<String> {
        let state = self.app.ui().snapshot();
        let composer = &state.composer;
        let selection = composer.display_selection;
        let dom_anchor = utf8_to_utf16(&composer.display_text, selection.anchor.byte)
            .map_err(|_| "InvalidInput")?;
        let dom_focus = utf8_to_utf16(&composer.display_text, selection.focus.byte)
            .map_err(|_| "InvalidInput")?;
        Ok(json!({
            "revision":state.revision.get().to_string(), "generation":state.composer_widget.generation.get().to_string(),
            "textRevision":composer.revision.get().to_string(), "text":composer.committed, "displayText":composer.display_text,
            "anchor":dom_anchor, "focus":dom_focus, "composing":composer.composition.is_some(),
            "composerVisible":state.composer_visible, "messages":state.messages, "scroll":state.scroll,
            "control":{"epoch":state.control.epoch.get().to_string(), "worldAllowed":state.control.world_input_allowed,
                "focused":state.control.focused_widget.map(|w|w.id.get().to_string()), "modal":state.control.modal_open, "suspended":state.control.suspended}
        }).to_string())
    }
    pub fn focus(&mut self, focused: bool) -> Result<()> {
        self.app
            .apply(Input::Focus(focused.then_some(COMPOSER)))
            .map_err(error)?;
        Ok(())
    }
    /// Semantic DOM element focus, including buttons and modal controls.
    pub fn focus_widget(&mut self, widget: &str) -> Result<()> {
        let focus = match widget {
            "0" => None,
            "1" => Some(COMPOSER),
            "2" => Some(SUBMIT),
            "3" => Some(MODAL),
            _ => return Err("InvalidInput".into()),
        };
        self.app.apply(Input::Focus(focus)).map_err(error)?;
        Ok(())
    }
    pub fn modal(&mut self, open: bool) -> Result<()> {
        self.app.apply(Input::Modal(open)).map_err(error)?;
        Ok(())
    }
    pub fn window_focused(&mut self, focused: bool) -> Result<()> {
        self.app
            .apply(Input::WindowFocused(focused))
            .map_err(error)?;
        Ok(())
    }
    pub fn suspend(&mut self, suspended: bool) -> Result<()> {
        self.app.apply(Input::Suspend(suspended)).map_err(error)?;
        Ok(())
    }
    pub fn receive(&mut self, text: &str) -> Result<()> {
        self.app
            .apply(Input::ReceiveMessage(text.to_owned()))
            .map_err(error)?;
        Ok(())
    }
    pub fn scroll(&mut self, delta: i32) -> Result<()> {
        self.app.apply(Input::Scroll(delta)).map_err(error)?;
        Ok(())
    }
    pub fn submit(&mut self) -> Result<()> {
        self.app.apply(Input::Activate(SUBMIT)).map_err(error)?;
        Ok(())
    }
    /// Physical identity is independent of the semantic command. Text editing
    /// keys use `other`: native input is committed once with `replace`.
    pub fn key(
        &mut self,
        physical: f64,
        command: &str,
        pressed: bool,
        repeat: bool,
        platform_composing: bool,
    ) -> Result<String> {
        let physical = u16::try_from(offset(physical)?).map_err(error)?;
        if physical == 0 {
            return Err("InvalidInput".into());
        }
        let key = match command {
            "enter" => Key::Enter,
            "escape" => Key::Escape,
            "other" => Key::Other,
            _ => return Err("InvalidInput".into()),
        };
        let disposition = self
            .app
            .apply(Input::Key(KeyEvent {
                physical,
                key,
                pressed,
                repeat,
                platform_composing,
            }))
            .map_err(error)?;
        Ok(json!({"consumed": disposition == Disposition::Consumed,
            "keyEffects": self.app.ui().can_apply_key_effect(physical)})
        .to_string())
    }
    /// Recheck ownership at the point of a synchronous host key effect.
    pub fn key_effects(&self, physical: f64) -> Result<bool> {
        let physical = u16::try_from(offset(physical)?).map_err(error)?;
        if physical == 0 {
            return Err("InvalidInput".into());
        }
        Ok(self.app.ui().can_apply_key_effect(physical))
    }
    /// `generation`, `epoch`, and `revision` are captured before the native edit.
    #[allow(clippy::too_many_arguments)]
    pub fn replace(
        &mut self,
        text: &str,
        anchor: f64,
        focus: f64,
        generation: &str,
        epoch: &str,
        revision: &str,
    ) -> Result<()> {
        let selection = TextSelection {
            anchor: TextCursor::new(
                utf16_to_utf8(text, offset(anchor)?).map_err(|_| "InvalidInput")?,
            ),
            focus: TextCursor::new(
                utf16_to_utf8(text, offset(focus)?).map_err(|_| "InvalidInput")?,
            ),
        };
        self.app
            .apply(Input::ReplaceText {
                target: target(generation, epoch, revision)?,
                text: text.to_owned(),
                selection,
            })
            .map_err(error)?;
        Ok(())
    }
    pub fn preedit(
        &mut self,
        text: &str,
        generation: &str,
        epoch: &str,
        revision: &str,
    ) -> Result<()> {
        // Browser composition events expose a string, not a reliable candidate
        // selection. -1/-1 explicitly marks the preedit selection as unavailable.
        self.app
            .apply(Input::Preedit {
                target: target(generation, epoch, revision)?,
                text: text.to_owned(),
                start: -1,
                length: -1,
            })
            .map_err(error)?;
        Ok(())
    }
    pub fn cancel_composition(
        &mut self,
        generation: &str,
        epoch: &str,
        revision: &str,
    ) -> Result<()> {
        self.app
            .apply(Input::CancelComposition {
                target: target(generation, epoch, revision)?,
            })
            .map_err(error)?;
        Ok(())
    }
    /// Selection from a native element is still fenced by the displayed commit.
    #[allow(clippy::too_many_arguments)]
    pub fn select(
        &mut self,
        presentation: &str,
        anchor: f64,
        focus: f64,
        generation: &str,
        epoch: &str,
        revision: &str,
    ) -> Result<()> {
        let state = self.app.ui().snapshot();
        let selection = TextSelection {
            anchor: TextCursor::new(
                utf16_to_utf8(&state.composer.committed, offset(anchor)?)
                    .map_err(|_| "InvalidInput")?,
            ),
            focus: TextCursor::new(
                utf16_to_utf8(&state.composer.committed, offset(focus)?)
                    .map_err(|_| "InvalidInput")?,
            ),
        };
        self.app
            .select_text(
                stamp(presentation)?,
                target(generation, epoch, revision)?,
                selection,
            )
            .map_err(error)?;
        Ok(())
    }
    pub fn peek_actions(&self) -> String {
        self.app.ui().peek_actions().map_or_else(||"null".into(), |batch| json!({"id":batch.id.get().to_string(),"actions":batch.actions.iter().map(|action|match action { Action::SubmitChat{text}=>json!({"kind":"chat","text":text}) }).collect::<Vec<_>>()}).to_string())
    }
    pub fn ack_actions(&mut self, id: &str) -> Result<()> {
        self.app
            .ack_actions(BatchId::new(number(id)?).map_err(error)?)
            .map_err(error)
    }
    pub fn prepare(&mut self, width: f32, height: f32, scale: f32) -> Result<String> {
        let frame = self
            .app
            .prepare(Viewport::new(width, height, scale).map_err(error)?)
            .map_err(error)?;
        Ok(stamp_json(frame.stamp).to_string())
    }
    pub fn ack_presented(&mut self, presentation: &str) -> Result<()> {
        self.app.ack_presented(stamp(presentation)?).map_err(error)
    }
    pub fn discard_prepared(&mut self, presentation: &str) -> Result<()> {
        self.app
            .discard_prepared(stamp(presentation)?)
            .map_err(error)
    }
    pub fn invalidate_surface(&mut self) -> Result<()> {
        self.app.invalidate_surface().map_err(error)?;
        Ok(())
    }
    pub fn validate_displayed(&self, presentation: &str) -> Result<()> {
        self.app
            .validate_displayed(stamp(presentation)?)
            .map_err(error)?;
        Ok(())
    }
    pub fn hit(&self, presentation: &str, x: f32, y: f32) -> Result<Option<String>> {
        Ok(self
            .app
            .hit_test(stamp(presentation)?, x, y)
            .map_err(error)?
            .map(|w| w.id.get().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(ui: &BrowserUi) -> Value {
        serde_json::from_str(&ui.snapshot().unwrap()).unwrap()
    }
    fn replace(ui: &mut BrowserUi, text: &str) {
        let state = state(ui);
        ui.replace(
            text,
            f64::from(u32::try_from(text.encode_utf16().count()).unwrap()),
            f64::from(u32::try_from(text.encode_utf16().count()).unwrap()),
            state["generation"].as_str().unwrap(),
            state["control"]["epoch"].as_str().unwrap(),
            state["textRevision"].as_str().unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn bounded_output_preserves_draft_and_duplicate_ack_is_idempotent() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        for _ in 0..16 {
            replace(&mut ui, "hello");
            ui.submit().unwrap();
        }
        replace(&mut ui, "e\u{301}👩🏽‍💻 אבג");
        let before = ui.snapshot().unwrap();
        assert!(ui.submit().is_err());
        assert_eq!(ui.snapshot().unwrap(), before);
        let oldest = ui.peek_actions();
        assert_eq!(ui.peek_actions(), oldest);
        let id = serde_json::from_str::<Value>(&oldest).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        ui.ack_actions(&id).unwrap();
        ui.ack_actions(&id).unwrap();
        ui.submit().unwrap();
        assert_eq!(state(&ui)["text"], "");
    }
    #[test]
    fn physical_keys_keep_independent_effect_eligibility() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        let key = |ui: &mut BrowserUi, physical, pressed, repeat| -> Value {
            serde_json::from_str(&ui.key(physical, "other", pressed, repeat, false).unwrap())
                .unwrap()
        };
        assert_eq!(key(&mut ui, 11., true, false)["keyEffects"], true);
        assert_eq!(key(&mut ui, 12., true, false)["keyEffects"], true);
        key(&mut ui, 11., false, false);
        assert_eq!(key(&mut ui, 12., true, true)["keyEffects"], true);
        ui.modal(true).unwrap();
        ui.modal(false).unwrap();
        ui.focus(true).unwrap();
        let quarantined = key(&mut ui, 12., true, true);
        assert_eq!(quarantined["consumed"], true);
        assert_eq!(quarantined["keyEffects"], false);
        key(&mut ui, 12., false, false);
        assert_eq!(key(&mut ui, 12., true, false)["keyEffects"], true);
    }

    #[test]
    fn distinct_enter_keys_each_submit_and_invalid_physical_ids_are_atomic() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        replace(&mut ui, "first");
        ui.key(11., "enter", true, false, false).unwrap();
        replace(&mut ui, "second");
        ui.key(12., "enter", true, false, false).unwrap();
        let first: Value = serde_json::from_str(&ui.peek_actions()).unwrap();
        assert_eq!(first["actions"][0]["text"], "first");
        ui.ack_actions(first["id"].as_str().unwrap()).unwrap();
        let second: Value = serde_json::from_str(&ui.peek_actions()).unwrap();
        assert_eq!(second["actions"][0]["text"], "second");
        let before = ui.snapshot().unwrap();
        for physical in [0., -1., 65_536., 1.5, f64::NAN] {
            assert!(ui.key(physical, "enter", true, false, false).is_err());
            assert_eq!(ui.snapshot().unwrap(), before);
        }
    }

    #[test]
    fn platform_composing_enter_fences_button_without_visible_preedit() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        replace(&mut ui, "draft");
        ui.key(11., "enter", true, false, true).unwrap();
        assert!(!ui.key_effects(11.).unwrap());
        ui.submit().unwrap();
        assert_eq!(ui.peek_actions(), "null");
        assert_eq!(state(&ui)["text"], "draft");
        ui.key(11., "enter", false, false, false).unwrap();
        ui.submit().unwrap();
        assert_ne!(ui.peek_actions(), "null");
    }

    #[test]
    fn refuses_lossy_browser_numbers_before_modifying_state() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        let state = state(&ui);
        let before = ui.snapshot().unwrap();
        for position in [f64::NAN, f64::INFINITY, -1., 0.5, 4_294_967_296.] {
            assert!(
                ui.replace(
                    "",
                    position,
                    position,
                    state["generation"].as_str().unwrap(),
                    state["control"]["epoch"].as_str().unwrap(),
                    state["textRevision"].as_str().unwrap()
                )
                .is_err()
            );
            assert_eq!(ui.snapshot().unwrap(), before);
        }
    }
    #[test]
    fn stale_revisions_and_uncommitted_geometry_cannot_edit() {
        let mut ui = BrowserUi::new().unwrap();
        ui.focus(true).unwrap();
        let old = state(&ui);
        let frame = ui.prepare(640., 480., 1.).unwrap();
        assert!(
            ui.select(
                &frame,
                0.,
                0.,
                old["generation"].as_str().unwrap(),
                old["control"]["epoch"].as_str().unwrap(),
                old["textRevision"].as_str().unwrap()
            )
            .is_err()
        );
        ui.ack_presented(&frame).unwrap();
        replace(&mut ui, "hello");
        assert!(
            ui.replace(
                "bad",
                3.,
                3.,
                old["generation"].as_str().unwrap(),
                old["control"]["epoch"].as_str().unwrap(),
                old["textRevision"].as_str().unwrap()
            )
            .is_err()
        );
        assert!(ui.validate_displayed(&frame).is_err());
        ui.invalidate_surface().unwrap();
        assert!(ui.ack_presented(&frame).is_err());
    }
}
