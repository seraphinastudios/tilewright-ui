// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! The single field's style in the compiled frame, and the frame's raster
//! sample count (both since 0.1.2).
#![allow(clippy::float_cmp)] // Identical engines must give bit-identical geometry.

use super::tests::fonts;
use super::*;
use tilewright_ui::{
    COMPOSER, Composition, FieldStyle, Input, LayoutOptions, Limits, TextCursor, TextSelection, Ui,
    Viewport,
};

const NAMESPACE: Namespace = Namespace {
    first_semantic: 100,
    solid_resource: 100,
    atlas_resource: 101,
};
/// A row as a host's own 30 px field row draws it: the whole row filled,
/// 18 px text 4 px from the left and 1 px from the top, a scrolled caret's
/// left edge kept 8 px from the right edge, and one accent for selection,
/// caret and underline.
const ROW: Rect = Rect {
    x: 470.0,
    y: 96.0,
    width: 300.0,
    height: 30.0,
};
const ACCENT: [f32; 4] = [0.28, 0.48, 0.66, 1.0];
const TEXT: [f32; 4] = [0.91, 0.94, 0.98, 1.0];
const ROW_FILL: [f32; 4] = [0.12, 0.32, 0.47, 1.0];

fn host_row_style() -> LayoutOptions {
    let mut style = FieldStyle::default();
    style.text_size = 18.0;
    style.inset = 0.0;
    style.height = 30.0;
    style.padding_left = 4.0;
    style.padding_top = 1.0;
    // The caret is 1 px wide, so its right edge then stays 7 px from the
    // row's right edge.
    style.padding_right = 7.0;
    style.text_color = TEXT;
    style.box_color = Some(ROW_FILL);
    style.selection_color = ACCENT;
    style.caret_color = Some(ACCENT);
    style.preedit_color = Some(ACCENT);
    let mut options = LayoutOptions::default();
    options.submit_control = false;
    options.field_style = style;
    options
}

fn field(text: &str) -> Ui {
    let mut ui = Ui::with_composition(Limits::default(), Composition::SingleField).unwrap();
    ui.bind_field(text).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui
}

fn compile(
    system: &mut TextSystem,
    ui: &Ui,
    bounds: Rect,
    options: &LayoutOptions,
) -> (Layout, CompiledFrame) {
    let snapshot = ui.snapshot();
    let viewport = Viewport::new(800.0, 600.0, 1.0).unwrap();
    let layout =
        tilewright_ui::layout_in_with(&snapshot, viewport, bounds, bounds, options).unwrap();
    let frame = UiCompiler::new(NAMESPACE, UiLimits::default())
        .unwrap()
        .compile(system, &snapshot, &layout, [800, 600])
        .unwrap();
    (layout, frame)
}

fn solid_colours(frame: &CompiledFrame) -> Vec<[f32; 4]> {
    frame
        .quads
        .iter()
        .filter(|quad| quad.resource_id.0 == NAMESPACE.solid_resource)
        .map(|quad| quad.color)
        .collect()
}

/// The style's text size, padding and caret margin reach shaping, the text
/// origin, the scroll, the caret, hit-testing and caret steps alike; the
/// field then lands where a host's own 30 px row draws its text.
#[test]
fn a_host_row_style_places_text_and_caret_as_the_host_row_does() {
    let mut system = TextSystem::new(fonts(), TextLimits::default()).unwrap();
    for text in [
        "label",
        "",
        "A\u{301} אבג مرحبا",
        "a long value that scrolls well past the end of a three hundred pixel row",
    ] {
        let ui = field(text);
        let (layout, frame) = compile(&mut system, &ui, ROW, &host_row_style());
        assert_eq!(layout.composer_bounds, Some(ROW));
        let geometry = frame.editable().unwrap();
        // Shaped at 18 px by the lent system, as the host shapes its row.
        let own = system
            .layout(&ui.snapshot().composer.display_text, 18.0, None)
            .unwrap();
        assert_eq!(geometry.layout.size(), own.size());
        // The host row's origin and scroll rule.
        let focus = renderer_cursor(ui.snapshot().composer.display_selection.focus);
        let caret = own.caret(focus).unwrap();
        let mut origin = [ROW.x + 4.0, ROW.y + 1.0];
        origin[0] -= (caret.x - (ROW.width - 12.0)).max(0.0);
        assert!((geometry.origin[0] - origin[0]).abs() < 1e-3, "{text}");
        assert_eq!(geometry.origin[1], origin[1]);
        assert_eq!(geometry.clip, ROW);
        let drawn = translated(caret, geometry.origin);
        assert_eq!(geometry.caret(), intersect(drawn, ROW));
        assert!(drawn.x <= ROW.x + ROW.width - 8.0 + 1e-3);
        // A click on the caret's place hits the caret's position.
        let point = [drawn.x + 0.25, drawn.y + drawn.height / 2.0];
        assert_eq!(
            geometry.hit(point).map(|cursor| cursor.byte),
            Some(
                own.hit([point[0] - geometry.origin[0], point[1] - geometry.origin[1]])
                    .byte
            )
        );
        if !text.is_empty() {
            assert_ne!(
                geometry.step_caret(VisualMotion::Home).unwrap().byte,
                text.len()
            );
        }
        // Row fill, then the caret in the accent; no 0.1.1 colours.
        let colours = solid_colours(&frame);
        assert_eq!(colours.first(), Some(&ROW_FILL));
        assert_eq!(colours.last(), Some(&ACCENT));
        assert!(!colours.contains(&[0.18, 0.2, 0.25, 1.0]));
        assert!(!colours.contains(&[1.0; 4]));
    }
}

#[test]
fn selection_caret_and_underline_use_the_style_colours() {
    let mut system = TextSystem::new(fonts(), TextLimits::default()).unwrap();
    let mut options = host_row_style();
    options.field_style.box_color = None;
    options.field_style.selection_color = [0.1, 0.2, 0.3, 1.0];
    options.field_style.caret_color = Some([0.4, 0.5, 0.6, 1.0]);
    options.field_style.preedit_color = Some([0.7, 0.8, 0.9, 1.0]);
    let mut ui = field("select me");
    let (_, frame) = compile(&mut system, &ui, ROW, &options);
    // No box: the caret is the only solid quad.
    assert_eq!(solid_colours(&frame), vec![[0.4, 0.5, 0.6, 1.0]]);

    let target = ui.text_target().unwrap();
    ui.apply(Input::Selection {
        target,
        selection: TextSelection {
            anchor: TextCursor::new(0),
            focus: TextCursor::new(6),
        },
    })
    .unwrap();
    let (_, frame) = compile(&mut system, &ui, ROW, &options);
    let colours = solid_colours(&frame);
    assert!(colours.contains(&[0.1, 0.2, 0.3, 1.0]));
    assert!(!colours.contains(&[0.23, 0.40, 0.62, 1.0]));

    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "ab".into(),
        start: 2,
        length: 0,
    })
    .unwrap();
    let (_, frame) = compile(&mut system, &ui, ROW, &options);
    let colours = solid_colours(&frame);
    assert!(colours.contains(&[0.7, 0.8, 0.9, 1.0]));
    assert!(colours.contains(&[0.4, 0.5, 0.6, 1.0]));
    assert!(!colours.contains(&TEXT));

    // A caret colour alone leaves the underline in the text's colour.
    options.field_style.preedit_color = None;
    let (_, frame) = compile(&mut system, &ui, ROW, &options);
    let colours = solid_colours(&frame);
    assert_eq!(colours.iter().filter(|colour| **colour == TEXT).count(), 1);
    assert_eq!(
        colours
            .iter()
            .filter(|colour| **colour == [0.4, 0.5, 0.6, 1.0])
            .count(),
        1
    );

    // Without caret and underline colours, both take the text's, as in 0.1.1.
    options.field_style.caret_color = None;
    options.field_style.preedit_color = None;
    let (_, frame) = compile(&mut system, &ui, ROW, &options);
    let colours = solid_colours(&frame);
    assert!(colours.iter().filter(|colour| **colour == TEXT).count() >= 2);
}

/// With the default style the composer is drawn at 20 px from the box's
/// corner, and the caret scrolls to the box's very edge, as in 0.1.1.
#[test]
fn the_default_style_compiles_as_0_1_1() {
    let mut system = TextSystem::new(fonts(), TextLimits::default()).unwrap();
    let text = "a long value that scrolls well past the end of a three hundred pixel row";
    let ui = field(text);
    let bounds = Rect {
        height: 52.0,
        ..ROW
    };
    let (layout, frame) = compile(&mut system, &ui, bounds, &LayoutOptions::default());
    let composer = layout.composer_bounds.unwrap();
    let geometry = frame.editable().unwrap();
    let own = system.layout(text, 20.0, None).unwrap();
    assert_eq!(geometry.layout.size(), own.size());
    let caret = own
        .caret(renderer_cursor(TextCursor::new(text.len())))
        .unwrap();
    assert_eq!(
        geometry.origin,
        [
            composer.x - (caret.x + caret.width - composer.width).max(0.0),
            composer.y
        ]
    );
    let colours = solid_colours(&frame);
    // Box, caret, Apply control.
    assert_eq!(
        colours,
        vec![[0.18, 0.2, 0.25, 1.0], [1.0; 4], [0.2, 0.35, 0.6, 1.0]]
    );
}

fn sprite_quad(positions: [[f32; 2]; 4], glow: Option<tilewright_scene::Glow>) -> Quad {
    Quad {
        semantic_id: SemanticId(1),
        part: 0,
        resource_id: ResourceId(7),
        resource_revision: 1,
        resource_digest: Digest256([7; 32]),
        frame: None,
        positions,
        uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        depth: 0,
        painter_order: 0,
        color: [1.0; 4],
        grayscale: 0.0,
        glow,
        pickable: glow.is_none(),
        sampling: tilewright_scene::Sampling::Nearest,
        layer: tilewright_scene::Layer::World,
    }
}

fn world(quads: Vec<Quad>, viewport: [u32; 2]) -> SceneBundle {
    SceneBundle::from_quads(
        1,
        0,
        tilewright_scene::Viewport {
            width: viewport[0],
            height: viewport[1],
            scale_milli: 1000,
        },
        [0.0; 4],
        quads,
        SceneLimits {
            maximum_raster_samples: u64::MAX,
            ..SceneLimits::default()
        },
    )
    .unwrap()
}

/// The count of one quad is the one `SceneBundle::from_quads` charges: a
/// scene is accepted at exactly that budget and refused one sample below.
#[test]
fn raster_samples_match_the_scene_limit_for_every_quad_shape() {
    let glow = tilewright_scene::Glow {
        color: [1.0; 4],
        radius_uv: [0.25, 0.0],
    };
    let flat = tilewright_scene::Glow {
        radius_uv: [0.0; 2],
        ..glow
    };
    let cases = [
        (
            sprite_quad([[0.5, 0.5], [9.2, 0.5], [9.2, 4.0], [0.5, 4.0]], None),
            40,
        ),
        // Clipped to the 64 x 48 viewport.
        (
            sprite_quad([[60.0, -5.0], [70.0, -5.0], [70.0, 3.0], [60.0, 3.0]], None),
            12,
        ),
        (
            sprite_quad([[80.0, 0.0], [90.0, 0.0], [90.0, 9.0], [80.0, 9.0]], None),
            0,
        ),
        // Not axis aligned: both triangles' boxes.
        (
            sprite_quad([[0.0, 0.0], [8.0, 2.0], [10.0, 10.0], [2.0, 8.0]], None),
            200,
        ),
        (
            sprite_quad([[0.0, 0.0], [4.0, 0.0], [4.0, 2.0], [0.0, 2.0]], Some(glow)),
            8 * 33,
        ),
        (
            sprite_quad([[0.0, 0.0], [4.0, 0.0], [4.0, 2.0], [0.0, 2.0]], Some(flat)),
            8,
        ),
    ];
    for (quad, samples) in cases {
        assert_eq!(raster_samples(&quad, 64, 48), samples, "{quad:?}");
        let scene = |budget| {
            SceneBundle::from_quads(
                1,
                0,
                tilewright_scene::Viewport {
                    width: 64,
                    height: 48,
                    scale_milli: 1000,
                },
                [0.0; 4],
                [quad],
                SceneLimits {
                    maximum_raster_samples: budget,
                    ..SceneLimits::default()
                },
            )
        };
        assert!(scene(samples).is_ok());
        if samples > 0 {
            assert!(scene(samples - 1).is_err());
        }
    }
}

/// `compose_with_limits` accepts a frame over a world exactly when the
/// world's samples plus `raster_sample_count` are within the budget.
#[test]
fn compose_with_limits_charges_exactly_the_raster_sample_count() {
    let mut system = TextSystem::new(fonts(), TextLimits::default()).unwrap();
    let mut ui = field("A\u{301} אבג مرحبا select");
    let target = ui.text_target().unwrap();
    ui.apply(Input::Selection {
        target,
        selection: TextSelection {
            anchor: TextCursor::new(0),
            focus: TextCursor::new(4),
        },
    })
    .unwrap();
    let demo = Ui::new(Limits::default()).unwrap();
    let frames = [
        compile(&mut system, &ui, ROW, &host_row_style()).1,
        compile(&mut system, &ui, ROW, &LayoutOptions::default()).1,
        UiCompiler::new(NAMESPACE, UiLimits::default())
            .unwrap()
            .compile(
                &mut system,
                &demo.snapshot(),
                &tilewright_ui::layout(&demo.snapshot(), Viewport::new(320.0, 200.0, 1.5).unwrap())
                    .unwrap(),
                [480, 300],
            )
            .unwrap(),
    ];
    for frame in frames {
        let size = frame.viewport.physical_size;
        let count = frame.raster_sample_count();
        assert!(count > 0);
        let mut quads = vec![
            sprite_quad([[1.0, 1.0], [41.5, 1.0], [41.5, 30.0], [1.0, 30.0]], None),
            sprite_quad([[2.0, 0.0], [9.0, 3.0], [12.0, 12.0], [0.0, 9.0]], None),
        ];
        quads[1].semantic_id = SemanticId(2);
        let world_samples: u64 = quads
            .iter()
            .map(|quad| raster_samples(quad, size[0], size[1]))
            .sum();
        for (world, before) in [
            (world(Vec::new(), size), 0),
            (world(quads, size), world_samples),
        ] {
            let resources = || frame.standalone([0.0; 4]).unwrap().resources;
            let budget = |samples| SceneLimits {
                maximum_raster_samples: samples,
                ..SceneLimits::default()
            };
            assert!(
                frame
                    .compose_with_limits(&world, resources(), budget(before + count))
                    .is_ok()
            );
            assert!(matches!(
                frame.compose_with_limits(&world, resources(), budget(before + count - 1)),
                Err(Error::Scene(tilewright_scene::Error::LimitExceeded))
            ));
        }
    }
}

/// The padding moves only the composer's text: the Apply label stays at its
/// control's corner, 18 px, as in 0.1.1.
#[test]
fn the_padding_leaves_the_submit_label_where_it_was() {
    let mut system = TextSystem::new(fonts(), TextLimits::default()).unwrap();
    let mut options = host_row_style();
    options.submit_control = true;
    let ui = field("label");
    let (layout, frame) = compile(&mut system, &ui, ROW, &options);
    let submit = layout
        .hits
        .iter()
        .find(|hit| hit.widget.id == tilewright_ui::SUBMIT)
        .unwrap()
        .bounds;
    let label = frame
        .text
        .iter()
        .find(|text| text.widget.id == tilewright_ui::SUBMIT)
        .unwrap();
    assert_eq!(label.origin, [submit.x, submit.y]);
    assert_eq!(
        label.layout.size(),
        system.layout("Apply", 18.0, None).unwrap().size()
    );
    let composer = layout.composer_bounds.unwrap();
    assert_eq!(
        frame.editable().unwrap().origin,
        [composer.x + 4.0, composer.y + 1.0]
    );
}
