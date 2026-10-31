// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT
//! Optional, bounded CPU reference presentation through the shared renderer.
use super::*;
use std::sync::Arc;
use tilewright_ui_render::{CompiledFrame, FontBlob, FontPack, Namespace, TextLimits, UiRenderer};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Font {
    pub struct_size: u32,
    pub reserved: u32,
    pub family: *const u8,
    pub family_bytes: u64,
    pub bytes: *const u8,
    pub byte_length: u64,
    pub sha256: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Image {
    pub struct_size: u32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub byte_length: u64,
    pub stamp: Stamp,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rect {
    pub struct_size: u32,
    pub reserved: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
pub(super) struct Frame {
    pub stamp: tilewright_app::PresentationStamp,
    pub compiled: CompiledFrame,
    pub pixels: Vec<u8>,
    pub was_displayed: bool,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_set_fonts(
    context: *mut Context,
    fonts: *const Font,
    count: u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the header's nonaliasing span contract.
    unsafe {
        with_context(context, |engine| {
            if engine.renderer.is_some() || fonts.is_null() || count == 0 || count > 32 {
                return Err(INVALID_ARGUMENT);
            }
            let mut total = 0u64;
            let mut copied = Vec::new();
            for index in 0..count as usize {
                let font = read_sized(fonts.add(index))?;
                if font.struct_size as usize != size_of::<Font>()
                    || font.reserved != 0
                    || font.family.is_null()
                    || font.bytes.is_null()
                    || font.family_bytes == 0
                    || font.family_bytes > 128
                    || font.byte_length == 0
                    || font.byte_length > 32 * 1024 * 1024
                {
                    return Err(INVALID_ARGUMENT);
                }
                total += font.byte_length;
                if total > 64 * 1024 * 1024 {
                    return Err(LIMIT_EXCEEDED);
                }
                let family = std::str::from_utf8(std::slice::from_raw_parts(
                    font.family,
                    font.family_bytes as usize,
                ))
                .map_err(|_| INVALID_ARGUMENT)?
                .to_owned();
                let bytes: Arc<[u8]> =
                    std::slice::from_raw_parts(font.bytes, font.byte_length as usize).into();
                copied.push(FontBlob {
                    family,
                    bytes,
                    sha256: font.sha256,
                });
            }
            let limits = TextLimits::default();
            let pack = FontPack::new(copied, limits).map_err(|_| INVALID_ARGUMENT)?;
            let renderer = UiRenderer::new(
                pack,
                limits,
                Namespace {
                    first_semantic: 1_000_000,
                    solid_resource: 1_000_000,
                    atlas_resource: 1_000_001,
                },
                tilewright_render_ui::UiLimits::default(),
            )
            .map_err(|_| INVALID_ARGUMENT)?;
            engine.renderer = Some(renderer);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_render_prepare(
    context: *mut Context,
    viewport: *const Viewport,
    output: *mut Image,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |engine| {
            check_size(output)?;
            if engine.renderer.is_none() {
                return Err(INVALID_ARGUMENT);
            }
            let viewport = viewport_from_c(read_sized(viewport)?)?;
            let physical = [
                viewport.width * viewport.scale,
                viewport.height * viewport.scale,
            ];
            if physical
                .iter()
                .any(|v| !v.is_finite() || *v < 1.0 || *v > 4096.0 || v.fract() != 0.0)
            {
                return Err(INVALID_ARGUMENT);
            }
            let physical = physical.map(|v| v as u32);
            if u64::from(physical[0]) * u64::from(physical[1]) > 4 * 1024 * 1024 {
                return Err(LIMIT_EXCEEDED);
            }
            let prepared = engine.app.prepare(viewport).map_err(app_error)?;
            engine
                .frames
                .retain(|frame| frame.stamp.surface == prepared.stamp.surface);
            let rendered = (|| {
                let compiled = engine
                    .renderer
                    .as_mut()
                    .ok_or(INVALID_ARGUMENT)?
                    .compile(&prepared.snapshot, &prepared.layout, physical)
                    .map_err(|_| 13u32)?;
                let pixels = compiled
                    .standalone([0.0; 4])
                    .map_err(|_| 13u32)?
                    .render_rgba()
                    .map_err(|_| 13u32)?;
                Ok::<_, u32>((compiled, pixels))
            })();
            let (compiled, pixels) = match rendered {
                Ok(value) => value,
                Err(error) => {
                    engine
                        .app
                        .discard_prepared(prepared.stamp)
                        .map_err(app_error)?;
                    return Err(error);
                }
            };
            output.write(Image {
                struct_size: size_of::<Image>() as u32,
                width: pixels.width,
                height: pixels.height,
                stride: pixels.width * 4,
                byte_length: pixels.rgba8.len() as u64,
                stamp: stamp_to_c(prepared.stamp),
            });
            engine.frames.push(Frame {
                stamp: prepared.stamp,
                compiled,
                pixels: pixels.rgba8,
                was_displayed: false,
            });
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_copy_rgba(
    context: *mut Context,
    stamp: *const Stamp,
    buffer: *mut u8,
    capacity: u64,
    required: *mut u64,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |engine| {
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            let frame = engine
                .frames
                .iter()
                .find(|frame| frame.stamp == stamp)
                .ok_or(STALE)?;
            copy_bytes(&frame.pixels, buffer, capacity, required)
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_get_ime_rect(
    context: *mut Context,
    stamp: *const Stamp,
    output: *mut Rect,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |engine| {
            check_size(output)?;
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            let displayed = engine.app.validate_displayed(stamp).map_err(app_error)?;
            if displayed.snapshot.control.focused_widget != Some(displayed.snapshot.composer_widget)
            {
                return Err(EMPTY);
            }
            let frame = engine
                .frames
                .iter()
                .find(|frame| frame.stamp == stamp)
                .ok_or(STALE)?;
            let caret = frame
                .compiled
                .editable()
                .and_then(|text| text.caret())
                .ok_or(EMPTY)?;
            output.write(Rect {
                struct_size: size_of::<Rect>() as u32,
                reserved: 0,
                x: caret.x,
                y: caret.y,
                width: caret.width,
                height: caret.height,
            });
            Ok(())
        })
    }
}
