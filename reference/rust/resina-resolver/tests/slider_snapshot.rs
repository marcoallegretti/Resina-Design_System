use resina_color::OpaqueSrgbRange;
use resina_environment::{LayoutDirection, SafeArea};
use resina_model::PhysicalVector;
use resina_resolver::{
    CommandLabelInput, HitRegionError, SliderAccessibilityError, SliderOrientation,
    SliderSnapshotError, resolve_command_label, resolve_slider_snapshot,
};
use serde_json::json;
use std::convert::Infallible;
#[path = "common/slider_snapshot.rs"]
mod fixture;
use fixture::{Fixture, FixtureConfig, bounds, color, layout, reserve, size};

impl Fixture {
    fn new(
        family: &str,
        direction: LayoutDirection,
        orientation: SliderOrientation,
        enabled: bool,
        focused: bool,
        read_only: bool,
        preview: bool,
    ) -> Self {
        Self::resolve(
            FixtureConfig {
                family,
                direction,
                orientation,
                enabled,
                focused,
                read_only,
                preview,
                tracking: 0.25,
                text_scale: 1.0,
                text: "Volume",
                label_maximum_size: size(100.0, 64.0),
            },
            |input| {
                Ok::<_, Infallible>(size(
                    80.0,
                    input.typography.font_size() * input.typography.line_height(),
                ))
            },
        )
    }
}
#[test]
fn coherent_parts_compose_across_families_directions_axes_and_permissions() {
    for family in ["cast", "frost", "elastomer"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for orientation in [SliderOrientation::Horizontal, SliderOrientation::Vertical] {
                for enabled in [false, true] {
                    for focused in [false, true] {
                        for read_only in [false, true] {
                            let f = Fixture::new(
                                family,
                                direction,
                                orientation,
                                enabled,
                                focused,
                                read_only,
                                false,
                            );
                            let mut input = f.input();
                            input.focusable = enabled || focused;
                            let snapshot = resolve_slider_snapshot(input).unwrap();
                            assert_eq!(
                                snapshot.layout().value(),
                                snapshot.presentation().visible()
                            );
                            assert_eq!(
                                snapshot.accessibility().value(),
                                snapshot.presentation().visible().value()
                            );
                            assert_eq!(snapshot.accessibility().orientation(), orientation);
                            assert_eq!(snapshot.accessibility().state().read_only(), read_only);
                            assert_eq!(snapshot.accessibility().state().focused(), focused);
                            assert_eq!(snapshot.accessibility().state().enabled(), enabled);
                            assert_eq!(snapshot.accessibility().focusable(), enabled || focused);
                            assert_eq!(
                                snapshot.accessibility().actions()[0].available(),
                                enabled && !read_only
                            );
                            assert_eq!(snapshot.hit_region().bounds(), f.target.bounds());
                            assert!(snapshot.thumb_contrast_ratio() >= 3.0);
                            assert!(snapshot.track_contrast_ratio() >= 3.0);
                            assert_eq!(snapshot.label_contrast_ratio(), 21.0);
                            assert_eq!(snapshot.label().text(), "Volume");
                            assert!(snapshot.reduced_motion());
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn pointer_preview_drives_layout_paint_and_semantics_without_committing() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        true,
    );
    assert_ne!(f.presentation.visible(), f.presentation.committed());
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    assert_eq!(
        snapshot.accessibility().value(),
        f.presentation.visible().value()
    );
    assert_eq!(
        snapshot.thumb().phase(),
        resina_model::SliderPhase::Dragging
    );
    let stale = layout(
        f.presentation.committed(),
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
    );
    let mut input = f.input();
    input.layout = &stale;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("current visible value")
    );
    let mut input = f.input();
    input.hit_region = reserve(&f.environment, bounds(-16.0, -12.0, 324.0, 196.0));
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("pointer ownership")
    );
}
#[test]
fn stale_states_size_direction_and_part_slots_are_rejected() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let focused = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        false,
    );
    let mut input = f.input();
    input.thumb = &focused.thumb;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("part states")
    );
    let mut input = f.input();
    input.interaction.read_only = true;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("readOnly")
    );
    let mut input = f.input();
    input.track = &f.thumb;
    input.thumb = &f.track;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("declared slots")
    );
    let rtl = layout(
        f.presentation.visible(),
        LayoutDirection::Rtl,
        SliderOrientation::Horizontal,
    );
    let mut input = f.input();
    input.layout = &rtl;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("direction")
    );
    let rtl_paint = Fixture::new(
        "cast",
        LayoutDirection::Rtl,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut input = f.input();
    input.track = &rtl_paint.track;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("part paint direction")
    );
    let vertical = layout(
        f.presentation.visible(),
        LayoutDirection::Ltr,
        SliderOrientation::Vertical,
    );
    let mut input = f.input();
    input.layout = &vertical;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("front size")
    );
}
#[test]
fn actual_ranges_labels_and_reserved_target_are_verified() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        false,
    );
    let dark = color(0.0);
    let dark_canvas = [OpaqueSrgbRange::try_new(dark.clone(), dark).unwrap()];
    let mut input = f.input();
    input.canvas_ranges = &dark_canvas;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("track edge contrast")
    );
    let mut input = f.input();
    input.canvas_ranges = &[];
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("canvas ranges")
    );
    let dimmer = color(0.9);
    let dimmer_canvas = [OpaqueSrgbRange::try_new(dimmer.clone(), dimmer).unwrap()];
    let mut input = f.input();
    input.canvas_ranges = &dimmer_canvas;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::OverstatedContrast {
            stage: "track edge",
            ..
        })
    ));
    let mut input = f.input();
    input.label_foreground = &f.white;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label contrast")
    );
    let mut input = f.input();
    input.label_origin = PhysicalVector { x: 20.0, y: 8.0 };
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label must not overlap")
    );
    let mut input = f.input();
    input.hit_region = reserve(&f.environment, bounds(20.0, -12.0, 288.0, 196.0));
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("swept thumb")
    );
    let mut input = f.input();
    input.minimum_thumb_contrast = f64::NAN;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("thumb edge threshold")
    );
    let mut input = f.input();
    input.focusable = false;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Accessibility(
            SliderAccessibilityError::EnabledNotFocusable
        ))
    ));
    let mut input = f.input();
    input.available_bounds = bounds(0.0, 0.0, 200.0, 160.0);
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Target(HitRegionError::Clipped))
    ));
    let occupied = [bounds(300.0, 0.0, 20.0, 20.0)];
    let mut input = f.input();
    input.occupied_regions = &occupied;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Target(HitRegionError::Occupied(0)))
    ));
}

#[test]
fn different_track_appearance_cannot_validate_stale_thumb_reports() {
    let frost = Fixture::new(
        "frost",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let cast = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut input = frost.input();
    input.thumb = &cast.thumb;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::OverstatedContrast {
            stage: "thumb edge",
            ..
        })
    ));
}

#[test]
fn label_between_endpoints_cannot_overlap_swept_thumb() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let label = resolve_command_label(
        CommandLabelInput {
            text: "L",
            typography: f.label.typography(),
            minimum_size: size(20.0, 12.0),
            maximum_size: size(20.0, 12.0),
            padding: SafeArea {
                start: 0.0,
                end: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| Ok::<_, Infallible>(size(20.0, 12.0)),
    )
    .unwrap();
    let mut input = f.input();
    input.label = &label;
    input.label_origin = PhysicalVector { x: 80.0, y: 5.0 };
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label must not overlap")
    );
}

#[test]
fn published_colors_and_preferences_do_not_borrow_live_context() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut foreground = f.black.clone();
    let mut environment = f.environment.clone();
    let mut input = f.input();
    input.label_foreground = &foreground;
    input.environment = &environment;
    let snapshot = resolve_slider_snapshot(input).unwrap();
    foreground = color(1.0);
    let mut updated = serde_json::to_value(&environment).unwrap();
    updated["accessibilityPreferences"]["reducedMotion"] = json!(false);
    environment = serde_json::from_value(updated).unwrap();
    assert_eq!(foreground.components(), [1.0; 3]);
    assert!(!environment.accessibility_preferences().reduced_motion);
    assert_eq!(snapshot.label_foreground().components(), [0.0; 3]);
    assert!(snapshot.reduced_motion());
}
