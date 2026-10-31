use tilewright_app::*;
use tilewright_ui::{COMPOSER, Input, Limits, UiError};

fn viewport() -> Viewport {
    Viewport::new(800.0, 600.0, 1.0).unwrap()
}
fn bounds() -> Rect {
    Rect {
        x: 50.0,
        y: 100.0,
        width: 300.0,
        height: 150.0,
    }
}
fn app() -> App {
    App::with_composition(Limits::default(), Composition::SingleField).unwrap()
}

#[test]
fn moved_bounds_and_clip_retire_geometry_before_another_prepare() {
    let mut app = app();
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    let first = app.prepare_current().unwrap();
    app.ack_presented(first.stamp).unwrap();
    let pending = app.prepare_current().unwrap();
    let epoch = app.ui().control().epoch;
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    assert_eq!(app.surface_epoch(), first.stamp.surface);
    assert_eq!(app.ui().control().epoch, epoch);
    assert!(app.validate_displayed(first.stamp).is_ok());
    let moved = Rect {
        x: 60.0,
        ..bounds()
    };
    app.set_layout(viewport(), moved, moved).unwrap();
    assert!(app.ui().control().epoch > epoch);
    assert!(app.displayed().is_none());
    assert_eq!(app.hit_test(first.stamp, 75.0, 225.0), Err(AppError::Stale));
    assert_eq!(app.ack_presented(pending.stamp), Err(AppError::Stale));
    let second = app.prepare_current().unwrap();
    assert!(second.stamp.surface > first.stamp.surface);
    assert!(second.layout.composer_bounds.unwrap().x > first.layout.composer_bounds.unwrap().x);
    app.ack_presented(second.stamp).unwrap();
    let clipped = Rect {
        width: 100.0,
        ..moved
    };
    app.set_layout(viewport(), moved, clipped).unwrap();
    assert!(app.displayed().is_none());
    assert_eq!(app.ack_presented(second.stamp), Err(AppError::Stale));
    let third = app.prepare_current().unwrap();
    app.ack_presented(third.stamp).unwrap();
    assert_eq!(app.hit_test(third.stamp, 200.0, 225.0).unwrap(), None);
    assert_eq!(
        app.hit_test(third.stamp, 75.0, 225.0).unwrap().unwrap().id,
        COMPOSER
    );
}

#[test]
fn invalid_attachment_is_atomic_and_degenerate_bounds_are_accepted() {
    let mut app = app();
    assert_eq!(app.prepare_current(), Err(AppError::Stale));
    assert_eq!(app.viewport(), None);
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    let frame = app.prepare_current().unwrap();
    app.ack_presented(frame.stamp).unwrap();
    let before = app.ui().snapshot();
    for invalid in [
        Rect {
            x: f32::NAN,
            ..bounds()
        },
        Rect {
            width: -1.0,
            ..bounds()
        },
        Rect {
            x: 790.0,
            ..bounds()
        },
        Rect {
            x: f32::MAX,
            width: f32::MAX,
            ..bounds()
        },
    ] {
        assert_eq!(
            app.set_layout(viewport(), invalid, bounds()),
            Err(AppError::Ui(UiError::InvalidInput))
        );
        assert_eq!(app.ui().snapshot(), before);
        assert_eq!(app.displayed().unwrap(), &frame);
        assert_eq!(app.surface_epoch(), frame.stamp.surface);
    }
    assert!(
        app.set_layout(
            viewport(),
            bounds(),
            Rect {
                height: f32::INFINITY,
                ..bounds()
            }
        )
        .is_err()
    );
    assert!(
        app.set_layout(
            Viewport {
                scale: 0.0,
                ..viewport()
            },
            bounds(),
            bounds()
        )
        .is_err()
    );
    assert_eq!(app.displayed().unwrap(), &frame);
    let zero = Rect {
        width: 0.0,
        height: 0.0,
        ..bounds()
    };
    app.set_layout(viewport(), zero, zero).unwrap();
    let empty = app.prepare_current().unwrap();
    assert!(empty.layout.paint.is_empty());
    assert!(empty.layout.hits.is_empty());
}

#[test]
fn field_action_can_be_drained_without_invalidating_unchanged_presentation() {
    let mut app = app();
    app.bind_field("value").unwrap();
    app.apply(Input::Focus(Some(COMPOSER))).unwrap();
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    let frame = app.prepare_current().unwrap();
    app.ack_presented(frame.stamp).unwrap();
    app.apply(Input::Activate(tilewright_ui::SUBMIT)).unwrap();
    assert!(app.validate_displayed(frame.stamp).is_ok());
    let batch = app.ui().peek_actions().unwrap().id;
    app.ack_actions(batch).unwrap();
    assert!(app.validate_displayed(frame.stamp).is_ok());
    app.apply(Input::Focus(None)).unwrap();
    app.bind_field("value").unwrap();
    assert!(app.validate_displayed(frame.stamp).is_err());
}

#[test]
fn changed_raw_clip_retires_geometry_even_if_effective_clip_is_identical() {
    let mut app = app();
    let wide_clip = Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    app.set_layout(viewport(), bounds(), wide_clip).unwrap();
    let frame = app.prepare_current().unwrap();
    app.ack_presented(frame.stamp).unwrap();
    let epoch = app.ui().control().epoch;
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    assert!(app.surface_epoch() > frame.stamp.surface);
    assert!(app.ui().control().epoch > epoch);
    assert!(app.displayed().is_none());
    let replacement = app.prepare_current().unwrap();
    assert_eq!(replacement.layout.paint, frame.layout.paint);
    assert_eq!(replacement.layout.hits, frame.layout.hits);
    assert_eq!(
        replacement.layout.composer_bounds,
        frame.layout.composer_bounds
    );
    let surface = app.surface_epoch();
    app.set_layout(viewport(), bounds(), bounds()).unwrap();
    assert_eq!(app.surface_epoch(), surface);
}
