// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

use serde_json::{Value, json};
use tilewright_app::{App, FrameId, GeometryEpoch, PresentationStamp, SurfaceEpoch};
use tilewright_ui::{Action, BatchId, ControlEpoch, Input, Limits, TextCursor, TextRevision, TextSelection, TextTarget, UiRevision, Viewport, WidgetGeneration, WidgetRef, COMPOSER, SUBMIT};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use crate::{utf16_to_utf8, utf8_to_utf16};

type Result<T> = std::result::Result<T, String>;
fn error(value: impl std::fmt::Display) -> String { value.to_string() }
fn number(value: &str) -> Result<u64> {
    if value.is_empty() || value.starts_with('0') || !value.bytes().all(|c| c.is_ascii_digit()) { return Err("InvalidInput".into()); }
    value.parse().map_err(error)
}
fn target(generation: &str, epoch: &str, revision: &str) -> Result<TextTarget> {
    Ok(TextTarget { widget: WidgetRef { id: COMPOSER, generation: WidgetGeneration::new(number(generation)?).map_err(error)? }, control_epoch: ControlEpoch::new(number(epoch)?).map_err(error)?, revision: TextRevision::new(number(revision)?).map_err(error)? })
}
fn stamp(value: &str) -> Result<PresentationStamp> {
    let value: Value = serde_json::from_str(value).map_err(error)?;
    let field = |key| value.get(key).and_then(Value::as_str).ok_or_else(|| "InvalidInput".to_owned()).and_then(number);
    Ok(PresentationStamp { surface: SurfaceEpoch::new(field("surface")?).map_err(error)?, frame: FrameId::new(field("frame")?).map_err(error)?, geometry: GeometryEpoch::new(field("geometry")?).map_err(error)?, ui_revision: UiRevision::new(field("revision")?).map_err(error)? })
}
fn stamp_json(stamp: PresentationStamp) -> Value {
    json!({"surface": stamp.surface.get().to_string(), "frame": stamp.frame.get().to_string(), "geometry": stamp.geometry.get().to_string(), "revision": stamp.ui_revision.get().to_string()})
}

/// One browser-owned app. JavaScript holds presentation/event bookkeeping only.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct BrowserUi { app: App }
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl BrowserUi {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Result<BrowserUi> { Ok(Self { app: App::new(Limits::default()).map_err(error)? }) }
    pub fn snapshot(&self) -> Result<String> {
        let state = self.app.ui().snapshot();
        let composer = &state.composer;
        let selection = composer.display_selection;
        let dom_anchor = utf8_to_utf16(&composer.display_text, selection.anchor.byte).map_err(|_| "InvalidInput")?;
        let dom_focus = utf8_to_utf16(&composer.display_text, selection.focus.byte).map_err(|_| "InvalidInput")?;
        Ok(json!({
            "revision":state.revision.get().to_string(), "generation":state.composer_widget.generation.get().to_string(),
            "textRevision":composer.revision.get().to_string(), "text":composer.committed, "displayText":composer.display_text,
            "anchor":dom_anchor, "focus":dom_focus, "composing":composer.composition.is_some(),
            "composerVisible":state.composer_visible, "messages":state.messages, "scroll":state.scroll,
            "control":{"epoch":state.control.epoch.get().to_string(), "worldAllowed":state.control.world_input_allowed,
                "focused":state.control.focused_widget.map(|w|w.id.get().to_string()), "modal":state.control.modal_open, "suspended":state.control.suspended}
        }).to_string())
    }
    pub fn focus(&mut self, focused: bool) -> Result<()> { self.app.apply(Input::Focus(focused.then_some(COMPOSER))).map_err(error)?; Ok(()) }
    pub fn modal(&mut self, open: bool) -> Result<()> { self.app.apply(Input::Modal(open)).map_err(error)?; Ok(()) }
    pub fn window_focused(&mut self, focused: bool) -> Result<()> { self.app.apply(Input::WindowFocused(focused)).map_err(error)?; Ok(()) }
    pub fn suspend(&mut self, suspended: bool) -> Result<()> { self.app.apply(Input::Suspend(suspended)).map_err(error)?; Ok(()) }
    pub fn receive(&mut self, text: &str) -> Result<()> { self.app.apply(Input::ReceiveMessage(text.to_owned())).map_err(error)?; Ok(()) }
    pub fn scroll(&mut self, delta: i32) -> Result<()> { self.app.apply(Input::Scroll(delta)).map_err(error)?; Ok(()) }
    pub fn submit(&mut self) -> Result<()> { self.app.apply(Input::Activate(SUBMIT)).map_err(error)?; Ok(()) }
    /// `generation`, `epoch`, and `revision` are captured before the native edit.
    pub fn replace(&mut self, text: &str, anchor: usize, focus: usize, generation: &str, epoch: &str, revision: &str) -> Result<()> {
        let selection = TextSelection { anchor: TextCursor::new(utf16_to_utf8(text, anchor).map_err(|_|"InvalidInput")?), focus: TextCursor::new(utf16_to_utf8(text, focus).map_err(|_|"InvalidInput")?) };
        self.app.apply(Input::ReplaceText { target: target(generation, epoch, revision)?, text: text.to_owned(), selection }).map_err(error)?; Ok(())
    }
    pub fn preedit(&mut self, text: &str, generation: &str, epoch: &str, revision: &str) -> Result<()> {
        // Browser composition events expose a string, not a reliable candidate
        // selection. -1/-1 explicitly marks the preedit selection as unavailable.
        self.app.apply(Input::Preedit { target: target(generation, epoch, revision)?, text:text.to_owned(), start:-1, length:-1 }).map_err(error)?; Ok(())
    }
    pub fn cancel_composition(&mut self, generation: &str, epoch: &str, revision: &str) -> Result<()> {
        self.app.apply(Input::CancelComposition { target:target(generation, epoch, revision)? }).map_err(error)?; Ok(())
    }
    /// Selection from a native element is still fenced by the displayed commit.
    pub fn select(&mut self, presentation: &str, anchor: usize, focus: usize, generation: &str, epoch: &str, revision: &str) -> Result<()> {
        let state = self.app.ui().snapshot();
        let selection = TextSelection { anchor: TextCursor::new(utf16_to_utf8(&state.composer.committed, anchor).map_err(|_|"InvalidInput")?), focus:TextCursor::new(utf16_to_utf8(&state.composer.committed, focus).map_err(|_|"InvalidInput")?) };
        self.app.select_text(stamp(presentation)?, target(generation, epoch, revision)?, selection).map_err(error)?; Ok(())
    }
    pub fn peek_actions(&self) -> String {
        self.app.ui().peek_actions().map_or_else(||"null".into(), |batch| json!({"id":batch.id.get().to_string(),"actions":batch.actions.iter().map(|action|match action { Action::SubmitChat{text}=>json!({"kind":"chat","text":text}) }).collect::<Vec<_>>()}).to_string())
    }
    pub fn ack_actions(&mut self, id: &str) -> Result<()> { self.app.ui_mut().ack_actions(BatchId::new(number(id)?).map_err(error)?).map_err(error) }
    pub fn prepare(&mut self, width: f32, height: f32, scale: f32) -> Result<String> {
        let frame = self.app.prepare(Viewport::new(width,height,scale).map_err(error)?).map_err(error)?;
        Ok(stamp_json(frame.stamp).to_string())
    }
    pub fn ack_presented(&mut self, presentation: &str) -> Result<()> { self.app.ack_presented(stamp(presentation)?).map_err(error) }
    pub fn discard_prepared(&mut self, presentation: &str) -> Result<()> { self.app.discard_prepared(stamp(presentation)?).map_err(error) }
    pub fn invalidate_surface(&mut self) -> Result<()> { self.app.invalidate_surface().map_err(error)?; Ok(()) }
    pub fn validate_displayed(&self, presentation: &str) -> Result<()> { self.app.validate_displayed(stamp(presentation)?).map_err(error)?; Ok(()) }
    pub fn hit(&self, presentation: &str, x: f32, y: f32) -> Result<Option<String>> { Ok(self.app.hit_test(stamp(presentation)?,x,y).map_err(error)?.map(|w|w.id.get().to_string())) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(ui: &BrowserUi) -> Value { serde_json::from_str(&ui.snapshot().unwrap()).unwrap() }
    fn replace(ui: &mut BrowserUi, text: &str) {
        let state = state(ui);
        ui.replace(text, text.encode_utf16().count(), text.encode_utf16().count(), state["generation"].as_str().unwrap(), state["control"]["epoch"].as_str().unwrap(), state["textRevision"].as_str().unwrap()).unwrap();
    }
    #[test]
    fn bounded_output_preserves_draft_and_duplicate_ack_is_idempotent() {
        let mut ui = BrowserUi::new().unwrap(); ui.focus(true).unwrap();
        for _ in 0..16 { replace(&mut ui,"hello"); ui.submit().unwrap(); }
        replace(&mut ui,"e\u{301}👩🏽‍💻 אבג"); let before = ui.snapshot().unwrap();
        assert!(ui.submit().is_err()); assert_eq!(ui.snapshot().unwrap(),before);
        let oldest = ui.peek_actions(); assert_eq!(ui.peek_actions(),oldest);
        let id = serde_json::from_str::<Value>(&oldest).unwrap()["id"].as_str().unwrap().to_owned();
        ui.ack_actions(&id).unwrap(); ui.ack_actions(&id).unwrap(); ui.submit().unwrap();
        assert_eq!(state(&ui)["text"],"");
    }
    #[test]
    fn stale_revisions_and_uncommitted_geometry_cannot_edit() {
        let mut ui = BrowserUi::new().unwrap(); ui.focus(true).unwrap();
        let old = state(&ui); let frame = ui.prepare(640.,480.,1.).unwrap();
        assert!(ui.select(&frame,0,0,old["generation"].as_str().unwrap(),old["control"]["epoch"].as_str().unwrap(),old["textRevision"].as_str().unwrap()).is_err());
        ui.ack_presented(&frame).unwrap(); replace(&mut ui,"hello");
        assert!(ui.replace("bad",3,3,old["generation"].as_str().unwrap(),old["control"]["epoch"].as_str().unwrap(),old["textRevision"].as_str().unwrap()).is_err());
        assert!(ui.validate_displayed(&frame).is_err()); ui.invalidate_surface().unwrap(); assert!(ui.ack_presented(&frame).is_err());
    }
}
