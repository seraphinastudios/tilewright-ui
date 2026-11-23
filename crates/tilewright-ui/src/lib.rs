// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Bounded host-independent chat and single-line editing semantics.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
mod keys;
mod text;
use keys::KeyLedger;
pub use text::{
    CONTROL_MARKER, CompositionSnapshot, DisplayMarker, LINE_BREAK_MARKER, PARAGRAPH_MARKER,
    TAB_MARKER, TextAffinity, TextCursor, TextInput, TextInputSnapshot, TextSelection,
    mark_stored_breaks, refuses_single_line, scalar_selection_bytes,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UiError {
    InvalidInput,
    LimitExceeded,
    /// New text holds a character a single-line field never accepts (see
    /// [`refuses_single_line`]). Checked before the byte budget.
    RefusedText,
    QueueFull,
    Stale,
    Exhausted,
}
impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for UiError {}
macro_rules! identity { ($($name:ident),+) => {$ (
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
    pub struct $name(u64);
    impl $name {
        pub const fn new(value: u64) -> Result<Self, UiError> { if value == 0 { Err(UiError::InvalidInput) } else { Ok(Self(value)) } }
        pub const fn get(self) -> u64 { self.0 }
        pub fn next(self) -> Result<Self, UiError> { self.0.checked_add(1).map(Self).ok_or(UiError::Exhausted) }
    }
)+}; }
identity!(
    WidgetId,
    WidgetGeneration,
    UiRevision,
    TextRevision,
    BatchId,
    ControlEpoch
);
pub const COMPOSER: WidgetId = WidgetId(1);
pub const SUBMIT: WidgetId = WidgetId(2);
pub const MODAL: WidgetId = WidgetId(3);
pub const WORLD: WidgetId = WidgetId(4);
pub const HISTORY: WidgetId = WidgetId(5);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WidgetRef {
    pub id: WidgetId,
    pub generation: WidgetGeneration,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextTarget {
    pub widget: WidgetRef,
    pub control_epoch: ControlEpoch,
    pub revision: TextRevision,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub message_bytes: usize,
    pub history_messages: usize,
    pub history_bytes: usize,
    pub composer_bytes: usize,
    pub queued_batches: usize,
    pub prepared_frames: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            message_bytes: 4096,
            history_messages: 64,
            history_bytes: 65536,
            composer_bytes: 4096,
            queued_batches: 16,
            prepared_frames: 3,
        }
    }
}
/// Immutable selection of the widgets owned by one context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum Composition {
    #[default]
    Demo,
    ChatPanel,
    SingleField,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    SubmitChat {
        text: String,
    },
    /// A retained field draft. The host owns mapping this identity to its domain.
    SubmitField {
        widget: WidgetRef,
        revision: TextRevision,
        text: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ActionBatch {
    pub id: BatchId,
    pub actions: Vec<Action>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Consumed,
    Unhandled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ControlSnapshot {
    pub epoch: ControlEpoch,
    pub world_input_allowed: bool,
    pub focused_widget: Option<WidgetRef>,
    pub modal_open: bool,
    pub suspended: bool,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Key {
    Enter,
    Backspace,
    Delete,
    SelectAll,
    Escape,
    Other,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    /// This event was handled by platform composition, even if preedit is empty.
    /// Event metadata only; it does not create a persistent composition session.
    pub platform_composing: bool,
    /// Stable host physical identity, independent of the semantic command.
    /// Hosts must distinguish keys such as Return and keypad Enter.
    pub physical: u16,
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TextCommand {
    Backspace,
    Delete,
    SelectAll,
}
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Input {
    Focus(Option<WidgetId>),
    CancelControls,
    Modal(bool),
    Suspend(bool),
    WindowFocused(bool),
    ComposerVisible(bool),
    Key(KeyEvent),
    TextCommit {
        target: TextTarget,
        text: String,
    },
    Preedit {
        target: TextTarget,
        text: String,
        start: i32,
        length: i32,
    },
    CancelComposition {
        target: TextTarget,
    },
    Selection {
        target: TextTarget,
        selection: TextSelection,
    },
    Edit {
        target: TextTarget,
        command: TextCommand,
    },
    ReplaceText {
        target: TextTarget,
        text: String,
        selection: TextSelection,
    },
    Activate(WidgetId),
    ReceiveMessage(String),
    Scroll(i32),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct UiSnapshot {
    pub composition: Composition,
    pub revision: UiRevision,
    pub composer_widget: WidgetRef,
    pub composer_visible: bool,
    pub composer: TextInputSnapshot,
    pub messages: Vec<String>,
    pub scroll: usize,
    pub control: ControlSnapshot,
}
#[derive(Clone, Debug)]
pub struct Ui {
    composition: Composition,
    limits: Limits,
    revision: UiRevision,
    composer: TextInput,
    generation: WidgetGeneration,
    composer_visible: bool,
    history: VecDeque<String>,
    history_bytes: usize,
    scroll: usize,
    queue: VecDeque<ActionBatch>,
    next_batch: BatchId,
    acknowledged: u64,
    control: ControlSnapshot,
    window_focused: bool,
    keys: KeyLedger,
}
impl Ui {
    pub fn new(limits: Limits) -> Result<Self, UiError> {
        Self::with_composition(limits, Composition::Demo)
    }
    pub fn with_composition(limits: Limits, composition: Composition) -> Result<Self, UiError> {
        if [
            limits.message_bytes,
            limits.history_messages,
            limits.history_bytes,
            limits.composer_bytes,
            limits.queued_batches,
            limits.prepared_frames,
        ]
        .contains(&0)
            || limits.message_bytes > limits.history_bytes
        {
            return Err(UiError::InvalidInput);
        }
        Ok(Self {
            composition,
            limits,
            revision: UiRevision(1),
            composer: TextInput::new(String::new(), limits.composer_bytes)?,
            generation: WidgetGeneration(1),
            composer_visible: true,
            history: VecDeque::new(),
            history_bytes: 0,
            scroll: 0,
            queue: VecDeque::new(),
            next_batch: BatchId(1),
            acknowledged: 0,
            control: ControlSnapshot {
                epoch: ControlEpoch(1),
                world_input_allowed: true,
                focused_widget: None,
                modal_open: false,
                suspended: false,
            },
            window_focused: true,
            keys: KeyLedger::new(),
        })
    }
    pub fn composition(&self) -> Composition {
        self.composition
    }
    /// Retire a host-bound field identity without consuming pending actions.
    /// Binding advances identities even when the bytes are unchanged, clears
    /// focus and preedit, and makes late input for the old binding stale.
    pub fn bind_field(&mut self, text: impl Into<String>) -> Result<(), UiError> {
        if self.composition != Composition::SingleField {
            return Err(UiError::InvalidInput);
        }
        let mut composer = self.composer.clone();
        composer.bind(text.into())?;
        let generation = self.generation.next()?;
        let revision = self.revision.next()?;
        let epoch = self.control.epoch.next()?;
        self.composer = composer;
        self.generation = generation;
        self.revision = revision;
        self.control.epoch = epoch;
        self.control.focused_widget = None;
        self.control.world_input_allowed =
            self.window_focused && !self.control.suspended && !self.control.modal_open;
        self.keys.cancel();
        Ok(())
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn revision(&self) -> UiRevision {
        self.revision
    }
    pub fn composer_widget(&self) -> WidgetRef {
        WidgetRef {
            id: COMPOSER,
            generation: self.generation,
        }
    }
    /// The composer's committed text (the single field's draft in
    /// [`Composition::SingleField`]), borrowed: `snapshot().composer.committed`
    /// without copying a snapshot. Stored line breaks are included; an open
    /// composition's preedit is not. Like every accessor it reads the current
    /// state, which the next accepted input may change.
    pub fn composer_text(&self) -> &str {
        self.composer.text()
    }
    /// The composer's text revision, `snapshot().composer.revision`, whether
    /// or not the composer is focused (unlike [`Ui::text_target`]). It moves
    /// on every accepted change to the text, selection or composition.
    pub fn composer_revision(&self) -> TextRevision {
        self.composer.revision()
    }
    /// The composer's selection in committed text,
    /// `snapshot().composer.selection`.
    pub fn composer_selection(&self) -> TextSelection {
        self.composer.selection()
    }
    /// Whether a composition (IME preedit) is open in the composer,
    /// `snapshot().composer.composition.is_some()`.
    pub fn is_composing(&self) -> bool {
        self.composer.is_composing()
    }
    pub fn text_target(&self) -> Option<TextTarget> {
        (self.control.focused_widget == Some(self.composer_widget())).then(|| TextTarget {
            widget: self.composer_widget(),
            control_epoch: self.control.epoch,
            revision: self.composer.revision(),
        })
    }
    /// After a successful key application, allow host visual/clipboard effects
    /// only for the active owned press. Consumed alone also includes quarantine.
    pub fn can_apply_key_effect(&self, physical: u16) -> bool {
        self.keys.state(physical) == Some((true, false))
            && self.text_target().is_some()
            && self.composer_visible
            && self.window_focused
            && !self.control.modal_open
            && !self.control.suspended
            && !self.composer.is_composing()
    }
    pub fn control(&self) -> ControlSnapshot {
        self.control
    }
    pub fn snapshot(&self) -> UiSnapshot {
        UiSnapshot {
            composition: self.composition,
            revision: self.revision,
            composer_widget: self.composer_widget(),
            composer_visible: self.composer_visible,
            composer: self.composer.snapshot(),
            messages: self.history.iter().cloned().collect(),
            scroll: self.scroll,
            control: self.control,
        }
    }
    pub fn peek_actions(&self) -> Option<&ActionBatch> {
        self.queue.front()
    }
    pub fn ack_actions(&mut self, id: BatchId) -> Result<(), UiError> {
        if id.get() <= self.acknowledged {
            return Ok(());
        }
        if self.queue.front().map(|batch| batch.id) != Some(id) {
            return Err(UiError::Stale);
        }
        self.queue.pop_front();
        self.acknowledged = id.get();
        Ok(())
    }
    /// Ordinary widget state, revisions, and output commit only after validation.
    /// Physical press ownership is a separate safety ledger: a rejected key
    /// action still owns its eventual repeat/release, preventing world leakage.
    pub fn apply(&mut self, input: Input) -> Result<Disposition, UiError> {
        let key_physical = if let Input::Key(event) = &input {
            Some(event.physical)
        } else {
            None
        };
        let mut next = self.clone();
        match next.apply_inner(input) {
            Ok(result) => {
                *self = next;
                Ok(result)
            }
            Err(error) => {
                if let Some(physical) = key_physical {
                    self.keys = next.keys;
                    self.keys.quarantine(physical);
                }
                Err(error)
            }
        }
    }
    fn barrier(&mut self) -> Result<(), UiError> {
        self.control.epoch = self.control.epoch.next()?;
        self.keys.cancel();
        self.control.world_input_allowed = self.window_focused
            && !self.control.suspended
            && !self.control.modal_open
            && self.control.focused_widget.is_none();
        Ok(())
    }
    // Explicit cancellation retires the platform IME session as well as preedit.
    // Natural empty preedit and committed text deliberately do not use this path.
    fn cancel_composition(&mut self) -> Result<(), UiError> {
        if self.composer.is_composing() {
            self.composer.cancel_preedit()?;
            self.barrier()?;
        }
        Ok(())
    }
    fn require_target(&self, target: TextTarget) -> Result<(), UiError> {
        if self.text_target() == Some(target) {
            Ok(())
        } else {
            Err(UiError::Stale)
        }
    }
    fn submit(&mut self) -> Result<(), UiError> {
        if !self.composer_visible
            || self.control.suspended
            || !self.window_focused
            || self.control.modal_open
        {
            return Err(UiError::Stale);
        }
        if self.composer.is_composing() || self.keys.enter_fenced() {
            return Ok(());
        }
        if self.composition != Composition::SingleField && self.composer.text().is_empty() {
            return Ok(());
        }
        if self.composition != Composition::SingleField
            && self.composer.text().len() > self.limits.message_bytes
        {
            return Err(UiError::LimitExceeded);
        }
        if self.queue.len() == self.limits.queued_batches {
            return Err(UiError::QueueFull);
        }
        let following = self.next_batch.next()?;
        let text = self.composer.text().to_owned();
        let action = if self.composition == Composition::SingleField {
            Action::SubmitField {
                widget: self.composer_widget(),
                revision: self.composer.revision(),
                text,
            }
        } else {
            self.composer.select_all()?;
            self.composer.input("")?;
            Action::SubmitChat { text }
        };
        self.queue.push_back(ActionBatch {
            id: self.next_batch,
            actions: vec![action],
        });
        self.next_batch = following;
        Ok(())
    }
    // Keep the semantic transaction cases together so state and revision commits are auditable.
    #[allow(clippy::too_many_lines)]
    fn apply_inner(&mut self, input: Input) -> Result<Disposition, UiError> {
        let old_text = self.composer.revision();
        let mut changed = false;
        match input {
            Input::CancelControls => {
                self.barrier()?;
                changed = true;
            }
            Input::Focus(focus) => {
                if focus.is_some_and(|id| {
                    id != COMPOSER
                        && id != SUBMIT
                        && (id != MODAL || self.composition != Composition::Demo)
                }) {
                    return Err(UiError::InvalidInput);
                }
                if focus.is_some_and(|id| id == COMPOSER || id == SUBMIT)
                    && (!self.composer_visible
                        || self.control.suspended
                        || !self.window_focused
                        || self.control.modal_open)
                {
                    return Err(UiError::InvalidInput);
                }
                let next = focus.map(|id| WidgetRef {
                    id,
                    generation: if id == COMPOSER {
                        self.generation
                    } else {
                        WidgetGeneration(1)
                    },
                });
                if next != self.control.focused_widget {
                    self.composer.cancel_preedit()?;
                    self.control.focused_widget = next;
                    self.barrier()?;
                    changed = true;
                }
            }
            Input::Modal(open) => {
                if self.control.modal_open != open {
                    self.composer.cancel_preedit()?;
                    self.control.modal_open = open;
                    self.control.focused_widget = None;
                    self.barrier()?;
                    changed = true;
                }
            }
            Input::Suspend(suspended) => {
                if self.control.suspended != suspended {
                    self.composer.cancel_preedit()?;
                    self.control.suspended = suspended;
                    self.control.focused_widget = None;
                    self.barrier()?;
                    changed = true;
                }
            }
            Input::WindowFocused(focused) => {
                if self.window_focused != focused {
                    self.composer.cancel_preedit()?;
                    self.window_focused = focused;
                    self.control.focused_widget = None;
                    self.barrier()?;
                    changed = true;
                }
            }
            Input::ComposerVisible(visible) => {
                if self.composer_visible != visible {
                    self.composer.cancel_preedit()?;
                    self.generation = self.generation.next()?;
                    self.composer_visible = visible;
                    self.control.focused_widget = None;
                    self.barrier()?;
                    changed = true;
                }
            }
            Input::Key(event) => {
                if !event.pressed {
                    let owned = self
                        .keys
                        .release(event.physical)
                        .unwrap_or(!self.control.world_input_allowed);
                    return Ok(if owned {
                        Disposition::Consumed
                    } else {
                        Disposition::Unhandled
                    });
                }
                let owned = if let Some((owned, quarantined)) = self.keys.state(event.physical) {
                    if quarantined || !event.repeat {
                        return Ok(if owned {
                            Disposition::Consumed
                        } else {
                            Disposition::Unhandled
                        });
                    }
                    // Repeats belong to the original press, even if later routing changes.
                    owned
                } else {
                    let owned = !self.control.world_input_allowed;
                    self.keys.press(event.physical, owned);
                    owned
                };
                if !owned {
                    return Ok(Disposition::Unhandled);
                }
                if event.key == Key::Enter
                    && (event.platform_composing || self.composer.is_composing())
                {
                    self.keys.fence_enter(event.physical);
                }
                if event.platform_composing {
                    // Native composition owns this press through release, including
                    // host helper effects; do not invent an empty preedit model.
                    self.keys.quarantine(event.physical);
                    return Ok(Disposition::Consumed);
                }
                if self
                    .control
                    .focused_widget
                    .is_some_and(|widget| widget.id == SUBMIT)
                    && event.key == Key::Enter
                    && !event.repeat
                {
                    self.submit()?;
                }
                if self.control.focused_widget == Some(self.composer_widget()) {
                    match event.key {
                        Key::Enter => {
                            if !event.repeat {
                                self.submit()?;
                            }
                        }
                        Key::Backspace if !self.composer.is_composing() => {
                            self.composer.backspace()?;
                        }
                        Key::Delete if !self.composer.is_composing() => self.composer.delete()?,
                        Key::SelectAll if !self.composer.is_composing() => {
                            self.composer.select_all()?;
                        }
                        Key::Escape => self.cancel_composition()?,
                        _ => (),
                    }
                }
            }
            Input::ReplaceText {
                target,
                text,
                selection,
            } => {
                self.require_target(target)?;
                self.composer.cancel_preedit()?;
                self.composer.select_all()?;
                self.composer.input(&text)?;
                self.composer
                    .set_selection(selection, self.composer.revision())?;
            }
            Input::TextCommit { target, text } => {
                self.require_target(target)?;
                self.composer.input(&text)?;
            }
            Input::Preedit {
                target,
                text,
                start,
                length,
            } => {
                self.require_target(target)?;
                self.composer.update_preedit(&text, start, length)?;
            }
            Input::CancelComposition { target } => {
                self.require_target(target)?;
                self.cancel_composition()?;
            }
            Input::Selection { target, selection } => {
                self.require_target(target)?;
                self.composer.set_selection(selection, target.revision)?;
            }
            Input::Edit { target, command } => {
                self.require_target(target)?;
                match command {
                    TextCommand::Backspace => self.composer.backspace()?,
                    TextCommand::Delete => self.composer.delete()?,
                    TextCommand::SelectAll => self.composer.select_all()?,
                }
            }
            Input::Activate(id) => {
                if self.control.suspended || !self.window_focused || self.control.modal_open {
                    return Err(UiError::Stale);
                }
                if id == SUBMIT && self.composer_visible {
                    self.submit()?;
                } else {
                    return Err(UiError::InvalidInput);
                }
            }
            Input::ReceiveMessage(text) => {
                if self.composition == Composition::SingleField {
                    return Err(UiError::InvalidInput);
                }
                // One history row is one line: no line or paragraph separators.
                if text.chars().any(refuses_single_line) {
                    return Err(UiError::RefusedText);
                }
                if text.len() > self.limits.message_bytes {
                    return Err(UiError::LimitExceeded);
                }
                while self.history.len() >= self.limits.history_messages
                    || self.history_bytes + text.len() > self.limits.history_bytes
                {
                    if let Some(old) = self.history.pop_front() {
                        self.history_bytes -= old.len();
                    }
                }
                self.history_bytes += text.len();
                self.history.push_back(text);
                self.scroll = self.scroll.min(self.history.len().saturating_sub(1));
                changed = true;
            }
            Input::Scroll(delta) => {
                if self.composition == Composition::SingleField {
                    return Err(UiError::InvalidInput);
                }
                let scroll = self
                    .scroll
                    .saturating_add_signed(delta as isize)
                    .min(self.history.len().saturating_sub(1));
                changed = scroll != self.scroll;
                self.scroll = scroll;
            }
        }
        if changed || old_text != self.composer.revision() {
            self.revision = self.revision.next()?;
        }
        Ok(Disposition::Consumed)
    }
}

/// Logical coordinates supplied by the host; no glyph metrics are inferred here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}
impl Viewport {
    pub fn new(width: f32, height: f32, scale: f32) -> Result<Self, UiError> {
        if [width, height, scale]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
            Err(UiError::InvalidInput)
        } else {
            Ok(Self {
                width,
                height,
                scale,
            })
        }
    }
    pub fn validate(self) -> Result<Self, UiError> {
        Self::new(self.width, self.height, self.scale)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl Rect {
    /// Validate coordinates, nonnegative extents, and finite endpoints.
    pub fn validate(self) -> Result<Self, UiError> {
        if [
            self.x,
            self.y,
            self.width,
            self.height,
            self.x + self.width,
            self.y + self.height,
        ]
        .iter()
        .any(|value| !value.is_finite())
            || self.width < 0.0
            || self.height < 0.0
        {
            Err(UiError::InvalidInput)
        } else {
            Ok(self)
        }
    }
    pub fn intersection(self, other: Self) -> Result<Self, UiError> {
        self.validate()?;
        other.validate()?;
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Ok(Self {
            x,
            y,
            width: ((self.x + self.width).min(other.x + other.width) - x).max(0.0),
            height: ((self.y + self.height).min(other.y + other.height) - y).max(0.0),
        })
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        x.is_finite()
            && y.is_finite()
            && x >= self.x
            && y >= self.y
            && x < self.x + self.width
            && y < self.y + self.height
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontRole {
    Body,
    Composer,
    Button,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    Rectangle {
        bounds: Rect,
        clip: Rect,
        color: [f32; 4],
    },
    Text {
        widget: WidgetRef,
        bounds: Rect,
        clip: Rect,
        text: String,
        font: FontRole,
        color: [f32; 4],
        text_revision: Option<TextRevision>,
    },
    World {
        bounds: Rect,
        clip: Rect,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct HitRegion {
    pub widget: WidgetRef,
    pub bounds: Rect,
    pub clip: Rect,
}
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub struct Layout {
    pub ui_revision: UiRevision,
    pub viewport: Viewport,
    pub paint: Vec<Paint>,
    pub hits: Vec<HitRegion>,
    pub composer_bounds: Option<Rect>,
    // The validated style this layout was made with; private so a compile
    // can never read a style other than the one the layout applied.
    field_style: FieldStyle,
}
impl Layout {
    /// The single field's style this layout was laid out with
    /// (`LayoutOptions::field_style`; the default for [`layout_in`] and
    /// [`layout`]). A compiler draws the composer's text with it, so the
    /// text size, padding and colours a layout was made with are the ones
    /// its frame is shaped, measured, hit-tested and drawn with.
    pub fn field_style(&self) -> &FieldStyle {
        &self.field_style
    }
}
// Written out so a layout with the default style prints as 0.1.1 printed it.
impl std::fmt::Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut out = f.debug_struct("Layout");
        out.field("ui_revision", &self.ui_revision)
            .field("viewport", &self.viewport)
            .field("paint", &self.paint)
            .field("hits", &self.hits)
            .field("composer_bounds", &self.composer_bounds);
        if self.field_style != FieldStyle::default() {
            out.field("field_style", &self.field_style);
        }
        out.finish()
    }
}
/// The single neutral layout used by all presenters. Text is unshaped.
// This declarative bounded prototype layout is intentionally kept in paint order.
#[allow(clippy::too_many_lines)]
fn demo_layout(snapshot: &UiSnapshot, viewport: Viewport) -> Result<Layout, UiError> {
    let viewport = viewport.validate()?;
    let full = Rect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    };
    let panel = Rect {
        x: 0.0,
        y: viewport.height * 0.55,
        width: viewport.width,
        height: viewport.height * 0.45,
    };
    let composer = Rect {
        x: 8.0_f32.min(viewport.width),
        y: (viewport.height - 44.0).max(panel.y),
        width: (viewport.width - 104.0).max(0.0),
        height: 36.0_f32.min(panel.height),
    };
    let submit = Rect {
        x: (viewport.width - 88.0).max(0.0),
        y: composer.y,
        width: 80.0_f32.min(viewport.width),
        height: composer.height,
    };
    let world = Rect {
        height: panel.y,
        ..full
    };
    let singleton = |id| WidgetRef {
        id,
        generation: WidgetGeneration(1),
    };
    let mut paint = vec![
        Paint::World {
            bounds: world,
            clip: full,
        },
        Paint::Rectangle {
            bounds: panel,
            clip: full,
            color: [0.08, 0.09, 0.12, 0.95],
        },
    ];
    let mut hits = vec![HitRegion {
        widget: singleton(WORLD),
        bounds: world,
        clip: full,
    }];
    hits.push(HitRegion {
        widget: singleton(HISTORY),
        bounds: panel,
        clip: full,
    });
    let history_clip = Rect {
        height: (composer.y - panel.y - 8.0).max(0.0),
        ..panel
    };
    let mut y = composer.y - 26.0;
    for text in snapshot
        .messages
        .iter()
        .rev()
        .skip(snapshot.scroll)
        .take(64)
    {
        if y < panel.y {
            break;
        }
        paint.push(Paint::Text {
            widget: singleton(HISTORY),
            bounds: Rect {
                x: 8.0,
                y,
                width: (viewport.width - 16.0).max(0.0),
                height: 22.0,
            },
            clip: history_clip,
            text: text.clone(),
            font: FontRole::Body,
            color: [0.9, 0.9, 0.95, 1.0],
            text_revision: None,
        });
        y -= 22.0;
    }
    if snapshot.composer_visible {
        paint.push(Paint::Rectangle {
            bounds: composer,
            clip: full,
            color: [0.18, 0.2, 0.25, 1.0],
        });
        paint.push(Paint::Text {
            widget: snapshot.composer_widget,
            bounds: composer,
            clip: composer,
            text: snapshot.composer.display_text.clone(),
            font: FontRole::Composer,
            color: [1.0; 4],
            text_revision: Some(snapshot.composer.revision),
        });
        paint.push(Paint::Rectangle {
            bounds: submit,
            clip: full,
            color: [0.2, 0.35, 0.6, 1.0],
        });
        paint.push(Paint::Text {
            widget: singleton(SUBMIT),
            bounds: submit,
            clip: submit,
            text: "Send".into(),
            font: FontRole::Button,
            color: [1.0; 4],
            text_revision: None,
        });
        hits.push(HitRegion {
            widget: snapshot.composer_widget,
            bounds: composer,
            clip: full,
        });
        hits.push(HitRegion {
            widget: singleton(SUBMIT),
            bounds: submit,
            clip: full,
        });
    }
    if snapshot.control.modal_open {
        paint.push(Paint::Rectangle {
            bounds: full,
            clip: full,
            color: [0.02, 0.02, 0.03, 0.85],
        });
        paint.push(Paint::Text {
            widget: singleton(MODAL),
            bounds: panel,
            clip: full,
            text: "Modal open".into(),
            font: FontRole::Body,
            color: [1.0; 4],
            text_revision: None,
        });
        hits.clear();
        hits.push(HitRegion {
            widget: singleton(MODAL),
            bounds: full,
            clip: full,
        });
    }
    Ok(Layout {
        ui_revision: snapshot.revision,
        viewport,
        paint,
        hits,
        composer_bounds: snapshot.composer_visible.then_some(composer),
        field_style: FieldStyle::default(),
    })
}

/// Validate a host attachment and return its effective clip. Bounds must lie
/// inside the viewport; the clip may extend beyond them and is intersected.
pub fn validate_layout(viewport: Viewport, bounds: Rect, clip: Rect) -> Result<Rect, UiError> {
    viewport.validate()?;
    bounds.validate()?;
    clip.validate()?;
    if bounds.x < 0.0
        || bounds.y < 0.0
        || bounds.x + bounds.width > viewport.width
        || bounds.y + bounds.height > viewport.height
    {
        return Err(UiError::InvalidInput);
    }
    bounds.intersection(clip)
}

/// The legacy full-viewport convenience layout.
pub fn layout(snapshot: &UiSnapshot, viewport: Viewport) -> Result<Layout, UiError> {
    let full = Rect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    };
    layout_in(snapshot, viewport, full, full)
}

/// Choices for [`layout_in_with`]. Start from `LayoutOptions::default()`,
/// which is what [`layout_in`] uses, and set the fields to change; later
/// versions may add fields with defaults that keep today's layout.
///
/// ```
/// use tilewright_ui::{Composition, Limits, LayoutOptions, Rect, Ui, Viewport, layout_in_with};
///
/// let ui = Ui::with_composition(Limits::default(), Composition::SingleField)?;
/// let mut options = LayoutOptions::default();
/// options.submit_control = false;
/// let viewport = Viewport::new(320.0, 200.0, 1.0)?;
/// let bounds = Rect { x: 0.0, y: 0.0, width: 320.0, height: 52.0 };
/// let layout = layout_in_with(&ui.snapshot(), viewport, bounds, bounds, &options)?;
/// assert!(layout.hits.iter().all(|hit| hit.widget.id != tilewright_ui::SUBMIT));
/// # Ok::<(), tilewright_ui::UiError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LayoutOptions {
    /// Lay out the submit control: Apply in [`Composition::SingleField`],
    /// Send in [`Composition::ChatPanel`]. Default `true`.
    ///
    /// With `false` the composer takes the whole inner width of the
    /// attachment and the layout holds no submit paint and no submit hit
    /// region. Submitting still works: Enter does, and so does
    /// `Input::Activate(SUBMIT)`, which a host with its own control sends.
    /// Only the embedded compositions have the option; with
    /// [`Composition::Demo`], `false` is refused with `UiError::InvalidInput`.
    pub submit_control: bool,
    /// How the single field's composer is sized and drawn. Default
    /// `FieldStyle::default()`, the 0.1.1 look. Only
    /// [`Composition::SingleField`] takes another style; the other
    /// compositions refuse one with `UiError::InvalidInput`.
    pub field_style: FieldStyle,
}
impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            submit_control: true,
            field_style: FieldStyle::default(),
        }
    }
}

/// The single field's metrics and colours, in [`LayoutOptions`]. Start from
/// `FieldStyle::default()`, which lays out and draws the field exactly as
/// 0.1.1 did, and set the fields to change; later versions may add fields
/// with defaults that keep today's look.
///
/// The layout uses the geometry (`inset`, `height`) and the text and box
/// colours, and keeps the whole style in the [`Layout`]
/// ([`Layout::field_style`]); the compiler reads the text size, the padding
/// and the selection, caret and underline colours from there. So one style
/// governs shaping, measurement, hit-testing, the caret, the input method
/// area and caret steps of a given layout.
///
/// [`layout_in_with`] refuses a style with `UiError::InvalidInput` when a
/// length is not finite, is negative, or is over 8192 (the largest logical
/// viewport the renderer accepts), when `height` is zero, when `text_size`
/// is outside 1–256 (the renderer's shaping range), or when a colour channel
/// is not finite or outside 0–1.
///
/// ```
/// use tilewright_ui::{Composition, FieldStyle, Limits, LayoutOptions, Rect, Ui, Viewport};
///
/// let ui = Ui::with_composition(Limits::default(), Composition::SingleField)?;
/// // A flat 30 px row with 18 px text, drawn on the row's own background.
/// let mut style = FieldStyle::default();
/// style.text_size = 18.0;
/// style.inset = 0.0;
/// style.height = 30.0;
/// style.padding_left = 4.0;
/// style.padding_top = 1.0;
/// style.box_color = Some([0.12, 0.32, 0.47, 1.0]);
/// let mut options = LayoutOptions::default();
/// options.submit_control = false;
/// options.field_style = style;
/// let viewport = Viewport::new(320.0, 200.0, 1.0)?;
/// let row = Rect { x: 10.0, y: 40.0, width: 300.0, height: 30.0 };
/// let layout = tilewright_ui::layout_in_with(&ui.snapshot(), viewport, row, row, &options)?;
/// assert_eq!(layout.composer_bounds, Some(row));
/// assert_eq!(layout.field_style().text_size, 18.0);
/// # Ok::<(), tilewright_ui::UiError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct FieldStyle {
    /// The composer's text size in logical pixels. Default `20.0`.
    pub text_size: f32,
    /// The composer box's height; it is at most the attachment's height
    /// less twice the inset, and sits at the bottom of the attachment.
    /// Default `36.0`.
    pub height: f32,
    /// The space between the attachment's edges and the composer box (and
    /// the submit control); at most half the attachment's width and height.
    /// Default `8.0`.
    pub inset: f32,
    /// The space from the box's left edge to the text's origin. Default `0.0`.
    pub padding_left: f32,
    /// The space from the box's top edge to the text's origin (the top of its
    /// line). Default `0.0`.
    pub padding_top: f32,
    /// When the text scrolls to keep the caret in view, the caret's right
    /// edge stays at least this far from the box's right edge. Default `0.0`.
    ///
    /// Text, selection, caret and underline are clipped to the box, not to
    /// the padding: scrolled text shows in the padding as in the rest of
    /// the box.
    pub padding_right: f32,
    /// The composer's text. Default white, `[1.0; 4]`.
    pub text_color: [f32; 4],
    /// The composer box's fill, or `None` for no box: the host draws the
    /// background itself. Default `Some([0.18, 0.2, 0.25, 1.0])`.
    pub box_color: Option<[f32; 4]>,
    /// The selection's highlight, under the text. Default
    /// `[0.23, 0.40, 0.62, 1.0]`.
    pub selection_color: [f32; 4],
    /// The caret, over the text; `None` draws it in the text's colour.
    /// Default `None`.
    pub caret_color: Option<[f32; 4]>,
    /// The input method preedit's underline, over the text; `None` draws it
    /// in the text's colour. Default `None`.
    pub preedit_color: Option<[f32; 4]>,
}
impl Default for FieldStyle {
    fn default() -> Self {
        Self {
            text_size: 20.0,
            height: 36.0,
            inset: 8.0,
            padding_left: 0.0,
            padding_top: 0.0,
            padding_right: 0.0,
            text_color: [1.0; 4],
            box_color: Some([0.18, 0.2, 0.25, 1.0]),
            selection_color: [0.23, 0.40, 0.62, 1.0],
            caret_color: None,
            preedit_color: None,
        }
    }
}
impl FieldStyle {
    /// The largest length a style takes, in logical pixels.
    const MAXIMUM_LENGTH: f32 = 8192.0;

    fn validate(&self) -> Result<(), UiError> {
        let length =
            |value: f32| value.is_finite() && (0.0..=Self::MAXIMUM_LENGTH).contains(&value);
        let color = |value: &[f32; 4]| {
            value
                .iter()
                .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
        };
        let lengths = [
            self.height,
            self.inset,
            self.padding_left,
            self.padding_top,
            self.padding_right,
        ];
        let colors = [
            Some(self.text_color),
            self.box_color,
            Some(self.selection_color),
        ]
        .into_iter()
        .chain([self.caret_color, self.preedit_color])
        .flatten();
        if lengths.into_iter().all(length)
            && self.height > 0.0
            && self.text_size.is_finite()
            && (1.0..=256.0).contains(&self.text_size)
            && colors.into_iter().all(|value| color(&value))
        {
            Ok(())
        } else {
            Err(UiError::InvalidInput)
        }
    }
}

/// Layout in host logical coordinates, with every paint and hit clipped to the
/// attachment. Embedded modes never describe a world viewport or host overlay.
/// It is [`layout_in_with`] with `LayoutOptions::default()`.
pub fn layout_in(
    snapshot: &UiSnapshot,
    viewport: Viewport,
    bounds: Rect,
    clip: Rect,
) -> Result<Layout, UiError> {
    layout_in_with(snapshot, viewport, bounds, clip, &LayoutOptions::default())
}

/// [`layout_in`] with [`LayoutOptions`]; for example, a single field without
/// its Apply control for a host that draws its own.
pub fn layout_in_with(
    snapshot: &UiSnapshot,
    viewport: Viewport,
    bounds: Rect,
    clip: Rect,
    options: &LayoutOptions,
) -> Result<Layout, UiError> {
    let clip = validate_layout(viewport, bounds, clip)?;
    if snapshot.composition == Composition::Demo && !options.submit_control {
        return Err(UiError::InvalidInput);
    }
    options.field_style.validate()?;
    if snapshot.composition != Composition::SingleField
        && options.field_style != FieldStyle::default()
    {
        return Err(UiError::InvalidInput);
    }
    if bounds.width == 0.0 || bounds.height == 0.0 {
        return Ok(Layout {
            ui_revision: snapshot.revision,
            viewport,
            paint: Vec::new(),
            hits: Vec::new(),
            composer_bounds: None,
            field_style: options.field_style.clone(),
        });
    }
    if snapshot.composition == Composition::Demo {
        let mut layout = demo_layout(
            snapshot,
            Viewport {
                width: bounds.width,
                height: bounds.height,
                scale: viewport.scale,
            },
        )?;
        let translate = |rect: &mut Rect| {
            rect.x += bounds.x;
            rect.y += bounds.y;
        };
        for paint in &mut layout.paint {
            let (paint_bounds, paint_clip) = match paint {
                Paint::Rectangle { bounds, clip, .. }
                | Paint::Text { bounds, clip, .. }
                | Paint::World { bounds, clip } => (bounds, clip),
            };
            translate(paint_bounds);
            translate(paint_clip);
            *paint_clip = paint_clip.intersection(clip)?;
        }
        for hit in &mut layout.hits {
            translate(&mut hit.bounds);
            translate(&mut hit.clip);
            hit.clip = hit.clip.intersection(clip)?;
        }
        if let Some(composer) = &mut layout.composer_bounds {
            translate(composer);
        }
        layout.viewport = viewport;
        return Ok(layout);
    }
    embedded_layout(snapshot, viewport, bounds, clip, options)
}

// Keep the bounded declarative paint sequence in presenter order.
#[allow(clippy::too_many_lines)]
fn embedded_layout(
    snapshot: &UiSnapshot,
    viewport: Viewport,
    bounds: Rect,
    clip: Rect,
    options: &LayoutOptions,
) -> Result<Layout, UiError> {
    let singleton = |id| WidgetRef {
        id,
        generation: WidgetGeneration(1),
    };
    let style = &options.field_style;
    let inset = style.inset.min(bounds.width / 2.0).min(bounds.height / 2.0);
    let inner_width = (bounds.width - inset * 2.0).max(0.0);
    let inner_height = (bounds.height - inset * 2.0).max(0.0);
    let height = style.height.min(inner_height);
    let button_width = if options.submit_control {
        80.0_f32.min(inner_width)
    } else {
        0.0
    };
    let gap = if options.submit_control {
        8.0_f32.min((inner_width - button_width).max(0.0))
    } else {
        0.0
    };
    let composer = Rect {
        x: bounds.x + inset,
        y: bounds.y + bounds.height - inset - height,
        width: (inner_width - button_width - gap).max(0.0),
        height,
    };
    let submit = Rect {
        x: composer.x + composer.width + gap,
        width: button_width,
        ..composer
    };
    let mut paint = Vec::new();
    let mut hits = Vec::new();
    if snapshot.composition == Composition::ChatPanel {
        paint.push(Paint::Rectangle {
            bounds,
            clip,
            color: [0.08, 0.09, 0.12, 0.95],
        });
        let history = Rect {
            height: (composer.y - bounds.y - inset).max(0.0),
            ..bounds
        };
        let history_clip = history.intersection(clip)?;
        hits.push(HitRegion {
            widget: singleton(HISTORY),
            bounds: history,
            clip: history_clip,
        });
        let mut y = composer.y - 26.0;
        for text in snapshot
            .messages
            .iter()
            .rev()
            .skip(snapshot.scroll)
            .take(64)
        {
            if y < bounds.y {
                break;
            }
            paint.push(Paint::Text {
                widget: singleton(HISTORY),
                bounds: Rect {
                    x: bounds.x + inset,
                    y,
                    width: inner_width,
                    height: 22.0,
                },
                clip: history_clip,
                text: text.clone(),
                font: FontRole::Body,
                color: [0.9, 0.9, 0.95, 1.0],
                text_revision: None,
            });
            y -= 22.0;
        }
    }
    if snapshot.composer_visible {
        if let Some(color) = style.box_color {
            paint.push(Paint::Rectangle {
                bounds: composer,
                clip,
                color,
            });
        }
        paint.push(Paint::Text {
            widget: snapshot.composer_widget,
            bounds: composer,
            clip: composer.intersection(clip)?,
            text: snapshot.composer.display_text.clone(),
            font: FontRole::Composer,
            color: style.text_color,
            text_revision: Some(snapshot.composer.revision),
        });
        if options.submit_control {
            paint.push(Paint::Rectangle {
                bounds: submit,
                clip,
                color: [0.2, 0.35, 0.6, 1.0],
            });
            paint.push(Paint::Text {
                widget: singleton(SUBMIT),
                bounds: submit,
                clip: submit.intersection(clip)?,
                text: if snapshot.composition == Composition::SingleField {
                    "Apply"
                } else {
                    "Send"
                }
                .into(),
                font: FontRole::Button,
                color: [1.0; 4],
                text_revision: None,
            });
        }
        hits.push(HitRegion {
            widget: snapshot.composer_widget,
            bounds: composer,
            clip,
        });
        if options.submit_control {
            hits.push(HitRegion {
                widget: singleton(SUBMIT),
                bounds: submit,
                clip,
            });
        }
    }
    if snapshot.control.modal_open {
        hits.clear();
    }
    Ok(Layout {
        ui_revision: snapshot.revision,
        viewport,
        paint,
        hits,
        composer_bounds: snapshot.composer_visible.then_some(composer),
        field_style: options.field_style.clone(),
    })
}

#[cfg(test)]
mod embedding_identity_tests {
    use super::*;

    #[test]
    fn binding_exhaustion_never_replaces_text_or_reuses_identities() {
        for exhausted in 0..4 {
            let mut ui = Ui::with_composition(Limits::default(), Composition::SingleField).unwrap();
            ui.bind_field("before").unwrap();
            match exhausted {
                0 => ui.generation = WidgetGeneration(u64::MAX),
                1 => ui.revision = UiRevision(u64::MAX),
                2 => ui.control.epoch = ControlEpoch(u64::MAX),
                _ => {
                    // TextInput cannot have its private revision overwritten here;
                    // advancing other identities still must precede all commits.
                    ui.next_batch = BatchId(u64::MAX);
                    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
                    let before = ui.snapshot();
                    assert_eq!(ui.apply(Input::Activate(SUBMIT)), Err(UiError::Exhausted));
                    assert_eq!(ui.snapshot(), before);
                    assert!(ui.peek_actions().is_none());
                    continue;
                }
            }
            let before = ui.snapshot();
            assert_eq!(ui.bind_field("after"), Err(UiError::Exhausted));
            assert_eq!(ui.snapshot(), before);
        }
    }
}
