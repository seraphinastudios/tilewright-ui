// Copyright 2026 Seraphina Studios
// SPDX-License-Identifier: MIT

//! The single field's style (`LayoutOptions::field_style`, since 0.1.2).
#![allow(clippy::float_cmp)] // Layout geometry is exact arithmetic on given values.

use tilewright_ui::*;

fn viewport() -> Viewport {
    Viewport::new(800.0, 600.0, 1.0).unwrap()
}

fn field(text: &str) -> UiSnapshot {
    let mut ui = Ui::with_composition(Limits::default(), Composition::SingleField).unwrap();
    ui.bind_field(text).unwrap();
    ui.apply(Input::Focus(Some(COMPOSER))).unwrap();
    ui.snapshot()
}

fn with_style(style: FieldStyle) -> LayoutOptions {
    let mut options = LayoutOptions::default();
    options.field_style = style;
    options
}

const ROW: Rect = Rect {
    x: 100.0,
    y: 200.0,
    width: 300.0,
    height: 52.0,
};

#[test]
fn the_default_style_is_the_0_1_1_look() {
    let style = FieldStyle::default();
    assert_eq!(style.text_size, 20.0);
    assert_eq!(style.height, 36.0);
    assert_eq!(style.inset, 8.0);
    assert_eq!(
        [style.padding_left, style.padding_top, style.padding_right],
        [0.0; 3]
    );
    assert_eq!(style.text_color, [1.0; 4]);
    assert_eq!(style.box_color, Some([0.18, 0.2, 0.25, 1.0]));
    assert_eq!(style.selection_color, [0.23, 0.40, 0.62, 1.0]);
    assert_eq!(style.caret_color, None);
    assert_eq!(style.preedit_color, None);
    assert_eq!(LayoutOptions::default().field_style, style);

    // The 0.1.1 single-field layout, written out.
    let snapshot = field("draft");
    let layout = layout_in(&snapshot, viewport(), ROW, ROW).unwrap();
    assert_eq!(layout.field_style(), &style);
    let composer = Rect {
        x: 108.0,
        y: 208.0,
        width: 196.0,
        height: 36.0,
    };
    assert_eq!(layout.composer_bounds, Some(composer));
    assert_eq!(
        layout.paint[0],
        Paint::Rectangle {
            bounds: composer,
            clip: ROW,
            color: [0.18, 0.2, 0.25, 1.0],
        }
    );
    assert!(matches!(
        &layout.paint[1],
        Paint::Text { bounds, font: FontRole::Composer, color, .. }
            if *bounds == composer && *color == [1.0; 4]
    ));
    assert_eq!(layout.paint.len(), 4);
    // A default-style layout prints as 0.1.1's did: no style member.
    assert!(!format!("{layout:?}").contains("field_style"));
}

#[test]
fn the_geometry_and_colours_follow_the_style() {
    let mut style = FieldStyle::default();
    style.inset = 0.0;
    style.height = 30.0;
    style.text_size = 18.0;
    style.text_color = [0.5, 0.6, 0.7, 1.0];
    style.box_color = Some([0.1, 0.2, 0.3, 1.0]);
    let mut options = with_style(style.clone());
    options.submit_control = false;
    let row = Rect {
        height: 30.0,
        ..ROW
    };
    let layout = layout_in_with(&field("draft"), viewport(), row, row, &options).unwrap();
    assert_eq!(layout.composer_bounds, Some(row));
    assert_eq!(layout.field_style(), &style);
    assert_eq!(
        layout.paint,
        vec![
            Paint::Rectangle {
                bounds: row,
                clip: row,
                color: [0.1, 0.2, 0.3, 1.0],
            },
            Paint::Text {
                widget: field("draft").composer_widget,
                bounds: row,
                clip: row,
                text: "draft".into(),
                font: FontRole::Composer,
                color: [0.5, 0.6, 0.7, 1.0],
                text_revision: Some(field("draft").composer.revision),
            },
        ]
    );
    assert!(format!("{layout:?}").contains("field_style"));

    // A shorter box sits at the bottom of the inset attachment.
    let mut short = FieldStyle::default();
    short.height = 20.0;
    short.inset = 4.0;
    let layout = layout_in_with(&field(""), viewport(), ROW, ROW, &with_style(short)).unwrap();
    let composer = layout.composer_bounds.unwrap();
    assert_eq!(
        [composer.x, composer.y, composer.height],
        [104.0, 228.0, 20.0]
    );
    // The Apply control keeps its fixed width and gap inside the inset.
    assert_eq!(composer.width, 300.0 - 8.0 - 80.0 - 8.0);

    // No box: the host draws the background.
    let mut bare = FieldStyle::default();
    bare.box_color = None;
    let layout = layout_in_with(&field(""), viewport(), ROW, ROW, &with_style(bare)).unwrap();
    assert!(matches!(
        layout.paint[0],
        Paint::Text {
            font: FontRole::Composer,
            ..
        }
    ));
    assert_eq!(layout.paint.len(), 3);
}

#[test]
fn nonsensical_styles_are_refused() {
    let snapshot = field("draft");
    let mut refused: Vec<FieldStyle> = Vec::new();
    for size in [0.0, 0.5, 256.5, f32::NAN, f32::INFINITY, -18.0] {
        let mut style = FieldStyle::default();
        style.text_size = size;
        refused.push(style);
    }
    for value in [-1.0, f32::NAN, f32::INFINITY, 8192.5] {
        for field in 0..5 {
            let mut style = FieldStyle::default();
            *[
                &mut style.height,
                &mut style.inset,
                &mut style.padding_left,
                &mut style.padding_top,
                &mut style.padding_right,
            ][field] = value;
            refused.push(style);
        }
    }
    let mut style = FieldStyle::default();
    style.height = 0.0;
    refused.push(style);
    for channel in [-0.1, 1.1, f32::NAN] {
        let colour = [0.5, channel, 0.5, 1.0];
        for field in 0..5 {
            let mut style = FieldStyle::default();
            match field {
                0 => style.text_color = colour,
                1 => style.box_color = Some(colour),
                2 => style.selection_color = colour,
                3 => style.caret_color = Some(colour),
                _ => style.preedit_color = Some(colour),
            }
            refused.push(style);
        }
    }
    for style in refused {
        assert_eq!(
            layout_in_with(&snapshot, viewport(), ROW, ROW, &with_style(style.clone())),
            Err(UiError::InvalidInput),
            "{style:?}"
        );
        // Even for an empty attachment.
        let empty = Rect { width: 0.0, ..ROW };
        assert_eq!(
            layout_in_with(&snapshot, viewport(), empty, empty, &with_style(style)),
            Err(UiError::InvalidInput)
        );
    }
    // The bounds of each range are accepted.
    let mut style = FieldStyle::default();
    style.text_size = 256.0;
    style.inset = 8192.0;
    style.padding_left = 0.0;
    style.padding_right = 8192.0;
    style.caret_color = Some([0.0, 1.0, 0.0, 1.0]);
    assert!(layout_in_with(&snapshot, viewport(), ROW, ROW, &with_style(style.clone())).is_ok());
    style.text_size = 1.0;
    style.height = f32::MIN_POSITIVE;
    assert!(layout_in_with(&snapshot, viewport(), ROW, ROW, &with_style(style)).is_ok());
}

#[test]
fn only_the_single_field_takes_a_style() {
    let mut style = FieldStyle::default();
    style.text_size = 18.0;
    for composition in [Composition::ChatPanel, Composition::Demo] {
        let ui = Ui::with_composition(Limits::default(), composition).unwrap();
        assert_eq!(
            layout_in_with(
                &ui.snapshot(),
                viewport(),
                ROW,
                ROW,
                &with_style(style.clone())
            ),
            Err(UiError::InvalidInput)
        );
        // The default style is the one they have always used.
        let layout = layout_in_with(
            &ui.snapshot(),
            viewport(),
            ROW,
            ROW,
            &with_style(FieldStyle::default()),
        )
        .unwrap();
        assert_eq!(
            layout,
            layout_in(&ui.snapshot(), viewport(), ROW, ROW).unwrap()
        );
        assert_eq!(layout.field_style(), &FieldStyle::default());
    }
}
