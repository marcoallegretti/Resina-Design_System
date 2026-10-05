use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalVector, SliderValue, SurfaceSize};
use resina_resolver::{
    SliderAnchorError, SliderEditAction, SliderEditInput, SliderEditSession, SliderLayoutInput,
    SliderLayoutIr, SliderMinimumPosition, SliderOrientation, SliderPointerAnchor,
    SliderPositionInput, resolve_slider_edit, resolve_slider_layout, resolve_slider_position,
    resolve_slider_value,
};
use serde::Deserialize;
use serde_json::Value;
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Layout {
    allocation_size: SurfaceSize,
    thumb_size: SurfaceSize,
    track_thickness: f64,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    value: SliderValue,
}
impl Layout {
    fn resolve(&self) -> SliderLayoutIr {
        let value = resolve_slider_value(&self.value).unwrap();
        resolve_slider_layout(SliderLayoutInput {
            allocation_size: self.allocation_size,
            thumb_size: self.thumb_size,
            track_thickness: self.track_thickness,
            insets: &self.insets,
            layout_direction: self.layout_direction,
            orientation: self.orientation,
            minimum_position: self.minimum_position,
            value: &value,
        })
        .unwrap_or_else(|error| panic!("{error:?}: {self:?}"))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    mode: String,
    initial_layout: Layout,
    current_layout: Layout,
    grab_point: Option<PhysicalVector>,
    point: PhysicalVector,
    expected: Option<f64>,
    error: Option<String>,
}
fn cases() -> Vec<Case> {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-anchor-cases.json"
    ))
    .unwrap();
    assert_eq!(source["schemaVersion"], "0.1.0");
    serde_json::from_value(source["cases"].clone()).unwrap()
}
#[test]
fn public_cases_preserve_grab_distance_and_center_track_jumps() {
    let cases = cases();
    assert_eq!(cases.len(), 88);
    for case in cases {
        let initial = case.initial_layout.resolve();
        let current = case.current_layout.resolve();
        let anchor = match case.mode.as_str() {
            "grab" => SliderPointerAnchor::grab(&initial, case.grab_point.unwrap()),
            "center" => SliderPointerAnchor::center(&initial),
            _ => panic!("unknown anchor mode"),
        }
        .unwrap();
        let result = anchor.desired_origin(&current, case.point);
        if let Some(error) = case.error {
            assert!(case.expected.is_none());
            assert_eq!(error, "incompatibleLayout");
            assert_eq!(
                result.unwrap_err(),
                SliderAnchorError::IncompatibleLayout,
                "{}",
                case.name
            );
        } else {
            assert_eq!(result.unwrap(), case.expected.unwrap(), "{}", case.name);
        }
    }
}
#[test]
fn grab_does_not_jump_and_cross_axis_motion_does_not_adjust_value() {
    let case = cases().remove(0);
    let layout = case.initial_layout.resolve();
    let grab = case.grab_point.unwrap();
    let anchor = SliderPointerAnchor::grab(&layout, grab).unwrap();
    for point in [
        grab,
        PhysicalVector {
            x: grab.x,
            y: -1e300,
        },
    ] {
        let desired_origin = anchor.desired_origin(&layout, point).unwrap();
        let result = resolve_slider_position(SliderPositionInput {
            layout: &layout,
            desired_origin,
            enabled: true,
            read_only: false,
        })
        .unwrap();
        assert!(result.accepted());
        assert!(!result.changed());
        assert_eq!(result.value(), layout.value());
    }
}
#[test]
fn nonfinite_coordinates_fail_even_on_unused_axis_and_stale_layout() {
    let all = cases();
    let case = &all[0];
    let initial = case.initial_layout.resolve();
    let incompatible = all
        .iter()
        .find(|c| c.name.starts_with("changed orientation"))
        .unwrap()
        .current_layout
        .resolve();
    let anchor = SliderPointerAnchor::grab(&initial, case.grab_point.unwrap()).unwrap();
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for point in [
            PhysicalVector { x: n, y: 0.0 },
            PhysicalVector { x: 0.0, y: n },
        ] {
            assert_eq!(
                SliderPointerAnchor::grab(&initial, point).unwrap_err(),
                SliderAnchorError::InvalidPoint
            );
            for layout in [&initial, &incompatible] {
                let error = anchor.desired_origin(layout, point).unwrap_err();
                assert_eq!(error, SliderAnchorError::InvalidPoint);
                assert!(error.to_string().contains("finite"));
                assert!(std::error::Error::source(&error).is_none());
            }
        }
    }
}
#[test]
fn unrepresentable_displacement_and_lost_motion_fail_explicitly() {
    let layout = cases().remove(0).initial_layout.resolve();
    let anchor = SliderPointerAnchor::grab(
        &layout,
        PhysicalVector {
            x: -f64::MAX,
            y: 0.0,
        },
    )
    .unwrap();
    assert_eq!(
        anchor
            .desired_origin(
                &layout,
                PhysicalVector {
                    x: f64::MAX,
                    y: 0.0
                }
            )
            .unwrap_err(),
        SliderAnchorError::NumericRange
    );
    let mut huge = cases().remove(0).initial_layout;
    huge.allocation_size.width = 2.0_f64.powi(1004);
    huge.thumb_size.width = 2.0_f64.powi(980);
    huge.insets = SafeArea {
        start: 0.0,
        end: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    let layout = huge.resolve();
    let anchor = SliderPointerAnchor::grab(&layout, PhysicalVector { x: 0.0, y: 0.0 }).unwrap();
    assert_eq!(
        anchor
            .desired_origin(&layout, PhysicalVector { x: 1.0, y: 0.0 })
            .unwrap_err(),
        SliderAnchorError::NumericRange
    );
}

#[test]
fn unrepresentable_center_is_not_replaced_with_an_edge() {
    let mut tiny = cases().remove(0).initial_layout;
    tiny.allocation_size.width = f64::from_bits(10);
    tiny.thumb_size.width = f64::from_bits(3);
    tiny.insets.start = 0.0;
    tiny.insets.end = 0.0;
    tiny.value = SliderValue::try_new(0.0, 1.0, 0.0).unwrap();
    assert_eq!(
        SliderPointerAnchor::center(&tiny.resolve()).unwrap_err(),
        SliderAnchorError::NumericRange
    );

    let mut large = cases().remove(0).initial_layout;
    large.allocation_size.width = 2.0_f64.powi(53) + 4.0;
    large.thumb_size.width = 2.0;
    large.insets.start = 2.0;
    large.insets.end = 0.0;
    large.value = SliderValue::try_new(0.0, 1.0, 1.0 - 2.0_f64.powi(-52)).unwrap();
    assert_eq!(
        SliderPointerAnchor::center(&large.resolve()).unwrap_err(),
        SliderAnchorError::NumericRange
    );
}

#[test]
fn one_grab_anchor_drives_preview_frames_and_one_final_commit() {
    let mut visual = cases().remove(0).initial_layout;
    let initial = visual.resolve();
    let committed = *initial.value();
    let anchor = SliderPointerAnchor::grab(&initial, PhysicalVector { x: 45.0, y: 17.0 }).unwrap();
    let mut session = SliderEditSession::begin(&committed, "initial").unwrap();
    for (pointer, value) in [(75.0, 10.0), (105.0, 20.0)] {
        let layout = visual.resolve();
        let desired_origin = anchor
            .desired_origin(
                &layout,
                PhysicalVector {
                    x: pointer,
                    y: 17.0,
                },
            )
            .unwrap();
        let result = resolve_slider_edit(SliderEditInput {
            session: &session,
            current: &committed,
            revision: "initial",
            enabled: true,
            read_only: false,
            action: SliderEditAction::Preview {
                layout: &layout,
                desired_origin,
            },
        })
        .unwrap();
        assert!(result.commit().is_none());
        assert_eq!(result.preview().value().value(), value);
        visual.value = *result.preview().value();
        session = result.session().unwrap().clone();
    }
    let result = resolve_slider_edit(SliderEditInput {
        session: &session,
        current: &committed,
        revision: "initial",
        enabled: true,
        read_only: false,
        action: SliderEditAction::Commit,
    })
    .unwrap();
    assert!(result.session().is_none());
    let commit = result.commit().unwrap();
    assert!(commit.accepted() && commit.changed());
    assert_eq!(commit.value().value().value(), 20.0);
    assert_eq!(committed.value().value(), 0.0);
}
