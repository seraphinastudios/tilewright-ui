// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Host-neutral lifecycle and acknowledged presentation bookkeeping.
#![forbid(unsafe_code)]
use tilewright_ui::{Disposition, Input, Layout, Limits, TextSelection, TextTarget, Ui, UiError, UiRevision, UiSnapshot, WidgetRef};
pub use tilewright_ui::{HitRegion, Paint, Rect, Viewport};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppError { Ui(UiError), Stale, QueueFull, Exhausted }
impl From<UiError> for AppError { fn from(value: UiError) -> Self { Self::Ui(value) } }
impl std::fmt::Display for AppError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for AppError {}
macro_rules! identity { ($($name:ident),+) => {$ (
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)] pub struct $name(u64);
    impl $name { pub const fn new(value: u64) -> Result<Self, AppError> { if value == 0 { Err(AppError::Stale) } else { Ok(Self(value)) } } pub const fn get(self) -> u64 { self.0 } fn next(self) -> Result<Self, AppError> { self.0.checked_add(1).map(Self).ok_or(AppError::Exhausted) } }
)+}; }
identity!(SurfaceEpoch, FrameId, GeometryEpoch);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationStamp { pub surface: SurfaceEpoch, pub frame: FrameId, pub geometry: GeometryEpoch, pub ui_revision: UiRevision }
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFrame { pub stamp: PresentationStamp, pub snapshot: UiSnapshot, pub layout: Layout }
pub type DisplayedFrame = PreparedFrame;
#[derive(Clone, Debug)]
pub struct App { ui: Ui, surface: SurfaceEpoch, next_frame: FrameId, geometry: GeometryEpoch, viewport: Option<Viewport>, prepared: Vec<PreparedFrame>, displayed: Option<DisplayedFrame> }
impl App {
    pub fn new(limits: Limits) -> Result<Self, AppError> { Ok(Self { ui: Ui::new(limits)?, surface: SurfaceEpoch(1), next_frame: FrameId(1), geometry: GeometryEpoch(1), viewport: None, prepared: Vec::new(), displayed: None }) }
    pub fn ui(&self) -> &Ui { &self.ui }
    /// Mutable access is safe: geometric input always checks the live revision.
    pub fn ui_mut(&mut self) -> &mut Ui { &mut self.ui }
    pub fn apply(&mut self, input: Input) -> Result<Disposition, AppError> { Ok(self.ui.apply(input)?) }
    pub fn displayed(&self) -> Option<&DisplayedFrame> { self.displayed.as_ref() }
    pub fn surface_epoch(&self) -> SurfaceEpoch { self.surface }
    pub fn prepare(&mut self, viewport: Viewport) -> Result<PreparedFrame, AppError> {
        let viewport = viewport.validate()?;
        if self.viewport.is_some_and(|old| old != viewport) { self.invalidate_surface()?; }
        self.viewport = Some(viewport);
        if self.prepared.len() >= self.ui.limits().prepared_frames { return Err(AppError::QueueFull); }
        let next = self.next_frame.next()?;
        let geometry = self.geometry.next()?;
        let snapshot = self.ui.snapshot();
        let frame = PreparedFrame { stamp: PresentationStamp { surface: self.surface, frame: self.next_frame, geometry, ui_revision: snapshot.revision }, layout: tilewright_ui::layout(&snapshot, viewport)?, snapshot };
        self.next_frame = next; self.geometry = geometry; self.prepared.push(frame.clone()); Ok(frame)
    }
    pub fn ack_presented(&mut self, stamp: PresentationStamp) -> Result<(), AppError> {
        if stamp.surface != self.surface { return Err(AppError::Stale); }
        if self.displayed.as_ref().is_some_and(|frame| frame.stamp == stamp) { return Ok(()); }
        if self.displayed.as_ref().is_some_and(|frame| frame.stamp.frame >= stamp.frame) { return Err(AppError::Stale); }
        let index = self.prepared.iter().position(|frame| frame.stamp == stamp).ok_or(AppError::Stale)?;
        self.displayed = Some(self.prepared.remove(index));
        // Other submitted snapshots remain retained until the host retires them.
        Ok(())
    }
    /// Host confirms a prepared frame is no longer in flight and will not display.
    pub fn discard_prepared(&mut self, stamp: PresentationStamp) -> Result<(), AppError> { let index = self.prepared.iter().position(|frame| frame.stamp == stamp).ok_or(AppError::Stale)?; self.prepared.remove(index); Ok(()) }
    pub fn invalidate_surface(&mut self) -> Result<SurfaceEpoch, AppError> { let next = self.surface.next()?; self.surface = next; self.displayed = None; self.prepared.clear(); Ok(next) }
    pub fn validate_displayed(&self, stamp: PresentationStamp) -> Result<&DisplayedFrame, AppError> { self.displayed.as_ref().filter(|frame| frame.stamp == stamp && stamp.surface == self.surface && stamp.ui_revision == self.ui.revision()).ok_or(AppError::Stale) }
    pub fn hit_test(&self, stamp: PresentationStamp, x: f32, y: f32) -> Result<Option<WidgetRef>, AppError> { let frame = self.validate_displayed(stamp)?; Ok(frame.layout.hits.iter().rev().find(|hit| hit.bounds.contains(x,y) && hit.clip.contains(x,y)).map(|hit| hit.widget)) }
    pub fn select_text(&mut self, stamp: PresentationStamp, target: TextTarget, selection: TextSelection) -> Result<Disposition, AppError> { self.validate_displayed(stamp)?; self.apply(Input::Selection { target, selection }) }
}
