// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! Exact neutral paint translated through the existing Unicode renderer.
//! Retain frames with the application's presentation stamp; never use prepared
//! text geometry before its presentation has been acknowledged.
#![forbid(unsafe_code)]

mod resources;
use std::{collections::HashSet, fmt, sync::Arc};
use tilewright_render_resources::ResourceProvider;
// The root re-exports the renderer types needed to construct and drive
// `UiRenderer`/`UiCompiler` and to read a layout. A host that composes its own
// scene or reads frame internals (`SceneBundle`, `ResourceProvider`, `Quad`,
// `UiViewport`, `TextError`, `tilewright_scene::Error`) uses the renderer
// crates directly, at the same exact version. The renderer's own `TextCursor`
// and `TextAffinity` are not re-exported: this crate's API uses
// `tilewright_ui`'s types of those names, and `UnicodeLayout`'s methods take
// the renderer's, from `tilewright-render-ui` directly.
pub use tilewright_render_ui::TextSystem;
pub use tilewright_render_ui::UiLimits;
/// Verified font-pack metadata for explicit host loading; contains no font bytes.
pub use tilewright_render_ui::unicode_fonts;
pub use tilewright_render_ui::{FontBlob, FontPack, TextLimits, UnicodeLayout, VisualMotion};
use tilewright_render_ui::{TextAffinity, TextCursor, TextError};
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
#[non_exhaustive]
pub enum Error {
    InvalidNamespace,
    InvalidViewport,
    StaleSnapshot,
    IdentityCollision,
    LimitExceeded,
    /// The lent text system issued this compiler's atlas ID a revision lower
    /// than one this compiler was issued before, or the same revision with
    /// different bytes: a different text system was lent. Nothing changed.
    TextSystemChanged,
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
#[non_exhaustive]
pub struct TextGeometry {
    pub widget: WidgetRef,
    pub layout: UnicodeLayout,
    pub origin: [f32; 2],
    pub clip: Rect,
    pub snapshot: Option<TextInputSnapshot>,
}
impl TextGeometry {
    /// Returns a displayed UTF-8 cursor. For committed selection, map it with
    /// `snapshot.committed_cursor`, which rejects an interior preedit position.
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
    /// One raw layout step in display coordinates. It does not skip a
    /// marker's guard; hosts use [`TextGeometry::step_caret`].
    fn move_visual(
        &self,
        cursor: tilewright_ui::TextCursor,
        motion: VisualMotion,
    ) -> tilewright_ui::TextCursor {
        neutral_cursor(self.layout.move_visual(renderer_cursor(cursor), motion))
    }
    /// Moves the caret one visual step from `snapshot.display_selection.focus`
    /// and returns the new focus in committed text, ready for
    /// `Input::Selection` (keep `snapshot.selection.anchor` to extend).
    ///
    /// The layout holds `display_text`, where a stored line break or control
    /// is one marker glyph. A marker with a zero-width guard has two display
    /// positions for one committed position, so a step that moves the display
    /// byte but not the committed byte only crossed the guard: it is repeated,
    /// so the caret does not stall there. A step that moves the affinity alone
    /// and leaves the caret where it was drawn (the two caret rectangles' x and
    /// y each within 1/64 logical pixel) is not a stop either and
    /// is repeated: the layout can give one position two caret stops that
    /// differ only by floating-point rounding. A step that moves the affinity
    /// alone to a visibly different place is a real visual stop at a direction
    /// boundary and is returned at once, as is a step that does not move. One
    /// call takes at most four layout steps. In some mixed-direction text a
    /// guard can still make the caret alternate between two positions.
    ///
    /// Fails with `UiError::InvalidInput`, without stepping, while a
    /// composition is active or when this text is not the editable one
    /// (`snapshot` is `None`).
    pub fn step_caret(
        &self,
        motion: VisualMotion,
    ) -> Result<tilewright_ui::TextCursor, tilewright_ui::UiError> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or(tilewright_ui::UiError::InvalidInput)?;
        step_visual(
            snapshot,
            |cursor| self.move_visual(cursor, motion),
            |cursor| {
                self.layout
                    .caret(renderer_cursor(cursor))
                    .map(|rect| [rect.x, rect.y])
            },
        )
    }
}

/// How far apart, in logical scene pixels on each axis, two caret
/// rectangles may be and still count as the same drawn position when
/// [`TextGeometry::step_caret`] decides whether a change of affinity alone
/// moved the caret: 1/64 px.
///
/// The rounding it absorbs is a few units in the last place (ULPs) of an
/// `f32` coordinate: 97.700005 against 97.7 at 20 px is 8e-6 px. One ULP stays
/// below 1/64 px for any coordinate under 131072 px; three ULPs reach it near
/// 65536 px. On text that long the rule can miss the rounding and falls back
/// to the 0.1.0 behaviour, one press that does nothing; it never swallows a
/// real stop. A genuine direction boundary puts its two stops at least one
/// glyph advance apart, whole pixels, and 1/64 logical px is under 1/16 of a
/// physical pixel at any scale up to 4.
const SAME_CARET_POSITION: f32 = 1.0 / 64.0;

/// Layout steps one [`TextGeometry::step_caret`] call may take. One
/// committed position has at most two display positions (beside a marker's
/// guard), each with at most two affinities, so three steps can stay on it
/// and the fourth leaves it.
const MAXIMUM_STEPS: usize = 4;

/// The stepping loop behind [`TextGeometry::step_caret`]. `step` moves a
/// display cursor one position and `caret` gives a display cursor's caret
/// position; tests drive both with synthetic layouts.
fn step_visual(
    snapshot: &TextInputSnapshot,
    mut step: impl FnMut(tilewright_ui::TextCursor) -> tilewright_ui::TextCursor,
    caret: impl Fn(tilewright_ui::TextCursor) -> Option<[f32; 2]>,
) -> Result<tilewright_ui::TextCursor, tilewright_ui::UiError> {
    if snapshot.composition.is_some() {
        return Err(tilewright_ui::UiError::InvalidInput);
    }
    let start = snapshot.selection.focus;
    let mut display = snapshot.display_selection.focus;
    let mut result = start;
    for _ in 0..MAXIMUM_STEPS {
        let next = step(display);
        if next == display {
            break;
        }
        let moved = next.byte != display.byte;
        let unseen = !moved && same_position(caret(display), caret(next));
        display = next;
        result = snapshot.committed_cursor(display)?;
        if unseen {
            continue;
        }
        if result.byte != start.byte || !moved {
            break;
        }
    }
    Ok(result)
}

/// Whether two caret positions are the same drawn position; a cursor with
/// no caret is never the same as anything.
fn same_position(a: Option<[f32; 2]>, b: Option<[f32; 2]>) -> bool {
    a.zip(b).is_some_and(|(a, b)| {
        (a[0] - b[0]).abs() <= SAME_CARET_POSITION && (a[1] - b[1]).abs() <= SAME_CARET_POSITION
    })
}

/// Compiles frames with a text system the host lends for each call.
///
/// It holds no fonts, shaping caches or glyph rasters, so constructing it reads
/// and parses no font. A host that already owns a [`TextSystem`] passes it to
/// [`UiCompiler::compile`]; the shared widgets are then shaped, measured and
/// rasterized by exactly the engine, fonts and caches the host's own text uses.
///
/// The host's own text and this compiler may alternate on one system: an
/// unchanged atlas keeps its exact identity per atlas ID, whatever other IDs
/// were compiled in between, so nothing is re-uploaded. The text system
/// remembers only a small fixed number of recently used atlas IDs. An ID
/// compiled again after more other IDs than that, or any ID after
/// `TextSystem::clear_cache`, gets a new revision for unchanged bytes: it
/// loses reuse, never correctness.
#[derive(Debug)]
pub struct UiCompiler {
    namespace: Namespace,
    limits: UiLimits,
    /// Revision and digest last issued for `namespace.atlas_resource`.
    last_atlas: Option<(u64, Digest256)>,
}

/// Owns its own text system; for standalone hosts with no text of their own.
/// Frames own immutable atlas bytes and layouts.
pub struct UiRenderer {
    text: TextSystem,
    compiler: UiCompiler,
}
impl UiRenderer {
    pub fn new(
        fonts: FontPack,
        font_limits: TextLimits,
        namespace: Namespace,
        limits: UiLimits,
    ) -> Result<Self, Error> {
        let compiler = UiCompiler::new(namespace, limits)?;
        Ok(Self {
            text: TextSystem::new(fonts, font_limits)?,
            compiler,
        })
    }
    pub fn clear_cache(&mut self) {
        self.text.clear_cache();
    }
    /// Compile the exact layout retained by `App::prepare`. Integral logical
    /// dimensions and explicit physical dimensions apply DPI exactly once.
    pub fn compile(
        &mut self,
        snapshot: &UiSnapshot,
        layout: &Layout,
        physical_size: [u32; 2],
    ) -> Result<CompiledFrame, Error> {
        self.compiler
            .compile(&mut self.text, snapshot, layout, physical_size)
    }
}

impl UiCompiler {
    pub fn new(namespace: Namespace, limits: UiLimits) -> Result<Self, Error> {
        if namespace.first_semantic == 0
            || namespace.solid_resource == 0
            || namespace.atlas_resource == 0
            || namespace.solid_resource == namespace.atlas_resource
        {
            return Err(Error::InvalidNamespace);
        }
        Ok(Self {
            namespace,
            limits,
            last_atlas: None,
        })
    }
    /// Compile the exact layout retained by `App::prepare`. Integral logical
    /// dimensions and explicit physical dimensions apply DPI exactly once.
    ///
    /// `system` may be one the host also uses for its own text; this call only
    /// borrows it, and its fonts and `TextLimits` apply. Lend the same system
    /// on every call: atlas revisions rise per system, so a second system could
    /// reissue an old revision of this namespace's atlas ID. A revision lower
    /// than one this compiler was issued before, or the same revision with
    /// different bytes, is refused with [`Error::TextSystemChanged`]; a refused
    /// compile changes nothing. So, given a reserved atlas ID, a compiler never
    /// returns two different atlases under one (ID, revision). To switch text
    /// systems on purpose, retire every frame carrying this atlas ID and create
    /// a new `UiCompiler`.
    ///
    /// The namespace's atlas ID must be one the host can never compute for its
    /// own atlases. [`CompiledFrame::compose`] rejects a host scene that uses
    /// it with [`Error::IdentityCollision`].
    #[allow(clippy::too_many_lines)]
    pub fn compile(
        &mut self,
        system: &mut TextSystem,
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
                    // The composer's text follows the layout's field style
                    // (20 px and no padding by default, as in 0.1.1).
                    let style = layout.field_style();
                    let (size, padding) = match font {
                        FontRole::Body | FontRole::Button => (18.0, [0.0; 3]),
                        FontRole::Composer => (
                            style.text_size,
                            [style.padding_left, style.padding_top, style.padding_right],
                        ),
                    };
                    let shaped = system.layout(text, size, None)?;
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
                    let mut origin = [padded(bounds.x, padding[0]), padded(bounds.y, padding[1])];
                    let target = Rect {
                        x: 0.0,
                        y: 0.0,
                        width: layout.viewport.width,
                        height: layout.viewport.height,
                    };
                    let text_clip = intersect(*bounds, *clip)
                        .and_then(|clip| intersect(clip, target))
                        .unwrap_or(Rect {
                            width: 0.0,
                            height: 0.0,
                            ..*clip
                        });
                    if let Some(input) = &editable {
                        let cursor = renderer_cursor(input.display_selection.focus);
                        let caret = shaped.caret(cursor).ok_or(Error::StaleSnapshot)?;
                        shaped
                            .caret(renderer_cursor(input.display_selection.anchor))
                            .ok_or(Error::StaleSnapshot)?;
                        let available = bounds.width - padding[0] - padding[2];
                        origin[0] -= (caret.x + caret.width - available).max(0.0);
                        for selected in shaped.selection_rects(
                            renderer_cursor(input.display_selection.anchor),
                            cursor,
                        ) {
                            primitives.push(solid_rect(
                                translated(selected, origin),
                                text_clip,
                                id,
                                order,
                                style.selection_color,
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
                                    style.preedit_color.unwrap_or(*color),
                                ));
                            }
                        }
                        if snapshot.control.focused_widget == Some(*widget) {
                            primitives.push(solid_rect(
                                translated(caret, origin),
                                text_clip,
                                id,
                                order + 2,
                                style.caret_color.unwrap_or(*color),
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
        let compiled = system.compile(
            viewport,
            UiDisplayList {
                primitives: &primitives,
            },
            solid,
            self.namespace.atlas_resource,
            self.limits,
        )?;
        if let Some(atlas) = compiled
            .resources
            .iter()
            .find(|texture| texture.identity.id.0 == self.namespace.atlas_resource)
        {
            let issued = (atlas.identity.revision, atlas.identity.digest);
            if self.last_atlas.is_some_and(|(revision, digest)| {
                issued.0 < revision || (issued.0 == revision && issued.1 != digest)
            }) {
                return Err(Error::TextSystemChanged);
            }
            self.last_atlas = Some(issued);
        }
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
    viewport: UiViewport,
    quads: Vec<Quad>,
    pub text: Vec<TextGeometry>,
    // Unsupported renderer primitives; only the prototype tests read it.
    #[cfg_attr(not(all(test, feature = "prototype")), allow(dead_code))]
    unsupported: usize,
    resources: resources::Resources,
    namespace: Namespace,
}
impl CompiledFrame {
    pub fn editable(&self) -> Option<&TextGeometry> {
        self.text.iter().find(|text| text.snapshot.is_some())
    }
    /// The number of quads this frame adds to a composed scene. A host that
    /// composes under its own scene limits budgets this many quads on top of
    /// its world's (see [`CompiledFrame::compose_with_limits`]).
    pub fn quad_count(&self) -> usize {
        self.quads.len()
    }
    /// The raster samples this frame's quads add to a composed scene, as
    /// `SceneLimits::maximum_raster_samples` counts them: for each quad, the
    /// pixels of its bounding box (or, for a quad that is not an axis-aligned
    /// rectangle, of each of its two triangles' boxes) clipped to the frame's
    /// physical viewport in whole pixels, times
    /// `tilewright_scene::GLOW_SAMPLES_PER_PIXEL` for a glow with a nonzero
    /// radius, summed with saturation. A host composing under its own limits
    /// budgets this many samples on top of its world's, as it budgets
    /// [`CompiledFrame::quad_count`] quads: [`CompiledFrame::compose_with_limits`]
    /// accepts the frame over a world exactly when the two together are
    /// within `maximum_raster_samples` (and the other limits hold).
    pub fn raster_sample_count(&self) -> u64 {
        let [width, height] = self.viewport.physical_size;
        self.quads.iter().fold(0_u64, |total, quad| {
            total.saturating_add(raster_samples(quad, width, height))
        })
    }
    /// Checks the reserved namespace at composition, not just construction.
    /// The composed scene is checked against the default scene limits; it is
    /// [`CompiledFrame::compose_with_limits`] with `SceneLimits::default()`.
    pub fn compose(
        &self,
        world: &SceneBundle,
        world_resources: Arc<dyn ResourceProvider>,
    ) -> Result<Presentation, Error> {
        self.compose_with_limits(world, world_resources, SceneLimits::default())
    }
    /// [`CompiledFrame::compose`] under the host's scene limits: the composed
    /// scene, the world's quads followed by this frame's
    /// [`CompiledFrame::quad_count`] quads, is checked against `limits`
    /// (quad count, raster samples, viewport) instead of the defaults. A scene
    /// over them fails with `Error::Scene`; nothing changes.
    pub fn compose_with_limits(
        &self,
        world: &SceneBundle,
        world_resources: Arc<dyn ResourceProvider>,
        limits: SceneLimits,
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
            limits,
        )?;
        Ok(Presentation {
            scene,
            resources: Arc::new(self.resources.with_world(world_resources)),
        })
    }
    /// Prototype overlay. A transparent clear produces premultiplied RGB from
    /// the reference renderer; the host must use matching alpha composition.
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
/// One quad's raster samples in a `width` by `height` scene, counted as
/// `tilewright-scene` 0.7.2 counts them against
/// `SceneLimits::maximum_raster_samples` (its documented rule: an aligned
/// quad's box, or both triangles' boxes, clipped in whole pixels, times the
/// glow factor). `tests.rs` checks it against `compose_with_limits`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn raster_samples(quad: &Quad, width: u32, height: u32) -> u64 {
    let span = |low: f32, high: f32, size: u32| {
        let low = f64::from(low).floor().max(0.0);
        let high = f64::from(high).ceil().min(f64::from(size));
        if high > low { (high - low) as u64 } else { 0 }
    };
    let pixels = |points: &[[f32; 2]]| {
        let (mut left, mut right, mut top, mut bottom) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for point in points {
            left = left.min(point[0]);
            right = right.max(point[0]);
            top = top.min(point[1]);
            bottom = bottom.max(point[1]);
        }
        span(left, right, width).saturating_mul(span(top, bottom, height))
    };
    let [first, second, third, fourth] = quad.positions;
    let covered = if quad.aligned_rect().is_some() {
        pixels(&quad.positions)
    } else {
        pixels(&[first, second, third]).saturating_add(pixels(&[first, third, fourth]))
    };
    let glowing = quad
        .glow
        .is_some_and(|glow| glow.radius_uv.iter().any(|radius| *radius > 0.0));
    covered.saturating_mul(if glowing {
        tilewright_scene::GLOW_SAMPLES_PER_PIXEL
    } else {
        1
    })
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
/// `value` moved by `padding`; a zero padding leaves its bits as they are.
fn padded(value: f32, padding: f32) -> f32 {
    if padding == 0.0 {
        value
    } else {
        value + padding
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
mod step_tests;
#[cfg(test)]
mod tests;
