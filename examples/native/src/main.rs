// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
//! Intentionally bounded CPU reference demonstration, not a GPU-performance proof.
mod input;
use input::HostInput;
use sdl3::{
    event::Event,
    keyboard::{Keycode, Mod},
    pixels::PixelFormat,
    rect::Rect,
    render::BlendMode,
};
use std::{error::Error, io::Read, sync::Arc, time::Duration};
use tilewright_app::App;
use tilewright_ui::{Action, COMPOSER, Input, Limits, TextSelection, Viewport};
use tilewright_ui_render::{
    FontBlob, FontPack, Namespace, TextLimits, UiLimits, UiRenderer, VisualMotion,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
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
                if bytes.len() > 32 * 1024 * 1024 { return Err("font exceeds bounded size".into()); }
                fonts.push(FontBlob { family: format!("explicit-{}", fonts.len()), bytes: Arc::from(bytes), sha256: digest(&hex)? });
            }
            "--frames" => max_frames = Some(args.next().ok_or("missing frame count")?.parse::<u64>()?),
            _ => return Err("usage: tilewright-ui-native-demo --font PATH SHA256 [--font PATH SHA256] [--frames N]".into()),
        }
    }
    let limits = TextLimits::default();
    let fonts = FontPack::new(fonts, limits)?;
    let mut renderer = UiRenderer::new(
        fonts,
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
        if input.synchronize(
            &app,
            sdl3::timer::ticks()
                .saturating_add(1)
                .saturating_mul(1_000_000),
        ) {
            text_input.stop(canvas.window());
            if input.text_active {
                text_input.start(canvas.window());
            }
        }
        for event in events.poll_iter() {
            // Clipboard is a host effect, requested only by a focused physical
            // shortcut. It never synthesizes an SDL TextInput event as well.
            if let Event::KeyDown {
                window_id: id,
                keycode: Some(key),
                keymod,
                repeat: false,
                ..
            } = &event
            {
                let effect = (|| -> Result<()> {
                    if *id == window_id && keymod.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD) {
                        if let Some(target) = app.ui().text_target() {
                            match key {
                                Keycode::C | Keycode::X => {
                                    let snapshot = app.ui().snapshot();
                                    let selection = snapshot.composer.selection.range();
                                    if !selection.is_empty()
                                        && snapshot.composer.composition.is_none()
                                    {
                                        clipboard.set_clipboard_text(
                                            &snapshot.composer.committed[selection],
                                        )?;
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
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = effect {
                    eprintln!("clipboard rejected: {error}");
                }
            }
            if let Event::KeyDown {
                window_id: id,
                keycode: Some(key),
                keymod,
                ..
            } = &event
            {
                if *id == window_id {
                    let motion = match key {
                        Keycode::Left => Some(VisualMotion::Left),
                        Keycode::Right => Some(VisualMotion::Right),
                        Keycode::Home => Some(VisualMotion::Home),
                        Keycode::End => Some(VisualMotion::End),
                        _ => None,
                    };
                    if let (Some(motion), Some(displayed), Some(compiled), Some(target)) = (
                        motion,
                        app.displayed(),
                        displayed_render.as_ref(),
                        app.ui().text_target(),
                    ) {
                        if let Some(editable) =
                            tilewright_ui_render::CompiledFrame::editable(compiled)
                        {
                            if let Some(snapshot) = &editable.snapshot {
                                if snapshot.composition.is_none() {
                                    let focus =
                                        editable.move_visual(snapshot.selection.focus, motion);
                                    let anchor =
                                        if keymod.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD) {
                                            snapshot.selection.anchor
                                        } else {
                                            focus
                                        };
                                    let _ = app.select_text(
                                        displayed.stamp,
                                        target,
                                        TextSelection { anchor, focus },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            // Pointer cursor geometry comes only from the acknowledged frame.
            if let Event::MouseButtonDown {
                window_id: id,
                x,
                y,
                ..
            } = &event
            {
                if *id == window_id {
                    if let (Some(displayed), Some(compiled), Some(target)) = (
                        app.displayed(),
                        displayed_render.as_ref(),
                        app.ui().text_target(),
                    ) {
                        if let Some(editable) =
                            tilewright_ui_render::CompiledFrame::editable(compiled)
                        {
                            if let Some(cursor) = editable.hit([*x, *y]) {
                                if let Some(snapshot) = &editable.snapshot {
                                    if let Ok(cursor) = snapshot.committed_cursor(cursor) {
                                        let _ = app.select_text(
                                            displayed.stamp,
                                            target,
                                            TextSelection::caret(cursor),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Err(error) = input
                .event(&mut app, &event, window_id)
                .map(|keep| running &= keep)
            {
                eprintln!("input rejected: {error}");
            }
            if input.synchronize(
                &app,
                sdl3::timer::ticks()
                    .saturating_add(1)
                    .saturating_mul(1_000_000),
            ) {
                text_input.stop(canvas.window());
                if input.text_active {
                    text_input.start(canvas.window());
                }
            }
        }
        while let Some(batch) = app.ui().peek_actions().cloned() {
            // Local echo is the demo's domain effect; acknowledgment follows its
            // successful acceptance, and no network delivery is implied.
            for action in batch.actions {
                let Action::SubmitChat { text } = action;
                app.apply(Input::ReceiveMessage(text))?;
            }
            app.ui_mut().ack_actions(batch.id)?;
        }
        if !running {
            break;
        }
        let (width, height) = canvas.window().size();
        if width == 0 || height == 0 {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        // Keep reference raster work bounded even for a resized external window.
        if width > 1600 || height > 1200 {
            return Err("reference viewport exceeds 1600x1200".into());
        }
        let prepared = app.prepare(Viewport::new(width as f32, height as f32, 1.0)?)?;
        let compiled = renderer.compile(&prepared.snapshot, &prepared.layout, [width, height])?;
        let mut output = compiled.standalone([0.0; 4])?.render_rgba()?;
        // CPU reference output is premultiplied. Composite the procedural
        // host map beneath it once, then submit an opaque SDL texture.
        for (index, pixel) in output.rgba8.chunks_exact_mut(4).enumerate() {
            let x = index % width as usize;
            let y = index / width as usize;
            let map = if (x / 40 + y / 32) % 3 == 0 {
                [39_u32, 73, 66]
            } else {
                [31_u32, 60, 56]
            };
            let inverse = 255 - u32::from(pixel[3]);
            for channel in 0..3 {
                pixel[channel] = (u32::from(pixel[channel]) + (map[channel] * inverse + 127) / 255)
                    .min(255) as u8;
            }
            pixel[3] = 255;
        }
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
        // This geometry was just committed by the sole SDL presenter.
        if input.text_active {
            if let Some(caret) = compiled.editable().and_then(|editable| editable.caret()) {
                text_input.set_rect(
                    canvas.window(),
                    Rect::new(
                        caret.x.floor() as i32,
                        caret.y.floor() as i32,
                        caret.width.ceil().max(1.0) as u32,
                        caret.height.ceil().max(1.0) as u32,
                    ),
                    0,
                );
            }
        }
        displayed_render = Some(compiled);
        frame_count += 1;
        if max_frames.is_some_and(|limit| frame_count >= limit) {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    text_input.stop(canvas.window());
    println!("presented_frames={frame_count}");
    Ok(())
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
