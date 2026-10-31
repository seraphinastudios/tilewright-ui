use tilewright_app::*;
use tilewright_ui::{COMPOSER, Input, Limits};
fn viewport() -> Viewport {
    Viewport::new(800.0, 600.0, 1.0).unwrap()
}
#[test]
fn only_exact_acknowledged_current_geometry_receives_input() {
    let mut app = App::new(Limits::default()).unwrap();
    let frame = app.prepare(viewport()).unwrap();
    assert_eq!(app.hit_test(frame.stamp, 20.0, 570.0), Err(AppError::Stale));
    assert!(app.displayed().is_none());
    app.ack_presented(frame.stamp).unwrap();
    assert_eq!(
        app.hit_test(frame.stamp, 20.0, 570.0).unwrap().unwrap().id,
        COMPOSER
    );
    app.apply(Input::Focus(Some(COMPOSER))).unwrap();
    assert_eq!(app.hit_test(frame.stamp, 20.0, 570.0), Err(AppError::Stale));
}
#[test]
fn failed_and_reordered_presentations_do_not_roll_back_display() {
    let mut app = App::new(Limits::default()).unwrap();
    let first = app.prepare(viewport()).unwrap();
    app.ack_presented(first.stamp).unwrap();
    let failed = app.prepare(viewport()).unwrap();
    assert_eq!(app.displayed().unwrap().stamp, first.stamp);
    let latest = app.prepare(viewport()).unwrap();
    app.ack_presented(latest.stamp).unwrap();
    assert_eq!(app.ack_presented(failed.stamp), Err(AppError::Stale));
    app.ack_presented(latest.stamp).unwrap();
    assert_eq!(app.displayed().unwrap().stamp, latest.stamp);
    app.discard_prepared(failed.stamp).unwrap();
}
#[test]
fn resize_and_surface_recreation_retire_old_stamps() {
    let mut app = App::new(Limits::default()).unwrap();
    let old = app.prepare(viewport()).unwrap();
    app.ack_presented(old.stamp).unwrap();
    let resized = app
        .prepare(Viewport::new(900.0, 600.0, 1.5).unwrap())
        .unwrap();
    assert!(resized.stamp.surface > old.stamp.surface);
    assert!(app.displayed().is_none());
    assert_eq!(app.ack_presented(old.stamp), Err(AppError::Stale));
    app.ack_presented(resized.stamp).unwrap();
    app.invalidate_surface().unwrap();
    assert_eq!(app.ack_presented(resized.stamp), Err(AppError::Stale));
}
#[test]
fn frame_capacity_is_bounded_until_host_retires_inflight_frames() {
    let mut app = App::new(Limits {
        prepared_frames: 1,
        ..Limits::default()
    })
    .unwrap();
    let first = app.prepare(viewport()).unwrap();
    assert_eq!(app.prepare(viewport()), Err(AppError::QueueFull));
    app.ack_presented(first.stamp).unwrap();
    let second = app.prepare(viewport()).unwrap();
    assert_eq!(app.displayed().unwrap().stamp, first.stamp);
    assert_eq!(app.prepare(viewport()), Err(AppError::QueueFull));
    app.discard_prepared(second.stamp).unwrap();
    assert!(app.prepare(viewport()).is_ok());
}
#[test]
fn stale_text_layout_and_pending_patch_are_rejected() {
    let mut app = App::new(Limits::default()).unwrap();
    app.apply(Input::Focus(Some(COMPOSER))).unwrap();
    let first = app.prepare(viewport()).unwrap();
    app.ack_presented(first.stamp).unwrap();
    let target = app.ui().text_target().unwrap();
    app.apply(Input::TextCommit {
        target,
        text: "edit".into(),
    })
    .unwrap();
    let pending = app.prepare(viewport()).unwrap();
    assert_eq!(
        app.select_text(first.stamp, target, tilewright_ui::TextSelection::default()),
        Err(AppError::Stale)
    );
    assert_eq!(
        app.hit_test(pending.stamp, 20.0, 570.0),
        Err(AppError::Stale)
    );
    app.ack_presented(pending.stamp).unwrap();
    let target = app.ui().text_target().unwrap();
    app.select_text(
        pending.stamp,
        target,
        tilewright_ui::TextSelection::default(),
    )
    .unwrap();
}

#[test]
fn surface_invalidation_is_a_durable_control_barrier() {
    let mut app = App::new(Limits::default()).unwrap();
    let before = app.ui().control();
    app.invalidate_surface().unwrap();
    let after = app.ui().control();
    assert!(after.epoch > before.epoch);
    assert!(after.world_input_allowed);
}

#[test]
fn acknowledging_output_preserves_current_presentation_identity() {
    let mut app = App::new(Limits::default()).unwrap();
    app.apply(Input::Focus(Some(COMPOSER))).unwrap();
    app.apply(Input::TextCommit {
        target: app.ui().text_target().unwrap(),
        text: "sent".into(),
    })
    .unwrap();
    app.apply(Input::Activate(tilewright_ui::SUBMIT)).unwrap();
    let batch = app.ui().peek_actions().unwrap().id;
    let frame = app.prepare(viewport()).unwrap();
    app.ack_presented(frame.stamp).unwrap();
    app.ack_actions(batch).unwrap();
    app.ack_actions(batch).unwrap();
    assert!(app.ui().peek_actions().is_none());
    assert!(app.validate_displayed(frame.stamp).is_ok());
}
