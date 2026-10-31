// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Bounded host-independent chat and single-line editing semantics.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
mod text;
pub use text::{
    CompositionSnapshot, TextAffinity, TextCursor, TextInput, TextInputSnapshot, TextSelection,
    scalar_selection_bytes,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiError {
    InvalidInput,
    LimitExceeded,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    SubmitChat { text: String },
}
#[derive(Clone, Debug, Eq, PartialEq)]
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
pub struct ControlSnapshot {
    pub epoch: ControlEpoch,
    pub world_input_allowed: bool,
    pub focused_widget: Option<WidgetRef>,
    pub modal_open: bool,
    pub suspended: bool,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Key {
    Enter,
    Backspace,
    Delete,
    SelectAll,
    Escape,
    Other(u16),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub key: Key,
    pub pressed: bool,
    pub repeat: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextCommand {
    Backspace,
    Delete,
    SelectAll,
}
#[derive(Clone, Debug, PartialEq)]
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
pub struct UiSnapshot {
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
    keys: Vec<(Key, bool, bool)>,
    enter_fenced: bool,
}
impl Ui {
    pub fn new(limits: Limits) -> Result<Self, UiError> {
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
            keys: Vec::new(),
            enter_fenced: false,
        })
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
    pub fn text_target(&self) -> Option<TextTarget> {
        (self.control.focused_widget == Some(self.composer_widget())).then(|| TextTarget {
            widget: self.composer_widget(),
            control_epoch: self.control.epoch,
            revision: self.composer.revision(),
        })
    }
    pub fn control(&self) -> ControlSnapshot {
        self.control
    }
    pub fn snapshot(&self) -> UiSnapshot {
        UiSnapshot {
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
        let is_key = matches!(input, Input::Key(_));
        let mut next = self.clone();
        match next.apply_inner(input) {
            Ok(result) => {
                *self = next;
                Ok(result)
            }
            Err(error) => {
                if is_key {
                    self.keys = next.keys;
                    self.enter_fenced = next.enter_fenced;
                }
                Err(error)
            }
        }
    }
    fn barrier(&mut self) -> Result<(), UiError> {
        self.control.epoch = self.control.epoch.next()?;
        for (_, owned, quarantined) in &mut self.keys {
            *owned = true;
            *quarantined = true;
        }
        self.control.world_input_allowed = self.window_focused
            && !self.control.suspended
            && !self.control.modal_open
            && self.control.focused_widget.is_none();
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
        if self.composer.is_composing() || self.enter_fenced {
            return Ok(());
        }
        if self.composer.text().is_empty() {
            return Ok(());
        }
        if self.composer.text().len() > self.limits.message_bytes {
            return Err(UiError::LimitExceeded);
        }
        if self.queue.len() == self.limits.queued_batches {
            return Err(UiError::QueueFull);
        }
        let following = self.next_batch.next()?;
        let text = self.composer.text().to_owned();
        self.composer.select_all()?;
        self.composer.input("")?;
        self.queue.push_back(ActionBatch {
            id: self.next_batch,
            actions: vec![Action::SubmitChat { text }],
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
                if focus.is_some_and(|id| id != COMPOSER && id != SUBMIT && id != MODAL) {
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
                        .iter()
                        .position(|(key, ..)| *key == event.key)
                        .map_or(!self.control.world_input_allowed, |index| {
                            self.keys.remove(index).1
                        });
                    if event.key == Key::Enter {
                        self.enter_fenced = false;
                    }
                    return Ok(if owned {
                        Disposition::Consumed
                    } else {
                        Disposition::Unhandled
                    });
                }
                let owned = if let Some((_, owned, quarantined)) =
                    self.keys.iter().find(|(key, ..)| *key == event.key)
                {
                    if *quarantined || !event.repeat {
                        return Ok(if *owned {
                            Disposition::Consumed
                        } else {
                            Disposition::Unhandled
                        });
                    }
                    // Repeats belong to the original press, even if later routing changes.
                    *owned
                } else {
                    if self.keys.len() >= 256 {
                        return Err(UiError::LimitExceeded);
                    }
                    let owned = !self.control.world_input_allowed;
                    self.keys.push((event.key, owned, false));
                    owned
                };
                if !owned {
                    return Ok(Disposition::Unhandled);
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
                            if self.composer.is_composing() {
                                self.enter_fenced = true;
                            } else if !event.repeat {
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
                        Key::Escape => self.composer.cancel_preedit()?,
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
                self.composer.cancel_preedit()?;
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
                if text.len() > self.limits.message_bytes || text.chars().any(char::is_control) {
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
pub struct HitRegion {
    pub widget: WidgetRef,
    pub bounds: Rect,
    pub clip: Rect,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub ui_revision: UiRevision,
    pub viewport: Viewport,
    pub paint: Vec<Paint>,
    pub hits: Vec<HitRegion>,
    pub composer_bounds: Option<Rect>,
}
/// The single neutral layout used by all presenters. Text is unshaped.
// This declarative bounded prototype layout is intentionally kept in paint order.
#[allow(clippy::too_many_lines)]
pub fn layout(snapshot: &UiSnapshot, viewport: Viewport) -> Result<Layout, UiError> {
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
            widget: singleton(WidgetId(5)),
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
    })
}
