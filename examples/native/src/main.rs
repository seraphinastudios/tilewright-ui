// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
//! Intentionally bounded CPU reference demonstration, not a GPU-performance proof.
mod input;
use input::HostInput;
use sdl3::{
    event::Event,
    keyboard::{Keycode, Mod, TextInputUtil},
    mouse::MouseButton,
    pixels::PixelFormat,
    rect::Rect,
    render::{BlendMode, WindowCanvas},
    video::Window,
};
use std::{error::Error, io::Read, sync::Arc, time::Duration};
use tilewright_app::App;
use tilewright_ui::{Action, COMPOSER, Input, Limits, TextSelection, Viewport};
use tilewright_ui_render::{
    CompiledFrame, FontBlob, FontPack, Namespace, TextGeometry, TextLimits, UiLimits, UiRenderer,
    VisualMotion,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let (fonts, max_frames) = arguments()?;
    let limits = TextLimits::default();
    let mut renderer = UiRenderer::new(
        FontPack::new(fonts, limits)?,
        limits,
        Namespace {
            first_semantic: 1000,
            solid_resource: 1000,
            atlas_resource: 1001,
        },
        UiLimits::default(),
    )?;
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let window = video
        .window("Tilewright shared UI — CPU reference / Rust SDL3", 800, 600)
        .resizable()
        .build()?;
    let mut canvas = window.into_canvas();
    let window_id = canvas.window().id();
    let text_input = video.text_input();
    let clipboard = video.clipboard();
    let mut events = sdl.event_pump()?;
    let mut app = App::new(Limits::default())?;
    for text in [
        "Shared UI / caller-owned SDL3",
        "Tab: chat focus · F1: modal · Ctrl+A/C/V: clipboard",
        "Neutral procedural map. CPU reference rendering.",
    ] {
        app.apply(Input::ReceiveMessage(text.into()))?;
    }
    app.apply(Input::Focus(Some(COMPOSER)))?;
    let mut input = HostInput::default();
    let mut displayed_render = None;
    let mut running = true;
    let mut frame_count = 0_u64;
    while running {
        synchronize_text(&mut input, &app, &text_input, canvas.window())?;
        for event in events.poll_iter() {
            if let Err(error) = clipboard_event(&mut app, &event, window_id, &clipboard) {
                eprintln!("clipboard rejected: {error}");
            }
            geometry_event(&mut app, &event, window_id, displayed_render.as_ref());
            match input.event(&mut app, &event, window_id) {
                Ok(keep) => running &= keep,
                Err(error) => eprintln!("input rejected: {error}"),
            }
            synchronize_text(&mut input, &app, &text_input, canvas.window())?;
        }
        while let Some(batch) = app.ui().peek_actions().cloned() {
            // Local echo is the host effect; acknowledge after acceptance.
            for action in batch.actions {
                let Action::SubmitChat { text } = action;
                app.apply(Input::ReceiveMessage(text))?;
            }
            app.ack_actions(batch.id)?;
        }
        if !running {
            break;
        }
        if let Some(compiled) = present_frame(&mut app, &mut renderer, &mut canvas)? {
            // Candidate geometry becomes usable only after successful present.
            if input.text_active
                && let Some(caret) = compiled.editable().and_then(TextGeometry::caret)
            {
                text_input.set_rect(canvas.window(), candidate_rect(caret), 0);
            }
            displayed_render = Some(compiled);
            frame_count += 1;
        }
        if max_frames.is_some_and(|limit| frame_count >= limit) {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    text_input.stop(canvas.window());
    println!("presented_frames={frame_count}");
    Ok(())
}

fn arguments() -> Result<(Vec<FontBlob>, Option<u64>)> {
    let mut args = std::env::args().skip(1);
    let mut fonts = Vec::new();
    let mut max_frames = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--font" => {
                let path = args.next().ok_or("--font requires PATH SHA256")?;
                let hex = args.next().ok_or("--font requires PATH SHA256")?;
                let mut bytes = Vec::new();
                std::fs::File::open(&path)?.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > 32 * 1024 * 1024 || fonts.len() >= 32 { return Err("font budget exceeded".into()); }
                fonts.push(FontBlob { family: format!("explicit-{}", fonts.len()), bytes: Arc::from(bytes), sha256: digest(&hex)? });
            }
            "--frames" => max_frames = Some(args.next().ok_or("missing frame count")?.parse::<u64>()?),
            _ => return Err("usage: tilewright-ui-native-demo --font PATH SHA256 [--font PATH SHA256] [--frames N]".into()),
        }
    }
    Ok((fonts, max_frames))
}
fn synchronize_text(
    input: &mut HostInput,
    app: &App,
    util: &TextInputUtil,
    window: &Window,
) -> Result<()> {
    // Millisecond wrapper rounds up, conservatively rejecting already queued
    // events at a session boundary. Event timestamps themselves are nanoseconds.
    if input.synchronize(
        app,
        sdl3::timer::ticks()
            .saturating_add(1)
            .saturating_mul(1_000_000),
    ) {
        util.stop(window);
        if input.text_active {
            util.start(window);
            if !util.is_active(window) {
                return Err(sdl3::get_error().into());
            }
        }
    }
    Ok(())
}
fn clipboard_event(
    app: &mut App,
    event: &Event,
    window_id: u32,
    clipboard: &sdl3::clipboard::ClipboardUtil,
) -> Result<()> {
    let Event::KeyDown {
        window_id: id,
        keycode: Some(key),
        keymod,
        repeat: false,
        ..
    } = event
    else {
        return Ok(());
    };
    if *id != window_id || !keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
        return Ok(());
    }
    let Some(target) = app.ui().text_target() else {
        return Ok(());
    };
    match key {
        Keycode::C | Keycode::X => {
            let snapshot = app.ui().snapshot();
            let selection = snapshot.composer.selection.range();
            if !selection.is_empty() && snapshot.composer.composition.is_none() {
                clipboard.set_clipboard_text(&snapshot.composer.committed[selection])?;
                if *key == Keycode::X {
                    app.apply(Input::TextCommit {
                        target,
                        text: String::new(),
                    })?;
                }
            }
        }
        Keycode::V => {
            let text = clipboard.clipboard_text()?;
            app.apply(Input::TextCommit { target, text })?;
        }
        _ => (),
    }
    Ok(())
}
fn geometry_event(app: &mut App, event: &Event, window_id: u32, compiled: Option<&CompiledFrame>) {
    let (Some(stamp), Some(editable), Some(target)) = (
        app.displayed().map(|frame| frame.stamp),
        compiled.and_then(CompiledFrame::editable),
        app.ui().text_target(),
    ) else {
        return;
    };
    let Some(snapshot) = &editable.snapshot else {
        return;
    };
    match event {
        Event::KeyDown {
            window_id: id,
            keycode: Some(key),
            keymod,
            ..
        } if *id == window_id && snapshot.composition.is_none() => {
            let motion = match key {
                Keycode::Left => VisualMotion::Left,
                Keycode::Right => VisualMotion::Right,
                Keycode::Home => VisualMotion::Home,
                Keycode::End => VisualMotion::End,
                _ => return,
            };
            let focus = editable.move_visual(snapshot.selection.focus, motion);
            let anchor = if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                snapshot.selection.anchor
            } else {
                focus
            };
            let _ = app.select_text(stamp, target, TextSelection { anchor, focus });
        }
        Event::MouseButtonDown {
            window_id: id,
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } if *id == window_id => {
            if let Some(cursor) = editable.hit([*x, *y])
                && let Ok(cursor) = snapshot.committed_cursor(cursor)
            {
                let _ = app.select_text(stamp, target, TextSelection::caret(cursor));
            }
        }
        _ => (),
    }
}
fn present_frame(
    app: &mut App,
    renderer: &mut UiRenderer,
    canvas: &mut WindowCanvas,
) -> Result<Option<CompiledFrame>> {
    let (width, height) = canvas.window().size();
    if width == 0 || height == 0 {
        return Ok(None);
    }
    if width > 1600 || height > 1200 {
        return Err("reference viewport exceeds 1600x1200".into());
    }
    // Bounded dimensions fit u16 and therefore convert to f32 exactly.
    let viewport = Viewport::new(
        f32::from(u16::try_from(width)?),
        f32::from(u16::try_from(height)?),
        1.0,
    )?;
    let prepared = app.prepare(viewport)?;
    let compiled = renderer.compile(&prepared.snapshot, &prepared.layout, [width, height])?;
    let mut output = compiled.standalone([0.0; 4])?.render_rgba()?;
    composite_map(&mut output.rgba8, width);
    let creator = canvas.texture_creator();
    let mut texture = creator.create_texture_streaming(PixelFormat::RGBA32, width, height)?;
    texture.set_blend_mode(BlendMode::None);
    texture.update(None, &output.rgba8, width as usize * 4)?;
    canvas.copy(&texture, None, None)?;
    if !canvas.present() {
        app.discard_prepared(prepared.stamp)?;
        return Err(sdl3::get_error().into());
    }
    app.ack_presented(prepared.stamp)?;
    Ok(Some(compiled))
}
fn composite_map(pixels: &mut [u8], width: u32) {
    // Reference pixels are premultiplied. Blend onto the independently authored
    // map here, then upload an opaque texture without a second alpha operation.
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let x = index % width as usize;
        let y = index / width as usize;
        let map = if (x / 40 + y / 32).is_multiple_of(3) {
            [39_u32, 73, 66]
        } else {
            [31_u32, 60, 56]
        };
        let inverse = 255 - u32::from(pixel[3]);
        for channel in 0..3 {
            pixel[channel] = u8::try_from(
                (u32::from(pixel[channel]) + (map[channel] * inverse + 127) / 255).min(255),
            )
            .expect("clamped channel");
        }
        pixel[3] = 255;
    }
}
// Renderer caret rectangles are finite, clipped and bounded to the 1600x1200
// viewport. Floor/ceil conservatively enclose them in SDL integer coordinates.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn candidate_rect(caret: tilewright_ui::Rect) -> Rect {
    Rect::new(
        caret.x.floor() as i32,
        caret.y.floor() as i32,
        caret.width.ceil().max(1.0) as u32,
        caret.height.ceil().max(1.0) as u32,
    )
}
fn digest(hex: &str) -> Result<[u8; 32]> {
    if hex.len() != 64 || !hex.is_ascii() {
        return Err("SHA256 must contain 64 hexadecimal digits".into());
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)?;
    }
    Ok(result)
}
