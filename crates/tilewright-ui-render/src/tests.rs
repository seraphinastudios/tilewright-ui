// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

use super::*;
use tilewright_render_resources::{Error as ResourceError, ResourceRequest};
#[cfg(feature = "prototype")]
use tilewright_ui::Input;
use tilewright_ui::{Limits, Ui, Viewport};

fn snapshot() -> UiSnapshot {
    Ui::new(Limits::default()).unwrap().snapshot()
}

#[test]
fn invalid_viewports_and_nonfinite_rectangles_fail() {
    let mut layout =
        tilewright_ui::layout(&snapshot(), Viewport::new(320.0, 200.0, 1.0).unwrap()).unwrap();
    assert!(viewport(&layout, [640, 400]).is_ok());
    layout.viewport.width = 320.5;
    assert!(matches!(
        viewport(&layout, [640, 400]),
        Err(Error::InvalidViewport)
    ));
    assert!(
        validate_rect(Rect {
            x: f32::NAN,
            y: 0.0,
            width: 3.0,
            height: 3.0
        })
        .is_err()
    );
    assert!(
        validate_rect(Rect {
            x: 0.0,
            y: 0.0,
            width: -1.0,
            height: 3.0
        })
        .is_err()
    );
}

#[test]
fn resource_requests_require_exact_identity_and_byte_budget() {
    let solid = UiTexture {
        id: ResourceId(10),
        revision: 1,
        digest: Digest256(builtin::SOLID_SHA256),
    };
    let resources = resources::Resources::new(Vec::new(), solid);
    let request = ResourceRequest {
        id: solid.id,
        revision: 1,
        digest: solid.digest,
        maximum_bytes: 1024,
    };
    assert_eq!(
        resources.load(request).unwrap().bytes(),
        builtin::SOLID_BYTES
    );
    assert_eq!(
        resources.load(ResourceRequest {
            revision: 2,
            ..request
        }),
        Err(ResourceError::DigestMismatch)
    );
    assert_eq!(
        resources.load(ResourceRequest {
            maximum_bytes: 1,
            ..request
        }),
        Err(ResourceError::LimitExceeded)
    );
    assert_eq!(
        resources.load(ResourceRequest {
            id: ResourceId(12),
            ..request
        }),
        Err(ResourceError::NotFound)
    );
}

#[cfg(feature = "prototype")]
fn fonts() -> FontPack {
    let root = std::path::PathBuf::from(
        std::env::var_os("TILEWRIGHT_UI_FONT_ROOT")
            .expect("explicit TILEWRIGHT_UI_FONT_ROOT required"),
    );
    let fonts = tilewright_render_ui::unicode_fonts::FONTS
        .iter()
        .map(|font| {
            let bytes = std::fs::read(root.join(font.path)).unwrap();
            assert_eq!(bytes.len(), font.bytes, "font byte identity");
            FontBlob {
                family: font.family.into(),
                bytes: bytes.into(),
                sha256: font.sha256,
            }
        })
        .collect();
    FontPack::new(fonts, TextLimits::default()).unwrap()
}

/// This test deliberately requires explicit verified external font bytes.
#[test]
#[cfg(feature = "prototype")]
#[allow(clippy::float_cmp)] // Exact retained geometry is a regression contract.
#[allow(clippy::too_many_lines)] // One engine lifetime exercises retained in-flight frames.
fn unicode_geometry_dpi_resources_and_cpu_reference_frame() {
    let mut ui = Ui::new(Limits::default()).unwrap();
    ui.apply(Input::Focus(Some(tilewright_ui::COMPOSER)))
        .unwrap();
    let original = "A\u{301} אבג مرحبا नमस्ते 👩‍💻";
    ui.apply(Input::TextCommit {
        target: ui.text_target().unwrap(),
        text: original.into(),
    })
    .unwrap();
    let snapshot = ui.snapshot();
    let layout =
        tilewright_ui::layout(&snapshot, Viewport::new(320.0, 200.0, 2.0).unwrap()).unwrap();
    let namespace = Namespace {
        first_semantic: 100,
        solid_resource: 100,
        atlas_resource: 101,
    };
    let mut renderer = UiRenderer::new(
        fonts(),
        TextLimits::default(),
        namespace,
        UiLimits::default(),
    )
    .unwrap();
    let first = renderer.compile(&snapshot, &layout, [640, 400]).unwrap();
    let editable = first.editable().unwrap();
    assert_eq!(editable.layout.text(), original);
    assert!(
        editable
            .layout
            .grapheme_boundaries()
            .contains(&original.len())
    );
    assert!(!editable.layout.grapheme_boundaries().contains(&1));
    let caret = editable.caret().unwrap();
    assert!(editable.clip.contains(caret.x, caret.y));
    assert!(editable.hit([caret.x, caret.y]).is_some());
    assert!(editable.hit([-10.0, -10.0]).is_none());
    for quad in &first.quads {
        assert!(quad.positions.iter().all(|position| position[0] >= 0.0
            && position[0] <= 640.0
            && position[1] >= 0.0
            && position[1] <= 400.0));
    }
    let frame = first.standalone([0.0; 4]).unwrap().render_rgba().unwrap();
    assert_eq!((frame.width, frame.height), (640, 400));
    assert!(frame.rgba8.chunks_exact(4).any(|pixel| pixel[3] != 0));
    let again = renderer.compile(&snapshot, &layout, [640, 400]).unwrap();
    assert_eq!(first.quads, again.quads);
    assert_eq!(
        frame,
        again.standalone([0.0; 4]).unwrap().render_rgba().unwrap()
    );
    let one = renderer.compile(&snapshot, &layout, [320, 200]).unwrap();
    assert_eq!(
        first.editable().unwrap().origin,
        one.editable().unwrap().origin
    );
    assert_eq!(
        first.editable().unwrap().layout.size(),
        one.editable().unwrap().layout.size()
    );
    ui.apply(Input::Selection {
        target: ui.text_target().unwrap(),
        selection: tilewright_ui::TextSelection {
            anchor: tilewright_ui::TextCursor::new(0),
            focus: tilewright_ui::TextCursor::new(3),
        },
    })
    .unwrap();
    let selected_snapshot = ui.snapshot();
    let selected_layout = tilewright_ui::layout(&selected_snapshot, layout.viewport).unwrap();
    let selected = renderer
        .compile(&selected_snapshot, &selected_layout, [640, 400])
        .unwrap();
    assert!(
        selected
            .quads
            .iter()
            .any(|quad| quad.color == [0.23, 0.40, 0.62, 1.0])
    );
    renderer.clear_cache();
    ui.apply(Input::Preedit {
        target: ui.text_target().unwrap(),
        text: "e\u{301}".into(),
        start: 0,
        length: 1,
    })
    .unwrap();
    let next_snapshot = ui.snapshot();
    let next_layout = tilewright_ui::layout(&next_snapshot, layout.viewport).unwrap();
    let next = renderer
        .compile(&next_snapshot, &next_layout, [640, 400])
        .unwrap();
    assert!(
        next.editable()
            .unwrap()
            .snapshot
            .as_ref()
            .unwrap()
            .composition
            .is_some()
    );
    assert_eq!(first.editable().unwrap().layout.text(), original);
    drop(renderer);
    assert_eq!(
        frame,
        first.standalone([0.0; 4]).unwrap().render_rgba().unwrap()
    );
    let mut stale = next_snapshot.clone();
    stale.revision = stale.revision.next().unwrap();
    let mut renderer = UiRenderer::new(
        fonts(),
        TextLimits::default(),
        namespace,
        UiLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        renderer.compile(&stale, &next_layout, [640, 400]),
        Err(Error::StaleSnapshot)
    ));
    let mut bad_layout = next_layout.clone();
    bad_layout.paint.push(Paint::Rectangle {
        bounds: Rect {
            x: f32::INFINITY,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        clip: Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        color: [1.0; 4],
    });
    assert!(
        renderer
            .compile(&next_snapshot, &bad_layout, [640, 400])
            .is_err()
    );
    renderer.limits.maximum_primitives = 1;
    assert!(matches!(
        renderer.compile(&next_snapshot, &next_layout, [640, 400]),
        Err(Error::LimitExceeded)
    ));
    let world = SceneBundle::from_quads(
        1,
        0,
        tilewright_scene::Viewport {
            width: 640,
            height: 400,
            scale_milli: 2000,
        },
        [0.0; 4],
        first.quads.iter().copied().take(1),
        SceneLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        first.compose(&world, first.standalone([0.0; 4]).unwrap().resources),
        Err(Error::IdentityCollision)
    ));
}
