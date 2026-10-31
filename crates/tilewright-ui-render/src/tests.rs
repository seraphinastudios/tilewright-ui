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
    // Exact ordered renderer pack manifest at 4488ffb; all bytes stay external.
    let entries = [
        (
            "Noto Sans",
            "fonts/notosans/NotoSans[wdth,wght].ttf",
            "bfb7bb691513f12e734dc346c03a03f784912432d7e3fa8e56efcf906fe86b3d",
        ),
        (
            "Noto Sans Arabic",
            "fonts/notosansarabic/NotoSansArabic[wdth,wght].ttf",
            "63111b5b2e074dd48cc67692e0a2726d86ee94c1c37fe8598257b7b4e87e869e",
        ),
        (
            "Noto Sans Hebrew",
            "fonts/notosanshebrew/NotoSansHebrew[wdth,wght].ttf",
            "7ef36a2c3593758cdb622e1bdef4f84523e92fbc3ccc667438dd80ff54c2de88",
        ),
        (
            "Noto Sans Devanagari",
            "fonts/notosansdevanagari/NotoSansDevanagari[wdth,wght].ttf",
            "14ec4af41f27482216d1c2229f417ff9b1425e1babb014e57d1d40d03229853e",
        ),
        (
            "Noto Sans Bengali",
            "fonts/notosansbengali/NotoSansBengali[wdth,wght].ttf",
            "dcd42978094e584a849c84a51450eeac40c8826057d566ea6d4b9627a403a05a",
        ),
        (
            "Noto Sans Gujarati",
            "fonts/notosansgujarati/NotoSansGujarati[wdth,wght].ttf",
            "9901d8552f1dd5d2c50dbd4caa6f6e174e74e8264f06594ab259ae6e7b1ac428",
        ),
        (
            "Noto Sans Gurmukhi",
            "fonts/notosansgurmukhi/NotoSansGurmukhi[wdth,wght].ttf",
            "1e6f728fa620e566f842d81e220265813faa12771214765d289c98e035adc5f2",
        ),
        (
            "Noto Sans Tamil",
            "fonts/notosanstamil/NotoSansTamil[wdth,wght].ttf",
            "aa3a9b321f4b0bb2c40203ffbde9af89713227866e0e13f76e5b9eeea727cf88",
        ),
        (
            "Noto Sans Telugu",
            "fonts/notosanstelugu/NotoSansTelugu[wdth,wght].ttf",
            "e618af7bf999df192ed4f388eba2e563f2b5015034e9cbb317b5bd793bd7334d",
        ),
        (
            "Noto Sans Kannada",
            "fonts/notosanskannada/NotoSansKannada[wdth,wght].ttf",
            "cca4f3b3a8cb12fb261f1b43baf5d2f7f59d90fe123d41f0065ed3a183997ec9",
        ),
        (
            "Noto Sans Malayalam",
            "fonts/notosansmalayalam/NotoSansMalayalam[wdth,wght].ttf",
            "312e0e7c3cc15fa09eb42a8f749eeb246b593ed420e3c81aafe8d910c3a6fb56",
        ),
        (
            "Noto Sans Oriya",
            "fonts/notosansoriya/NotoSansOriya[wdth,wght].ttf",
            "910342275d619081b896b4ed25fd9ad6ff320c25591b810de1fcc4beb153ba82",
        ),
        (
            "Noto Sans Sinhala",
            "fonts/notosanssinhala/NotoSansSinhala[wdth,wght].ttf",
            "9bd93e407a278075be403324063bc94a7e306c44de4df81214e932330c22eecf",
        ),
        (
            "Noto Sans Thai",
            "fonts/notosansthai/NotoSansThai[wdth,wght].ttf",
            "5a1c559bb539583c8a1fd99d1c5b9491e5e14478c9cd2bd0970d5c3096cc9ef8",
        ),
        (
            "Noto Sans SC",
            "fonts/notosanssc/NotoSansSC[wght].ttf",
            "a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da",
        ),
        (
            "Noto Sans KR",
            "fonts/notosanskr/NotoSansKR[wght].ttf",
            "194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252",
        ),
        (
            "Noto Emoji",
            "fonts/notoemoji/NotoEmoji[wght].ttf",
            "de6c18832938afc99caf132b39d6a30a19bac7f2e812e28db2535b4608d27551",
        ),
    ];
    let fonts = entries
        .into_iter()
        .map(|(family, path, hex)| {
            let mut sha256 = [0_u8; 32];
            for (index, byte) in sha256.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
            }
            FontBlob {
                family: family.into(),
                bytes: std::fs::read(root.join(path)).unwrap().into(),
                sha256,
            }
        })
        .collect();
    FontPack::new(fonts, TextLimits::default()).unwrap()
}

/// This test deliberately requires explicit verified external font bytes.
#[test]
#[cfg(feature = "prototype")]
#[allow(clippy::float_cmp)] // Exact retained geometry is a regression contract.
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
