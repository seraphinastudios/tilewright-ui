// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Exact neutral paint translated through the existing Unicode renderer.
//! Retain frames with the application's presentation stamp; never use prepared
//! text geometry before its presentation has been acknowledged.
#![forbid(unsafe_code)]

mod resources;
use std::{collections::HashSet, fmt, sync::Arc};
use tilewright_render_resources::ResourceProvider;
pub use tilewright_render_ui::UiLimits;
pub use tilewright_render_ui::unicode::{
    FontBlob, FontPack, TextAffinity, TextCursor, TextLimits, UnicodeLayout, VisualMotion,
};
use tilewright_render_ui::unicode::{TextError, TextSystem};
use tilewright_render_ui::{
    UiDisplayList, UiPaint, UiPrimitive, UiRect, UiTexture, UiViewport, builtin,
};
use tilewright_scene::{Digest256, Quad, ResourceId, SceneBundle, SceneLimits, SemanticId};
use tilewright_ui::{FontRole, Layout, Paint, Rect, TextInputSnapshot, UiSnapshot, WidgetRef};

/// Caller-reserved identities, exclusive to this adapter for its entire life.
/// Never reuse an atlas ID for another engine while an old frame is retained.
#[derive(Clone, Copy, Debug)]
pub struct Namespace {
    pub first_semantic: u32,
    pub solid_resource: u64,
    pub atlas_resource: u64,
}

#[derive(Debug)]
pub enum Error {
    InvalidNamespace,
    InvalidViewport,
    StaleSnapshot,
    IdentityCollision,
    LimitExceeded,
    Text(TextError),
    Scene(tilewright_scene::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UI renderer: {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<TextError> for Error {
    fn from(value: TextError) -> Self {
        Self::Text(value)
    }
}
impl From<tilewright_scene::Error> for Error {
    fn from(value: tilewright_scene::Error) -> Self {
        Self::Scene(value)
    }
}

/// Logical geometry from the exact shared layout used to paint the glyphs.
#[derive(Clone, Debug)]
pub struct TextGeometry {
    pub widget: WidgetRef,
    pub layout: UnicodeLayout,
    pub origin: [f32; 2],
    pub clip: Rect,
    pub snapshot: Option<TextInputSnapshot>,
}
impl TextGeometry {
    pub fn hit(&self, point: [f32; 2]) -> Option<tilewright_ui::TextCursor> {
        self.clip.contains(point[0], point[1]).then(|| {
            neutral_cursor(
                self.layout
                    .hit([point[0] - self.origin[0], point[1] - self.origin[1]]),
            )
        })
    }
    /// IME candidate area in logical coordinates, clipped to the text clip.
    pub fn caret(&self) -> Option<Rect> {
        let snapshot = self.snapshot.as_ref()?;
        let caret = self
            .layout
            .caret(renderer_cursor(snapshot.display_selection.focus))?;
        intersect(translated(caret, self.origin), self.clip)
    }
    pub fn move_visual(
        &self,
        cursor: tilewright_ui::TextCursor,
        motion: VisualMotion,
    ) -> tilewright_ui::TextCursor {
        neutral_cursor(self.layout.move_visual(renderer_cursor(cursor), motion))
    }
}

/// Owns bounded shaping scratch. Frames own immutable atlas bytes and layouts.
pub struct UiRenderer {
    text: TextSystem,
    namespace: Namespace,
    limits: UiLimits,
}
impl UiRenderer {
    pub fn new(
        fonts: FontPack,
        font_limits: TextLimits,
        namespace: Namespace,
        limits: UiLimits,
    ) -> Result<Self, Error> {
        if namespace.first_semantic == 0
            || namespace.solid_resource == 0
            || namespace.atlas_resource == 0
            || namespace.solid_resource == namespace.atlas_resource
        {
            return Err(Error::InvalidNamespace);
        }
        Ok(Self {
            text: TextSystem::new(fonts, font_limits)?,
            namespace,
            limits,
        })
    }
    pub fn clear_cache(&mut self) {
        self.text.clear_cache();
    }
    /// Compile the exact layout retained by `App::prepare`. Integral logical
    /// dimensions and explicit physical dimensions apply DPI exactly once.
    #[allow(clippy::too_many_lines)]
    pub fn compile(
        &mut self,
        snapshot: &UiSnapshot,
        layout: &Layout,
        physical_size: [u32; 2],
    ) -> Result<CompiledFrame, Error> {
        if layout.ui_revision != snapshot.revision {
            return Err(Error::StaleSnapshot);
        }
        let viewport = viewport(layout, physical_size)?;
        if layout.paint.len() > self.limits.maximum_primitives.min(4096) {
            return Err(Error::LimitExceeded);
        }
        let solid = UiTexture {
            id: ResourceId(self.namespace.solid_resource),
            revision: 1,
            digest: Digest256(builtin::SOLID_SHA256),
        };
        let bytes = layout
            .paint
            .iter()
            .try_fold(0_usize, |bytes, paint| match paint {
                Paint::Text { text, .. } => bytes.checked_add(text.len()),
                _ => Some(bytes),
            })
            .ok_or(Error::LimitExceeded)?;
        if bytes > self.limits.maximum_text_bytes.min(1_048_576) {
            return Err(Error::LimitExceeded);
        }
        let mut primitives = Vec::new();
        let mut texts = Vec::new();
        let mut text_indices = Vec::new();
        for (index, paint) in layout.paint.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| Error::LimitExceeded)?;
            let id = self
                .namespace
                .first_semantic
                .checked_add(index)
                .ok_or(Error::InvalidNamespace)?;
            let order = index.checked_mul(3).ok_or(Error::LimitExceeded)?;
            match paint {
                Paint::World { bounds, clip } => {
                    validate_rect(*bounds)?;
                    validate_rect(*clip)?;
                }
                Paint::Rectangle {
                    bounds,
                    clip,
                    color,
                } => {
                    validate_rect(*bounds)?;
                    validate_rect(*clip)?;
                    primitives.push(solid_rect(*bounds, *clip, id, order, *color));
                }
                Paint::Text {
                    widget,
                    bounds,
                    clip,
                    text,
                    font,
                    color,
                    text_revision,
                } => {
                    validate_rect(*bounds)?;
                    validate_rect(*clip)?;
                    let size = match font {
                        FontRole::Body | FontRole::Button => 18.0,
                        FontRole::Composer => 20.0,
                    };
                    let shaped = self.text.layout(text, size, None)?;
                    let editable = if let Some(revision) = text_revision {
                        if *widget != snapshot.composer_widget
                            || *revision != snapshot.composer.revision
                            || *text != snapshot.composer.display_text
                        {
                            return Err(Error::StaleSnapshot);
                        }
                        Some(snapshot.composer.clone())
                    } else {
                        None
                    };
                    let mut origin = [bounds.x, bounds.y];
                    let text_clip = intersect(*bounds, *clip).unwrap_or(Rect {
                        width: 0.0,
                        height: 0.0,
                        ..*clip
                    });
                    if let Some(input) = &editable {
                        let cursor = renderer_cursor(input.display_selection.focus);
                        let caret = shaped.caret(cursor).ok_or(Error::StaleSnapshot)?;
                        origin[0] -= (caret.x + caret.width - bounds.width).max(0.0);
                        for selected in shaped.selection_rects(
                            renderer_cursor(input.display_selection.anchor),
                            cursor,
                        ) {
                            primitives.push(solid_rect(
                                translated(selected, origin),
                                text_clip,
                                id,
                                order,
                                [0.23, 0.40, 0.62, 1.0],
                            ));
                        }
                        if let Some(composition) = &input.composition {
                            for mut underline in
                                composition_rects(&shaped, composition.display_range.clone())
                            {
                                underline.y += underline.height - 2.0;
                                underline.height = 1.0;
                                primitives.push(solid_rect(
                                    translated(underline, origin),
                                    text_clip,
                                    id,
                                    order + 2,
                                    *color,
                                ));
                            }
                        }
                        if snapshot.control.focused_widget == Some(*widget) {
                            primitives.push(solid_rect(
                                translated(caret, origin),
                                text_clip,
                                id,
                                order + 2,
                                *color,
                            ));
                        }
                    }
                    texts.push(TextGeometry {
                        widget: *widget,
                        layout: shaped,
                        origin,
                        clip: text_clip,
                        snapshot: editable,
                    });
                    text_indices.push(primitives.len());
                    primitives.push(UiPrimitive::Text {
                        origin,
                        clip: rect(text_clip),
                        paint: UiPaint {
                            semantic_id: SemanticId(id),
                            painter_order: order + 1,
                            color: *color,
                        },
                        font: builtin::FONT_ID,
                        size,
                        text,
                    });
                }
            }
            if primitives.len() > self.limits.maximum_primitives.min(4096) {
                return Err(Error::LimitExceeded);
            }
        }
        let compiled = self.text.compile(
            viewport,
            UiDisplayList {
                primitives: &primitives,
            },
            solid,
            self.namespace.atlas_resource,
            self.limits,
        )?;
        for (text, index) in texts.iter_mut().zip(text_indices) {
            text.layout = compiled
                .layouts
                .iter()
                .find(|(primitive, _)| *primitive == index)
                .ok_or(Error::StaleSnapshot)?
                .1
                .clone();
        }
        Ok(CompiledFrame {
            viewport,
            quads: compiled.quads,
            text: texts,
            unsupported: compiled.unsupported.len(),
            resources: resources::Resources::new(compiled.resources, solid),
            namespace: self.namespace,
        })
    }
}

/// Can outlive the renderer and any of its later atlas revisions.
pub struct CompiledFrame {
    pub viewport: UiViewport,
    pub quads: Vec<Quad>,
    pub text: Vec<TextGeometry>,
    pub unsupported: usize,
    resources: resources::Resources,
    namespace: Namespace,
}
impl CompiledFrame {
    pub fn editable(&self) -> Option<&TextGeometry> {
        self.text.iter().find(|text| text.snapshot.is_some())
    }
    /// Checks the reserved namespace at composition, not just construction.
    pub fn compose(
        &self,
        world: &SceneBundle,
        world_resources: Arc<dyn ResourceProvider>,
    ) -> Result<Presentation, Error> {
        if [world.viewport().width, world.viewport().height] != self.viewport.physical_size {
            return Err(Error::InvalidViewport);
        }
        let ids: HashSet<_> = self.quads.iter().map(|quad| quad.semantic_id).collect();
        if world.quads().iter().any(|quad| {
            ids.contains(&quad.semantic_id)
                || quad.resource_id.0 == self.namespace.solid_resource
                || quad.resource_id.0 == self.namespace.atlas_resource
        }) {
            return Err(Error::IdentityCollision);
        }
        let scene = SceneBundle::from_quads(
            world.revision(),
            world.clock_millis(),
            world.viewport(),
            world.clear_color(),
            world
                .quads()
                .iter()
                .copied()
                .chain(self.quads.iter().copied()),
            SceneLimits::default(),
        )?;
        Ok(Presentation {
            scene,
            resources: Arc::new(self.resources.with_world(world_resources)),
        })
    }
    /// Prototype overlay. Host must qualify alpha/compositing behavior.
    pub fn standalone(&self, clear_color: [f32; 4]) -> Result<Presentation, Error> {
        let scene = SceneBundle::from_quads(
            1,
            0,
            tilewright_scene::Viewport {
                width: self.viewport.physical_size[0],
                height: self.viewport.physical_size[1],
                scale_milli: 1000,
            },
            clear_color,
            self.quads.iter().copied(),
            SceneLimits::default(),
        )?;
        Ok(Presentation {
            scene,
            resources: Arc::new(self.resources.clone()),
        })
    }
}
pub struct Presentation {
    pub scene: SceneBundle,
    pub resources: Arc<dyn ResourceProvider>,
}
impl Presentation {
    /// CPU reference output for the prototype; not production GPU performance.
    #[cfg(feature = "prototype")]
    pub fn render_rgba(
        &self,
    ) -> Result<tilewright_render_api::FrameOutput, tilewright_render_api::Error> {
        use tilewright_render_api::Renderer;
        let viewport = self.scene.viewport();
        tilewright_render_testkit::ReferenceRenderer::new(
            tilewright_render_testkit::offscreen_target(viewport.width, viewport.height),
        )?
        .render(tilewright_render_api::RenderRequest {
            scene: &self.scene,
            resources: self.resources.clone(),
        })
    }
}

fn viewport(layout: &Layout, physical_size: [u32; 2]) -> Result<UiViewport, Error> {
    let sizes = [layout.viewport.width, layout.viewport.height];
    if sizes
        .iter()
        .any(|size| !size.is_finite() || *size < 1.0 || *size > 8192.0 || size.fract() != 0.0)
        || physical_size.contains(&0)
    {
        return Err(Error::InvalidViewport);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(UiViewport {
        logical_size: sizes.map(|size| size as u32),
        physical_size,
    })
}
fn rect(value: Rect) -> UiRect {
    UiRect {
        x: value.x,
        y: value.y,
        width: value.width,
        height: value.height,
    }
}
fn validate_rect(value: Rect) -> Result<(), Error> {
    if [
        value.x,
        value.y,
        value.width,
        value.height,
        value.x + value.width,
        value.y + value.height,
    ]
    .iter()
    .any(|v| !v.is_finite() || v.abs() > 16_777_216.0)
        || value.width < 0.0
        || value.height < 0.0
    {
        Err(Error::InvalidViewport)
    } else {
        Ok(())
    }
}
fn translated(value: UiRect, origin: [f32; 2]) -> Rect {
    Rect {
        x: value.x + origin[0],
        y: value.y + origin[1],
        width: value.width,
        height: value.height,
    }
}
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (right > x && bottom > y).then_some(Rect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}
fn solid_rect<'a>(
    bounds: Rect,
    clip: Rect,
    id: u32,
    order: u32,
    color: [f32; 4],
) -> UiPrimitive<'a> {
    UiPrimitive::Solid {
        bounds: rect(bounds),
        clip: rect(clip),
        paint: UiPaint {
            semantic_id: SemanticId(id),
            painter_order: order,
            color,
        },
        pickable: false,
    }
}
fn renderer_cursor(cursor: tilewright_ui::TextCursor) -> TextCursor {
    TextCursor {
        byte: cursor.byte,
        affinity: match cursor.affinity {
            tilewright_ui::TextAffinity::Before => TextAffinity::Before,
            tilewright_ui::TextAffinity::After => TextAffinity::After,
        },
    }
}
fn neutral_cursor(cursor: TextCursor) -> tilewright_ui::TextCursor {
    tilewright_ui::TextCursor {
        byte: cursor.byte,
        affinity: match cursor.affinity {
            TextAffinity::Before => tilewright_ui::TextAffinity::Before,
            TextAffinity::After => tilewright_ui::TextAffinity::After,
        },
    }
}
// Adapted from MIT editor workspace_render.rs; see PROVENANCE.md.
fn composition_rects(layout: &UnicodeLayout, range: std::ops::Range<usize>) -> Vec<UiRect> {
    let boundaries = layout.grapheme_boundaries();
    let start = boundaries
        .iter()
        .copied()
        .take_while(|byte| *byte <= range.start)
        .last()
        .unwrap_or(0);
    let end = boundaries
        .iter()
        .copied()
        .find(|byte| *byte >= range.end)
        .unwrap_or(layout.text().len());
    layout.selection_rects(
        TextCursor {
            byte: start,
            affinity: TextAffinity::After,
        },
        TextCursor {
            byte: end,
            affinity: TextAffinity::Before,
        },
    )
}

#[cfg(test)]
mod tests;
