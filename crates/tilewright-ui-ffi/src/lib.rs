// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! C facade; see `include/tilewright_ui.h` for the complete caller contract.
//!
//! # Safety
//! Every exported pointer argument must satisfy that header's validity,
//! alignment, nonaliasing and lifetime requirements. Checking nulls and lengths
//! cannot establish that arbitrary foreign addresses reference accessible memory.
#![allow(clippy::missing_safety_doc)]

use std::cell::UnsafeCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, ThreadId};
use tilewright_app::App;

pub const ABI_VERSION: u32 = 2;
pub const OK: u32 = 0;
pub const INVALID_ARGUMENT: u32 = 1;
pub const ABI_MISMATCH: u32 = 2;
pub const LIMIT_EXCEEDED: u32 = 3;
pub const QUEUE_FULL: u32 = 4;
pub const STALE: u32 = 5;
pub const EXHAUSTED: u32 = 6;
pub const BUFFER_TOO_SMALL: u32 = 7;
pub const EMPTY: u32 = 8;
pub const WRONG_THREAD: u32 = 9;
pub const REENTRANT: u32 = 10;
pub const POISONED: u32 = 11;
pub const INTERNAL_PANIC: u32 = 12;

type Result<T> = std::result::Result<T, u32>;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Config {
    pub struct_size: u32,
    pub abi_version: u32,
    pub message_bytes: u32,
    pub history_messages: u32,
    pub history_bytes: u32,
    pub composer_bytes: u32,
    pub queued_batches: u32,
    pub prepared_frames: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Input {
    pub struct_size: u32,
    pub kind: u32,
    pub value: u32,
    pub reserved: u32,
    pub text: *const u8,
    pub text_len: u64,
    pub anchor: u64,
    pub caret: u64,
    pub widget_generation: u64,
    pub control_epoch: u64,
    pub text_revision: u64,
    pub delta: i32,
    pub key_repeat: u32,
    pub physical_key: u32,
    pub platform_composing: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub struct_size: u32,
    pub focused: u32,
    pub modal_open: u32,
    pub suspended: u32,
    pub ui_revision: u64,
    pub text_revision: u64,
    pub anchor: u64,
    pub caret: u64,
    pub history_messages: u32,
    pub scroll: u32,
    pub composer_bytes: u64,
    pub preedit_bytes: u64,
    pub widget_generation: u64,
    pub control_epoch: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Control {
    pub struct_size: u32,
    pub world_input_allowed: u32,
    pub modal_open: u32,
    pub suspended: u32,
    pub epoch: u64,
    pub widget_id: u64,
    pub widget_generation: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Action {
    pub struct_size: u32,
    pub kind: u32,
    pub batch_id: u64,
    pub text_bytes: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Stamp {
    pub struct_size: u32,
    pub reserved: u32,
    pub surface: u64,
    pub frame: u64,
    pub geometry: u64,
    pub ui_revision: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Viewport {
    pub struct_size: u32,
    pub reserved: u32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub reserved2: u32,
}

/// Host attachment in logical coordinates. No existing ABI POD is extended.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Layout {
    pub struct_size: u32,
    pub reserved: u32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub viewport_scale: f32,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_width: f32,
    pub bounds_height: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub clip_width: f32,
    pub clip_height: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct WidgetRef {
    pub struct_size: u32,
    pub reserved: u32,
    pub widget_id: u64,
    pub widget_generation: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FieldAction {
    pub struct_size: u32,
    pub kind: u32,
    pub batch_id: u64,
    pub text_bytes: u64,
    pub widget_id: u64,
    pub widget_generation: u64,
    pub text_revision: u64,
}

#[cfg(feature = "cpu-prototype")]
mod render;
struct Engine {
    app: App,
    #[cfg(feature = "cpu-prototype")]
    renderer: Option<tilewright_ui_render::UiRenderer>,
    #[cfg(feature = "cpu-prototype")]
    frames: Vec<render::Frame>,
}
impl Engine {
    fn new(app: App) -> Self {
        Self {
            app,
            #[cfg(feature = "cpu-prototype")]
            renderer: None,
            #[cfg(feature = "cpu-prototype")]
            frames: Vec::new(),
        }
    }
    #[cfg(feature = "cpu-prototype")]
    fn prune_frames(&mut self) {
        let displayed = self.app.displayed().map(|frame| frame.stamp);
        self.frames
            .retain(|frame| Some(frame.stamp) == displayed || !frame.was_displayed);
    }
}
impl std::ops::Deref for Engine {
    type Target = App;
    fn deref(&self) -> &App {
        &self.app
    }
}
impl std::ops::DerefMut for Engine {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}

pub struct Context {
    owner: ThreadId,
    busy: AtomicBool,
    poisoned: AtomicBool,
    app: UnsafeCell<Engine>,
}

// Each raw pointer operation below follows the documented foreign caller
// obligations. Sized structs start with a valid readable u32 even when a caller
// advertises an older, smaller struct. Never read the full struct before sizing.
unsafe fn check_size<T>(ptr: *const T) -> Result<()> {
    if ptr.is_null() {
        return Err(INVALID_ARGUMENT);
    }
    // SAFETY: non-null foreign pointer with readable, aligned size prefix.
    let size = unsafe { ptr.cast::<u32>().read() };
    if (size as usize) < size_of::<T>() {
        return Err(ABI_MISMATCH);
    }
    Ok(())
}
unsafe fn read_sized<T: Copy>(ptr: *const T) -> Result<T> {
    // SAFETY: caller provides the size prefix and advertised readable extent.
    unsafe { check_size(ptr)? };
    // SAFETY: checked advertised extent, caller guarantees storage validity.
    Ok(unsafe { ptr.read() })
}
unsafe fn copy_bytes(
    text: &[u8],
    buffer: *mut u8,
    capacity: u64,
    required: *mut u64,
) -> Result<()> {
    if required.is_null() || capacity > isize::MAX as u64 || (buffer.is_null() && capacity != 0) {
        return Err(INVALID_ARGUMENT);
    }
    // SAFETY: required is caller-owned valid writable nonaliasing storage.
    unsafe { required.write(text.len() as u64) };
    if capacity < text.len() as u64 {
        return Err(BUFFER_TOO_SMALL);
    }
    if !text.is_empty() {
        // SAFETY: buffer has sufficient advertised writable capacity and does
        // not overlap library-owned text or any other argument.
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), buffer, text.len()) };
    }
    Ok(())
}
unsafe fn input_text(input: &Input, maximum: usize) -> Result<String> {
    // SAFETY: caller supplies the documented input span.
    unsafe { span_text(input.text, input.text_len, maximum) }
}
unsafe fn span_text(text: *const u8, length: u64, maximum: usize) -> Result<String> {
    let len = usize::try_from(length).map_err(|_| LIMIT_EXCEEDED)?;
    if len > maximum || len > isize::MAX as usize {
        return Err(LIMIT_EXCEEDED);
    }
    if len == 0 {
        return Ok(String::new());
    }
    if text.is_null() {
        return Err(INVALID_ARGUMENT);
    }
    // SAFETY: caller guarantees the readable nonaliasing span for this call.
    let bytes = unsafe { std::slice::from_raw_parts(text, len) };
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| INVALID_ARGUMENT)
}

unsafe fn with_context(context: *mut Context, f: impl FnOnce(&mut Engine) -> Result<()>) -> u32 {
    if context.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: the caller provides a live context returned by this library.
    let context = unsafe { &*context };
    if context.owner != thread::current().id() {
        return WRONG_THREAD;
    }
    if context.busy.swap(true, Ordering::Acquire) {
        return REENTRANT;
    }
    if context.poisoned.load(Ordering::Acquire) {
        context.busy.store(false, Ordering::Release);
        return POISONED;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: owning thread and exclusive busy guard exclude simultaneous
        // access; app references never escape the guarded call.
        f(unsafe { &mut *context.app.get() })
    }));
    let status = match result {
        Ok(result) => result.err().unwrap_or(OK),
        Err(_) => {
            context.poisoned.store(true, Ordering::Release);
            INTERNAL_PANIC
        }
    };
    context.busy.store(false, Ordering::Release);
    status
}

#[unsafe(no_mangle)]
pub extern "C" fn twui_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_destroy(context: *mut Context) -> u32 {
    if context.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: caller provides a live, uniquely destroyed library handle.
    let value = unsafe { &*context };
    if value.owner != thread::current().id() {
        return WRONG_THREAD;
    }
    if value.busy.swap(true, Ordering::Acquire) {
        return REENTRANT;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: matches the allocation in create; no references survive.
        drop(unsafe { Box::from_raw(context) });
    })) {
        Ok(()) => OK,
        Err(_) => INTERNAL_PANIC,
    }
}

fn ui_error(error: tilewright_ui::UiError) -> u32 {
    use tilewright_ui::UiError::*;
    match error {
        InvalidInput => INVALID_ARGUMENT,
        LimitExceeded => LIMIT_EXCEEDED,
        QueueFull => QUEUE_FULL,
        Stale => STALE,
        Exhausted => EXHAUSTED,
    }
}
fn app_error(error: tilewright_app::AppError) -> u32 {
    use tilewright_app::AppError::*;
    match error {
        Ui(error) => ui_error(error),
        Stale => STALE,
        QueueFull => QUEUE_FULL,
        Exhausted => EXHAUSTED,
    }
}
fn disposition(value: tilewright_ui::Disposition) -> u32 {
    u32::from(value == tilewright_ui::Disposition::Consumed)
}
fn target(input: &Input) -> Result<tilewright_ui::TextTarget> {
    use tilewright_ui::*;
    Ok(TextTarget {
        widget: WidgetRef {
            id: COMPOSER,
            generation: WidgetGeneration::new(input.widget_generation).map_err(ui_error)?,
        },
        control_epoch: ControlEpoch::new(input.control_epoch).map_err(ui_error)?,
        revision: TextRevision::new(input.text_revision).map_err(ui_error)?,
    })
}
fn stamp_to_c(value: tilewright_app::PresentationStamp) -> Stamp {
    Stamp {
        struct_size: size_of::<Stamp>() as u32,
        reserved: 0,
        surface: value.surface.get(),
        frame: value.frame.get(),
        geometry: value.geometry.get(),
        ui_revision: value.ui_revision.get(),
    }
}
fn stamp_from_c(value: Stamp) -> Result<tilewright_app::PresentationStamp> {
    use tilewright_app::*;
    if value.reserved != 0 {
        return Err(INVALID_ARGUMENT);
    }
    Ok(PresentationStamp {
        surface: SurfaceEpoch::new(value.surface).map_err(app_error)?,
        frame: FrameId::new(value.frame).map_err(app_error)?,
        geometry: GeometryEpoch::new(value.geometry).map_err(app_error)?,
        ui_revision: tilewright_ui::UiRevision::new(value.ui_revision).map_err(ui_error)?,
    })
}
fn viewport_from_c(value: Viewport) -> Result<tilewright_app::Viewport> {
    if value.reserved != 0 || value.reserved2 != 0 {
        return Err(INVALID_ARGUMENT);
    }
    Ok(tilewright_app::Viewport {
        width: value.width,
        height: value.height,
        scale: value.scale,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_create(config: *const Config, output: *mut *mut Context) -> u32 {
    // SAFETY: forwarded pointers retain the caller's documented obligations.
    unsafe { twui_create_with_composition(config, 0, output) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_create_with_composition(
    config: *const Config,
    composition: u32,
    output: *mut *mut Context,
) -> u32 {
    if output.is_null() {
        return INVALID_ARGUMENT;
    }
    // SAFETY: caller supplies writable nonaliasing output storage.
    unsafe { output.write(std::ptr::null_mut()) };
    match catch_unwind(AssertUnwindSafe(|| -> Result<()> {
        // SAFETY: caller supplies valid sized config storage.
        let config = unsafe { read_sized(config)? };
        if config.abi_version != ABI_VERSION {
            return Err(ABI_MISMATCH);
        }
        let composition = match composition {
            0 => tilewright_ui::Composition::Demo,
            1 => tilewright_ui::Composition::ChatPanel,
            2 => tilewright_ui::Composition::SingleField,
            _ => return Err(INVALID_ARGUMENT),
        };
        // Explicit allocation/work bounds for this prototype C surface.
        if [
            config.message_bytes,
            config.history_bytes,
            config.composer_bytes,
        ]
        .iter()
        .any(|n| *n > 16 * 1024 * 1024)
            || config.history_messages > 65536
            || config.queued_batches > 65536
            || config.prepared_frames > 64
        {
            return Err(LIMIT_EXCEEDED);
        }
        let app = App::with_composition(
            tilewright_ui::Limits {
                message_bytes: config.message_bytes as usize,
                history_messages: config.history_messages as usize,
                history_bytes: config.history_bytes as usize,
                composer_bytes: config.composer_bytes as usize,
                queued_batches: config.queued_batches as usize,
                prepared_frames: config.prepared_frames as usize,
            },
            composition,
        )
        .map_err(app_error)?;
        let context = Box::new(Context {
            owner: thread::current().id(),
            busy: AtomicBool::new(false),
            poisoned: AtomicBool::new(false),
            app: UnsafeCell::new(Engine::new(app)),
        });
        // SAFETY: transfers a library allocation's opaque handle to the caller.
        unsafe { output.write(Box::into_raw(context)) };
        Ok(())
    })) {
        Ok(result) => result.err().unwrap_or(OK),
        Err(_) => INTERNAL_PANIC,
    }
}

fn preedit_offset(value: u64) -> Result<i32> {
    if value == u64::MAX {
        Ok(-1)
    } else {
        i32::try_from(value).map_err(|_| INVALID_ARGUMENT)
    }
}

unsafe fn convert_input(input: Input, app: &App) -> Result<tilewright_ui::Input> {
    use tilewright_ui::{Input as I, *};
    if input.reserved != 0
        || input.key_repeat > 1
        || input.platform_composing > 1
        || (input.kind != 14 && input.key_repeat != 0)
    {
        return Err(INVALID_ARGUMENT);
    }
    let text_kind = matches!(input.kind, 1 | 2 | 12);
    let target_kind = matches!(input.kind, 1..=6);
    let offset_kind = matches!(input.kind, 2 | 4);
    let value_kind = matches!(input.kind, 8..=11 | 14 | 15);
    if (!text_kind && (!input.text.is_null() || input.text_len != 0))
        || (!target_kind
            && (input.widget_generation != 0
                || input.control_epoch != 0
                || input.text_revision != 0))
        || (!offset_kind && (input.anchor != 0 || input.caret != 0))
        || (!value_kind && input.value != 0)
        || (input.kind != 13 && input.delta != 0)
        || (!matches!(input.kind, 14 | 15)
            && (input.physical_key != 0 || input.platform_composing != 0))
    {
        return Err(INVALID_ARGUMENT);
    }
    let boolean = || match input.value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(INVALID_ARGUMENT),
    };
    let text = || {
        let limit = if input.kind == 12 {
            app.ui().limits().message_bytes
        } else {
            app.ui().limits().composer_bytes
        };
        // SAFETY: forwarded caller span obligations; bounded before reading.
        unsafe { input_text(&input, limit) }
    };
    Ok(match input.kind {
        1 => I::TextCommit {
            target: target(&input)?,
            text: text()?,
        },
        2 => I::Preedit {
            target: target(&input)?,
            text: text()?,
            start: preedit_offset(input.anchor)?,
            length: preedit_offset(input.caret)?,
        },
        3 => I::CancelComposition {
            target: target(&input)?,
        },
        4 => I::Selection {
            target: target(&input)?,
            selection: TextSelection {
                anchor: TextCursor::new(
                    usize::try_from(input.anchor).map_err(|_| INVALID_ARGUMENT)?,
                ),
                focus: TextCursor::new(usize::try_from(input.caret).map_err(|_| INVALID_ARGUMENT)?),
            },
        },
        5 | 6 => I::Edit {
            target: target(&input)?,
            command: if input.kind == 5 {
                TextCommand::Backspace
            } else {
                TextCommand::Delete
            },
        },
        7 => I::Activate(SUBMIT),
        8 => I::Focus(boolean()?.then_some(COMPOSER)),
        9 => I::Modal(boolean()?),
        10 => I::Suspend(boolean()?),
        11 => I::WindowFocused(boolean()?),
        12 => I::ReceiveMessage(text()?),
        13 => I::Scroll(input.delta),
        14 | 15 => I::Key(KeyEvent {
            physical: u16::try_from(input.physical_key).map_err(|_| INVALID_ARGUMENT)?,
            key: match input.value {
                1 => Key::Enter,
                2 => Key::Backspace,
                3 => Key::Delete,
                4 => Key::SelectAll,
                5 => Key::Escape,
                6 => Key::Other,
                _ => return Err(INVALID_ARGUMENT),
            },
            pressed: input.kind == 14,
            repeat: input.key_repeat != 0,
            platform_composing: input.platform_composing != 0,
        }),
        _ => return Err(INVALID_ARGUMENT),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_apply(
    context: *mut Context,
    input: *const Input,
    consumed: *mut u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            if consumed.is_null() {
                return Err(INVALID_ARGUMENT);
            }
            let input = convert_input(read_sized(input)?, app)?;
            let result = app.apply(input).map_err(app_error)?;
            consumed.write(disposition(result));
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_key_effect_allowed(
    context: *mut Context,
    physical_key: u32,
    allowed: *mut u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            if allowed.is_null() {
                return Err(INVALID_ARGUMENT);
            }
            let physical = u16::try_from(physical_key).map_err(|_| INVALID_ARGUMENT)?;
            allowed.write(u32::from(app.ui().can_apply_key_effect(physical)));
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_get_state(context: *mut Context, output: *mut State) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let snapshot = app.ui().snapshot();
            output.write(State {
                struct_size: size_of::<State>() as u32,
                focused: u32::from(snapshot.control.focused_widget.is_some()),
                modal_open: u32::from(snapshot.control.modal_open),
                suspended: u32::from(snapshot.control.suspended),
                ui_revision: snapshot.revision.get(),
                text_revision: snapshot.composer.revision.get(),
                anchor: snapshot.composer.selection.anchor.byte as u64,
                caret: snapshot.composer.selection.focus.byte as u64,
                history_messages: snapshot.messages.len() as u32,
                scroll: snapshot.scroll as u32,
                composer_bytes: snapshot.composer.committed.len() as u64,
                preedit_bytes: snapshot
                    .composer
                    .composition
                    .as_ref()
                    .map_or(0, |value| value.text.len()) as u64,
                widget_generation: snapshot.composer_widget.generation.get(),
                control_epoch: snapshot.control.epoch.get(),
            });
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_get_control(context: *mut Context, output: *mut Control) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let value = app.ui().control();
            output.write(Control {
                struct_size: size_of::<Control>() as u32,
                world_input_allowed: u32::from(value.world_input_allowed),
                modal_open: u32::from(value.modal_open),
                suspended: u32::from(value.suspended),
                epoch: value.epoch.get(),
                widget_id: value.focused_widget.map_or(0, |v| v.id.get()),
                widget_generation: value.focused_widget.map_or(0, |v| v.generation.get()),
            });
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_copy_text(
    context: *mut Context,
    source: u32,
    index: u32,
    buffer: *mut u8,
    capacity: u64,
    required: *mut u64,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            let snapshot = app.ui().snapshot();
            let text = match source {
                1 if index == 0 => snapshot.composer.committed.as_str(),
                2 if index == 0 => snapshot
                    .composer
                    .composition
                    .as_ref()
                    .map_or("", |v| v.text.as_str()),
                3 => snapshot
                    .messages
                    .get(index as usize)
                    .ok_or(INVALID_ARGUMENT)?
                    .as_str(),
                _ => return Err(INVALID_ARGUMENT),
            };
            copy_bytes(text.as_bytes(), buffer, capacity, required)
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_peek_action(context: *mut Context, output: *mut Action) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let batch = app.ui().peek_actions().ok_or(EMPTY)?;
            let (kind, text) = match &batch.actions[0] {
                tilewright_ui::Action::SubmitChat { text } => (1, text),
                tilewright_ui::Action::SubmitField { text, .. } => (2, text),
            };
            output.write(Action {
                struct_size: size_of::<Action>() as u32,
                kind,
                batch_id: batch.id.get(),
                text_bytes: text.len() as u64,
            });
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_copy_action(
    context: *mut Context,
    batch_id: u64,
    buffer: *mut u8,
    capacity: u64,
    required: *mut u64,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            let batch = app.ui().peek_actions().ok_or(EMPTY)?;
            if batch.id.get() != batch_id {
                return Err(STALE);
            }
            let text = match &batch.actions[0] {
                tilewright_ui::Action::SubmitChat { text }
                | tilewright_ui::Action::SubmitField { text, .. } => text,
            };
            copy_bytes(text.as_bytes(), buffer, capacity, required)
        })
    }
}
/// Read identity for the oldest field action without acknowledging it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_get_field_action(
    context: *mut Context,
    batch_id: u64,
    output: *mut FieldAction,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let batch = app.ui().peek_actions().ok_or(EMPTY)?;
            if batch.id.get() != batch_id {
                return Err(STALE);
            }
            let tilewright_ui::Action::SubmitField {
                widget,
                revision,
                text,
            } = &batch.actions[0]
            else {
                return Err(INVALID_ARGUMENT);
            };
            output.write(FieldAction {
                struct_size: size_of::<FieldAction>() as u32,
                kind: 2,
                batch_id,
                text_bytes: text.len() as u64,
                widget_id: widget.id.get(),
                widget_generation: widget.generation.get(),
                text_revision: revision.get(),
            });
            Ok(())
        })
    }
}

/// Rebind the single-field slot and return its new identity atomically.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_bind_field(
    context: *mut Context,
    text: *const u8,
    length: u64,
    output: *mut WidgetRef,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let text = span_text(text, length, app.ui().limits().composer_bytes)?;
            app.bind_field(text).map_err(app_error)?;
            let widget = app.ui().snapshot().composer_widget;
            output.write(WidgetRef {
                struct_size: size_of::<WidgetRef>() as u32,
                reserved: 0,
                widget_id: widget.id.get(),
                widget_generation: widget.generation.get(),
            });
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_set_layout(context: *mut Context, layout: *const Layout) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            let layout = read_sized(layout)?;
            if layout.reserved != 0 {
                return Err(INVALID_ARGUMENT);
            }
            let viewport = tilewright_app::Viewport {
                width: layout.viewport_width,
                height: layout.viewport_height,
                scale: layout.viewport_scale,
            };
            let bounds = tilewright_ui::Rect {
                x: layout.bounds_x,
                y: layout.bounds_y,
                width: layout.bounds_width,
                height: layout.bounds_height,
            };
            let clip = tilewright_ui::Rect {
                x: layout.clip_x,
                y: layout.clip_y,
                width: layout.clip_width,
                height: layout.clip_height,
            };
            app.set_layout(viewport, bounds, clip).map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            {
                let surface = app.surface_epoch();
                app.frames.retain(|frame| frame.stamp.surface == surface);
            }
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_prepare_current(context: *mut Context, output: *mut Stamp) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let frame = app.prepare_current().map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            app.frames
                .retain(|value| value.stamp.surface == frame.stamp.surface);
            output.write(stamp_to_c(frame.stamp));
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_ack_action(context: *mut Context, batch_id: u64) -> u32 {
    // SAFETY: context obeys the public ABI contract.
    unsafe {
        with_context(context, |app| {
            app.ack_actions(tilewright_ui::BatchId::new(batch_id).map_err(ui_error)?)
                .map_err(app_error)
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_prepare(
    context: *mut Context,
    viewport: *const Viewport,
    output: *mut Stamp,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            let viewport = viewport_from_c(read_sized(viewport)?)?;
            let frame = app.prepare(viewport).map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            app.frames
                .retain(|value| value.stamp.surface == frame.stamp.surface);
            output.write(stamp_to_c(frame.stamp));
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_ack_presented(context: *mut Context, stamp: *const Stamp) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            app.ack_presented(stamp).map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            {
                for frame in &mut app.frames {
                    if frame.stamp == stamp {
                        frame.was_displayed = true;
                    }
                }
                app.prune_frames();
            }
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_discard_prepared(context: *mut Context, stamp: *const Stamp) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            app.discard_prepared(stamp).map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            app.frames.retain(|frame| frame.stamp != stamp);
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_get_displayed(context: *mut Context, output: *mut Stamp) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            check_size(output)?;
            output.write(stamp_to_c(app.displayed().ok_or(EMPTY)?.stamp));
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_invalidate_surface(context: *mut Context) -> u32 {
    // SAFETY: context obeys the public ABI contract.
    unsafe {
        with_context(context, |app| {
            app.invalidate_surface().map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            app.frames.clear();
            Ok(())
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_pointer(
    context: *mut Context,
    stamp: *const Stamp,
    x: f32,
    y: f32,
    consumed: *mut u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            if consumed.is_null() || !x.is_finite() || !y.is_finite() {
                return Err(INVALID_ARGUMENT);
            }
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            let hit = app.hit_test(stamp, x, y).map_err(app_error)?;
            #[cfg(feature = "cpu-prototype")]
            if hit.is_some_and(|widget| widget.id == tilewright_ui::COMPOSER)
                && app.ui().text_target().is_some()
                && render::select_at(app, stamp, x, y)?
            {
                consumed.write(1);
                return Ok(());
            }
            let value = match hit.map(|v| v.id) {
                Some(tilewright_ui::COMPOSER) => disposition(
                    app.apply(tilewright_ui::Input::Focus(Some(tilewright_ui::COMPOSER)))
                        .map_err(app_error)?,
                ),
                Some(tilewright_ui::SUBMIT) => disposition(
                    app.apply(tilewright_ui::Input::Activate(tilewright_ui::SUBMIT))
                        .map_err(app_error)?,
                ),
                Some(tilewright_ui::WORLD) => {
                    app.apply(tilewright_ui::Input::Focus(None))
                        .map_err(app_error)?;
                    u32::from(!app.ui().control().world_input_allowed)
                }
                _ => 1,
            };
            consumed.write(value);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_scroll_at(
    context: *mut Context,
    stamp: *const Stamp,
    x: f32,
    y: f32,
    delta: i32,
    consumed: *mut u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |app| {
            if consumed.is_null() || !x.is_finite() || !y.is_finite() {
                return Err(INVALID_ARGUMENT);
            }
            let hit = app
                .hit_test(stamp_from_c(read_sized(stamp)?)?, x, y)
                .map_err(app_error)?;
            let value = match hit.map(|widget| widget.id) {
                Some(tilewright_ui::HISTORY) => disposition(
                    app.apply(tilewright_ui::Input::Scroll(delta))
                        .map_err(app_error)?,
                ),
                Some(tilewright_ui::WORLD) => u32::from(!app.ui().control().world_input_allowed),
                _ => 1,
            };
            consumed.write(value);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_poison_and_reentry_are_fenced() {
        let app = App::new(tilewright_ui::Limits::default()).unwrap();
        let raw = Box::into_raw(Box::new(Context {
            owner: thread::current().id(),
            busy: AtomicBool::new(true),
            poisoned: AtomicBool::new(false),
            app: UnsafeCell::new(Engine::new(app)),
        }));
        // SAFETY: these pointers refer to a live locally owned library context.
        unsafe {
            assert_eq!(with_context(raw, |_| Ok(())), REENTRANT);
            (*raw).busy.store(false, Ordering::Release);
            assert_eq!(
                with_context(raw, |_| panic!("injected internal failure")),
                INTERNAL_PANIC
            );
            assert_eq!(with_context(raw, |_| Ok(())), POISONED);
            assert_eq!(twui_destroy(raw), OK);
        }
    }
}
