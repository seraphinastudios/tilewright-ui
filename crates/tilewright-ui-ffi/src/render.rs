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
                tilewright_ui_render::UiLimits::default(),
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
            let viewport = viewport_from_c(read_sized(viewport)?)?;
            let image = prepare_image(engine, Some(viewport))?;
            output.write(image);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_render_prepare_current(
    context: *mut Context,
    output: *mut Image,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |engine| {
            check_size(output)?;
            let image = prepare_image(engine, None)?;
            output.write(image);
            Ok(())
        })
    }
}

/// Both entry points validate output bounds before retaining a prepared frame.
fn prepare_image(
    engine: &mut Engine,
    requested: Option<tilewright_app::Viewport>,
) -> Result<Image> {
    if engine.renderer.is_none() {
        return Err(INVALID_ARGUMENT);
    }
    let viewport = requested.or_else(|| engine.app.viewport()).ok_or(STALE)?;
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
    let prepared = if requested.is_some() {
        engine.app.prepare(viewport)
    } else {
        engine.app.prepare_current()
    }
    .map_err(app_error)?;
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
    let image = Image {
        struct_size: size_of::<Image>() as u32,
        width: pixels.width,
        height: pixels.height,
        stride: pixels.width * 4,
        byte_length: pixels.rgba8.len() as u64,
        stamp: stamp_to_c(prepared.stamp),
    };
    engine.frames.push(Frame {
        stamp: prepared.stamp,
        compiled,
        pixels: pixels.rgba8,
        was_displayed: false,
    });
    Ok(image)
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

pub(super) fn select_at(
    engine: &mut Engine,
    stamp: tilewright_app::PresentationStamp,
    x: f32,
    y: f32,
) -> Result<bool> {
    let Some(frame) = engine.frames.iter().find(|frame| frame.stamp == stamp) else {
        return Ok(false);
    };
    let Some(text) = frame.compiled.editable() else {
        return Ok(false);
    };
    let Some(cursor) = text.hit([x, y]) else {
        return Ok(false);
    };
    let cursor = text
        .snapshot
        .as_ref()
        .ok_or(STALE)?
        .committed_cursor(cursor)
        .map_err(ui_error)?;
    let target = engine.ui().text_target().ok_or(STALE)?;
    engine
        .select_text(stamp, target, tilewright_ui::TextSelection::caret(cursor))
        .map_err(app_error)?;
    Ok(true)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn twui_move_visual(
    context: *mut Context,
    stamp: *const Stamp,
    physical_key: u32,
    motion: u32,
    extend: u32,
) -> u32 {
    // SAFETY: all foreign pointers obey the public ABI contract.
    unsafe {
        with_context(context, |engine| {
            use tilewright_ui_render::VisualMotion;
            let physical = u16::try_from(physical_key).map_err(|_| INVALID_ARGUMENT)?;
            if !engine.ui().can_apply_key_effect(physical) {
                return Err(STALE);
            }
            let motion = match motion {
                1 => VisualMotion::Left,
                2 => VisualMotion::Right,
                3 => VisualMotion::Home,
                4 => VisualMotion::End,
                _ => return Err(INVALID_ARGUMENT),
            };
            if extend > 1 {
                return Err(INVALID_ARGUMENT);
            }
            let stamp = stamp_from_c(read_sized(stamp)?)?;
            engine.app.validate_displayed(stamp).map_err(app_error)?;
            let target = engine.ui().text_target().ok_or(STALE)?;
            let frame = engine
                .frames
                .iter()
                .find(|frame| frame.stamp == stamp)
                .ok_or(STALE)?;
            let text = frame.compiled.editable().ok_or(STALE)?;
            let snapshot = text.snapshot.as_ref().ok_or(STALE)?;
            if snapshot.composition.is_some() {
                return Err(STALE);
            }
            let cursor = text.move_visual(snapshot.selection.focus, motion);
            let selection = tilewright_ui::TextSelection {
                anchor: if extend == 1 {
                    snapshot.selection.anchor
                } else {
                    cursor
                },
                focus: cursor,
            };
            engine
                .select_text(stamp, target, selection)
                .map_err(app_error)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_fonts_pixels_and_geometry_keep_presentation_identity() {
        let root = std::path::PathBuf::from(
            std::env::var_os("TILEWRIGHT_UI_FONT_ROOT")
                .expect("explicit verified test font root required"),
        );
        let entries = tilewright_ui_render::unicode_fonts::FONTS;
        let bytes: Vec<Vec<u8>> = entries
            .iter()
            .map(|font| {
                let data = std::fs::read(root.join(font.path)).unwrap();
                assert_eq!(data.len(), font.bytes);
                data
            })
            .collect();
        let mut fonts: Vec<Font> = entries
            .iter()
            .zip(&bytes)
            .map(|(entry, data)| Font {
                struct_size: size_of::<Font>() as u32,
                reserved: 0,
                family: entry.family.as_ptr(),
                family_bytes: entry.family.len() as u64,
                bytes: data.as_ptr(),
                byte_length: data.len() as u64,
                sha256: entry.sha256,
            })
            .collect();
        let config = Config {
            struct_size: size_of::<Config>() as u32,
            abi_version: ABI_VERSION,
            message_bytes: 4096,
            history_messages: 16,
            history_bytes: 8192,
            composer_bytes: 4096,
            queued_batches: 2,
            prepared_frames: 2,
        };
        let mut context = std::ptr::null_mut();
        // SAFETY: all pointers in this test refer to live, nonaliasing local
        // storage of sufficient extent. Font backing storage survives the call.
        unsafe {
            assert_eq!(twui_create(&config, &mut context), OK);
            fonts[0].sha256[0] ^= 1;
            assert_eq!(
                twui_set_fonts(context, fonts.as_ptr(), fonts.len() as u32),
                INVALID_ARGUMENT
            );
            fonts[0].sha256[0] ^= 1;
            assert_eq!(
                twui_set_fonts(context, fonts.as_ptr(), fonts.len() as u32),
                OK
            );
            drop(fonts);
            drop(bytes); // subsequent rendering must use owned copies
            assert_eq!(
                with_context(context, |engine| {
                    use tilewright_ui::{COMPOSER, Input, Key, KeyEvent};
                    engine
                        .apply(Input::Key(KeyEvent {
                            physical: 80,
                            key: Key::Other,
                            pressed: true,
                            repeat: false,
                            platform_composing: false,
                        }))
                        .map_err(app_error)?;
                    engine
                        .apply(Input::Focus(Some(COMPOSER)))
                        .map_err(app_error)?;
                    engine
                        .apply(Input::Key(KeyEvent {
                            physical: 81,
                            key: Key::Other,
                            pressed: true,
                            repeat: false,
                            platform_composing: false,
                        }))
                        .map_err(app_error)?;
                    engine.apply(Input::Focus(None)).map_err(app_error)?;
                    engine
                        .apply(Input::Focus(Some(COMPOSER)))
                        .map_err(app_error)?;
                    let target = engine.ui().text_target().ok_or(STALE)?;
                    engine
                        .apply(tilewright_ui::Input::TextCommit {
                            target,
                            text: "A\u{301} אבג مرحبا 👩‍💻".into(),
                        })
                        .map_err(app_error)?;
                    Ok(())
                }),
                OK
            );
            let viewport = Viewport {
                struct_size: size_of::<Viewport>() as u32,
                reserved: 0,
                width: 640.0,
                height: 480.0,
                scale: 1.0,
                reserved2: 0,
            };
            let empty_stamp = Stamp {
                struct_size: size_of::<Stamp>() as u32,
                reserved: 0,
                surface: 0,
                frame: 0,
                geometry: 0,
                ui_revision: 0,
            };
            let mut first = Image {
                struct_size: size_of::<Image>() as u32,
                width: 0,
                height: 0,
                stride: 0,
                byte_length: 0,
                stamp: empty_stamp,
            };
            assert_eq!(twui_render_prepare(context, &viewport, &mut first), OK);
            let mut rect = Rect {
                struct_size: size_of::<Rect>() as u32,
                reserved: 0,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            };
            assert_eq!(twui_get_ime_rect(context, &first.stamp, &mut rect), STALE);
            let mut second = first;
            assert_eq!(twui_render_prepare(context, &viewport, &mut second), OK);
            let mut third = first;
            assert_eq!(
                twui_render_prepare(context, &viewport, &mut third),
                QUEUE_FULL
            );
            let mut required = 0;
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &second.stamp,
                    std::ptr::null_mut(),
                    0,
                    &mut required
                ),
                BUFFER_TOO_SMALL
            );
            assert_eq!(required, 640 * 480 * 4);
            let mut pixels = vec![0u8; required as usize];
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &second.stamp,
                    pixels.as_mut_ptr(),
                    required,
                    &mut required
                ),
                OK
            );
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
            assert_eq!(twui_ack_presented(context, &second.stamp), OK);
            assert_eq!(twui_get_ime_rect(context, &second.stamp, &mut rect), OK);
            assert!(rect.width > 0.0 && rect.height > 0.0);
            assert_eq!(twui_ack_presented(context, &first.stamp), STALE);
            assert_eq!(twui_discard_prepared(context, &first.stamp), OK);
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &first.stamp,
                    pixels.as_mut_ptr(),
                    required,
                    &mut required
                ),
                STALE
            );
            // Displayed geometry alone cannot authorize effects for absent,
            // world-owned or quarantined physical presses.
            assert_eq!(twui_move_visual(context, &second.stamp, 79, 1, 0), STALE);
            assert_eq!(twui_move_visual(context, &second.stamp, 80, 1, 0), STALE);
            assert_eq!(twui_move_visual(context, &second.stamp, 81, 1, 0), STALE);
            assert_eq!(
                with_context(context, |engine| {
                    engine
                        .apply(tilewright_ui::Input::Key(tilewright_ui::KeyEvent {
                            physical: 79,
                            key: tilewright_ui::Key::Other,
                            pressed: true,
                            repeat: false,
                            platform_composing: false,
                        }))
                        .map_err(app_error)?;
                    Ok(())
                }),
                OK
            );
            assert_eq!(twui_move_visual(context, &second.stamp, 79, 1, 0), OK);
            assert_eq!(twui_get_ime_rect(context, &second.stamp, &mut rect), STALE);
            assert_eq!(twui_invalidate_surface(context), OK);
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &second.stamp,
                    pixels.as_mut_ptr(),
                    required,
                    &mut required
                ),
                STALE
            );
            // Reuse the installed owned font pack in an independent embedded
            // context, so this proof cannot accidentally rely on demo geometry.
            let mut renderer = None;
            assert_eq!(
                with_context(context, |engine| {
                    renderer = engine.renderer.take();
                    Ok(())
                }),
                OK
            );
            assert_eq!(twui_destroy(context), OK);
            assert_eq!(twui_create_with_composition(&config, 2, &mut context), OK);
            assert_eq!(
                with_context(context, |engine| {
                    engine.renderer = renderer;
                    Ok(())
                }),
                OK
            );
            assert_eq!(twui_render_prepare_current(context, &mut first), STALE);
            let mut layout = Layout {
                struct_size: size_of::<Layout>() as u32,
                reserved: 0,
                viewport_width: 640.0,
                viewport_height: 480.0,
                viewport_scale: 1.0,
                bounds_x: 150.0,
                bounds_y: 90.0,
                bounds_width: 300.0,
                bounds_height: 80.0,
                clip_x: 150.0,
                clip_y: 90.0,
                clip_width: 300.0,
                clip_height: 80.0,
            };
            assert_eq!(twui_set_layout(context, &layout), OK);
            assert_eq!(
                with_context(context, |engine| {
                    engine.bind_field("embedded").map_err(app_error)?;
                    engine
                        .apply(tilewright_ui::Input::Focus(Some(tilewright_ui::COMPOSER)))
                        .map_err(app_error)?;
                    Ok(())
                }),
                OK
            );
            assert_eq!(twui_render_prepare_current(context, &mut first), OK);
            // An identical layout must retain unacknowledged pixel resources.
            assert_eq!(twui_set_layout(context, &layout), OK);
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &first.stamp,
                    std::ptr::null_mut(),
                    0,
                    &mut required
                ),
                BUFFER_TOO_SMALL
            );
            assert_eq!(twui_get_ime_rect(context, &first.stamp, &mut rect), STALE);
            assert_eq!(twui_ack_presented(context, &first.stamp), OK);
            assert_eq!(twui_get_ime_rect(context, &first.stamp, &mut rect), OK);
            assert!(rect.x >= 150.0 && rect.x < 450.0 && rect.y >= 90.0 && rect.y < 170.0);
            let mut embedded_pixels = vec![0; required as usize];
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &first.stamp,
                    embedded_pixels.as_mut_ptr(),
                    required,
                    &mut required
                ),
                OK
            );
            assert_eq!(&embedded_pixels[..4], &[0, 0, 0, 0]);
            assert!(embedded_pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
            layout.bounds_x += 20.0;
            layout.clip_x += 20.0;
            assert_eq!(twui_set_layout(context, &layout), OK);
            assert_eq!(twui_get_ime_rect(context, &first.stamp, &mut rect), STALE);
            assert_eq!(twui_ack_presented(context, &first.stamp), STALE);
            assert_eq!(
                twui_copy_rgba(
                    context,
                    &first.stamp,
                    std::ptr::null_mut(),
                    0,
                    &mut required
                ),
                STALE
            );
            assert_eq!(twui_render_prepare_current(context, &mut second), OK);
            assert_eq!(twui_ack_presented(context, &second.stamp), OK);
            assert_eq!(twui_get_ime_rect(context, &second.stamp, &mut rect), OK);
            assert!(rect.x >= 170.0 && rect.x < 470.0);
            assert_eq!(twui_destroy(context), OK);
        }
    }
}
