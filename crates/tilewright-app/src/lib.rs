// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Host-neutral lifecycle and acknowledged presentation bookkeeping.
#![forbid(unsafe_code)]
use tilewright_ui::{
    BatchId, Disposition, Input, Layout, Limits, TextSelection, TextTarget, Ui, UiError,
    UiRevision, UiSnapshot, WidgetRef,
};
pub use tilewright_ui::{Composition, HitRegion, Paint, Rect, Viewport};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppError {
    Ui(UiError),
    Stale,
    QueueFull,
    Exhausted,
}
impl From<UiError> for AppError {
    fn from(value: UiError) -> Self {
        Self::Ui(value)
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AppError {}
macro_rules! identity { ($($name:ident),+) => {$ (
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)] pub struct $name(u64);
    impl $name { pub const fn new(value: u64) -> Result<Self, AppError> { if value == 0 { Err(AppError::Stale) } else { Ok(Self(value)) } } pub const fn get(self) -> u64 { self.0 } fn next(self) -> Result<Self, AppError> { self.0.checked_add(1).map(Self).ok_or(AppError::Exhausted) } }
)+}; }
identity!(SurfaceEpoch, FrameId, GeometryEpoch);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationStamp {
    pub surface: SurfaceEpoch,
    pub frame: FrameId,
    pub geometry: GeometryEpoch,
    pub ui_revision: UiRevision,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFrame {
    pub stamp: PresentationStamp,
    pub snapshot: UiSnapshot,
    pub layout: Layout,
}
pub type DisplayedFrame = PreparedFrame;
#[derive(Clone, Copy, Debug, PartialEq)]
struct Attachment {
    viewport: Viewport,
    bounds: Rect,
    clip: Rect,
}
#[derive(Debug)]
pub struct App {
    ui: Ui,
    surface: SurfaceEpoch,
    next_frame: FrameId,
    geometry: GeometryEpoch,
    attachment: Option<Attachment>,
    prepared: Vec<PreparedFrame>,
    displayed: Option<DisplayedFrame>,
}
impl App {
    pub fn new(limits: Limits) -> Result<Self, AppError> {
        Self::with_composition(limits, Composition::Demo)
    }
    pub fn with_composition(limits: Limits, composition: Composition) -> Result<Self, AppError> {
        Ok(Self {
            ui: Ui::with_composition(limits, composition)?,
            surface: SurfaceEpoch(1),
            next_frame: FrameId(1),
            geometry: GeometryEpoch(1),
            attachment: None,
            prepared: Vec::new(),
            displayed: None,
        })
    }
    /// Read-only access preserves the context's monotonic presentation identity.
    ///
    /// ```compile_fail
    /// let mut app = tilewright_app::App::new(tilewright_ui::Limits::default()).unwrap();
    /// *app.ui_mut() = tilewright_ui::Ui::new(tilewright_ui::Limits::default()).unwrap();
    /// ```
    pub fn ui(&self) -> &Ui {
        &self.ui
    }
    pub fn bind_field(&mut self, text: impl Into<String>) -> Result<(), AppError> {
        Ok(self.ui.bind_field(text)?)
    }
    pub fn ack_actions(&mut self, id: BatchId) -> Result<(), AppError> {
        Ok(self.ui.ack_actions(id)?)
    }
    pub fn apply(&mut self, input: Input) -> Result<Disposition, AppError> {
        Ok(self.ui.apply(input)?)
    }
    pub fn displayed(&self) -> Option<&DisplayedFrame> {
        self.displayed.as_ref()
    }
    pub fn surface_epoch(&self) -> SurfaceEpoch {
        self.surface
    }
    pub fn viewport(&self) -> Option<Viewport> {
        self.attachment.map(|attachment| attachment.viewport)
    }
    /// Attach a panel before routing geometric input. Changed attachments retire
    /// both displayed and prepared frames immediately. Identical calls are no-ops;
    /// hosts with changed external transforms must call `invalidate_surface`.
    pub fn set_layout(
        &mut self,
        viewport: Viewport,
        bounds: Rect,
        clip: Rect,
    ) -> Result<(), AppError> {
        let clip = tilewright_ui::validate_layout(viewport, bounds, clip)?;
        let attachment = Attachment {
            viewport,
            bounds,
            clip,
        };
        if self.attachment == Some(attachment) {
            return Ok(());
        }
        // Retire control ownership even on an embedded context's first attach.
        // Preserve the original demo's initial preparation semantics.
        if self.attachment.is_some() || self.ui.composition() != Composition::Demo {
            self.invalidate_surface()?;
        }
        self.attachment = Some(attachment);
        Ok(())
    }
    /// Full-viewport convenience for the original demo composition.
    pub fn prepare(&mut self, viewport: Viewport) -> Result<PreparedFrame, AppError> {
        if self.ui.composition() != Composition::Demo {
            return Err(UiError::InvalidInput.into());
        }
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport.width,
            height: viewport.height,
        };
        self.set_layout(viewport, bounds, bounds)?;
        self.prepare_current()
    }
    /// Prepare an immutable frame using the exact current attachment.
    pub fn prepare_current(&mut self) -> Result<PreparedFrame, AppError> {
        let attachment = self.attachment.ok_or(AppError::Stale)?;
        if self.prepared.len() >= self.ui.limits().prepared_frames {
            return Err(AppError::QueueFull);
        }
        let next = self.next_frame.next()?;
        let geometry = self.geometry.next()?;
        let snapshot = self.ui.snapshot();
        let frame = PreparedFrame {
            stamp: PresentationStamp {
                surface: self.surface,
                frame: self.next_frame,
                geometry,
                ui_revision: snapshot.revision,
            },
            layout: tilewright_ui::layout_in(
                &snapshot,
                attachment.viewport,
                attachment.bounds,
                attachment.clip,
            )?,
            snapshot,
        };
        self.next_frame = next;
        self.geometry = geometry;
        self.prepared.push(frame.clone());
        Ok(frame)
    }
    pub fn ack_presented(&mut self, stamp: PresentationStamp) -> Result<(), AppError> {
        if stamp.surface != self.surface {
            return Err(AppError::Stale);
        }
        if self
            .displayed
            .as_ref()
            .is_some_and(|frame| frame.stamp == stamp)
        {
            return Ok(());
        }
        if self
            .displayed
            .as_ref()
            .is_some_and(|frame| frame.stamp.frame >= stamp.frame)
        {
            return Err(AppError::Stale);
        }
        let index = self
            .prepared
            .iter()
            .position(|frame| frame.stamp == stamp)
            .ok_or(AppError::Stale)?;
        self.displayed = Some(self.prepared.remove(index));
        // Other submitted snapshots remain retained until the host retires them.
        Ok(())
    }
    /// Host confirms a prepared frame is no longer in flight and will not display.
    pub fn discard_prepared(&mut self, stamp: PresentationStamp) -> Result<(), AppError> {
        let index = self
            .prepared
            .iter()
            .position(|frame| frame.stamp == stamp)
            .ok_or(AppError::Stale)?;
        self.prepared.remove(index);
        Ok(())
    }
    pub fn invalidate_surface(&mut self) -> Result<SurfaceEpoch, AppError> {
        let next = self.surface.next()?;
        self.ui.apply(Input::CancelControls)?;
        self.surface = next;
        self.displayed = None;
        self.prepared.clear();
        Ok(next)
    }
    pub fn validate_displayed(
        &self,
        stamp: PresentationStamp,
    ) -> Result<&DisplayedFrame, AppError> {
        self.displayed
            .as_ref()
            .filter(|frame| {
                frame.stamp == stamp
                    && stamp.surface == self.surface
                    && stamp.ui_revision == self.ui.revision()
            })
            .ok_or(AppError::Stale)
    }
    pub fn hit_test(
        &self,
        stamp: PresentationStamp,
        x: f32,
        y: f32,
    ) -> Result<Option<WidgetRef>, AppError> {
        let frame = self.validate_displayed(stamp)?;
        Ok(frame
            .layout
            .hits
            .iter()
            .rev()
            .find(|hit| hit.bounds.contains(x, y) && hit.clip.contains(x, y))
            .map(|hit| hit.widget))
    }
    pub fn select_text(
        &mut self,
        stamp: PresentationStamp,
        target: TextTarget,
        selection: TextSelection,
    ) -> Result<Disposition, AppError> {
        self.validate_displayed(stamp)?;
        self.apply(Input::Selection { target, selection })
    }
}

#[cfg(test)]
mod attachment_identity_tests {
    use super::*;

    #[test]
    fn attachment_exhaustion_preserves_display_and_control() {
        let viewport = Viewport::new(100.0, 100.0, 1.0).unwrap();
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 80.0,
        };
        let mut app = App::with_composition(Limits::default(), Composition::SingleField).unwrap();
        app.set_layout(viewport, bounds, bounds).unwrap();
        let frame = app.prepare_current().unwrap();
        app.ack_presented(frame.stamp).unwrap();
        app.surface = SurfaceEpoch(u64::MAX);
        let before = app.ui.snapshot();
        let attachment = app.attachment;
        assert_eq!(
            app.set_layout(viewport, Rect { x: 1.0, ..bounds }, bounds),
            Err(AppError::Exhausted)
        );
        assert_eq!(app.attachment, attachment);
        assert_eq!(app.ui.snapshot(), before);
        assert_eq!(app.displayed(), Some(&frame));
    }
}
