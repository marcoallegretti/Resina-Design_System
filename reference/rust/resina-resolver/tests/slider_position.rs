use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderValue, SurfaceSize};
use resina_resolver::{
    SliderLayoutError, SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    SliderPositionError, SliderPositionInput, SliderValueIr, resolve_slider_layout,
    resolve_slider_position, resolve_slider_value,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutFixture {
    allocation_size: SurfaceSize,
    thumb_size: SurfaceSize,
    track_thickness: f64,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    value: SliderValue,
}
impl LayoutFixture {
    fn resolve(&self, value: &SliderValueIr) -> Result<SliderLayoutIr, SliderLayoutError> {
        resolve_slider_layout(SliderLayoutInput {
            allocation_size: self.allocation_size,
            thumb_size: self.thumb_size,
            track_thickness: self.track_thickness,
            insets: &self.insets,
            layout_direction: self.layout_direction,
            orientation: self.orientation,
            minimum_position: self.minimum_position,
            value,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fixture {
    layout: LayoutFixture,
    desired_origin: f64,
    enabled: bool,
    read_only: bool,
}
fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-position-cases.json"
    ))
    .unwrap()
}
fn origin(layout: &SliderLayoutIr) -> f64 {
    match layout.orientation() {
        SliderOrientation::Horizontal => layout.thumb_bounds().x,
        SliderOrientation::Vertical => layout.thumb_bounds().y,
    }
}
#[test]
fn public_vectors_verify_complete_position_results_and_rejection() {
    let source = corpus();
    assert_eq!(source["schemaVersion"], "0.1.0");
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let input: Fixture = serde_json::from_value(case["input"].clone()).unwrap();
        let current = resolve_slider_value(&input.layout.value).unwrap();
        let layout = input.layout.resolve(&current).expect(name);
        let result = resolve_slider_position(SliderPositionInput {
            layout: &layout,
            desired_origin: input.desired_origin,
            enabled: input.enabled,
            read_only: input.read_only,
        });
        if let Some(error) = case["error"].as_str() {
            let actual = match result.expect_err(name) {
                SliderPositionError::NumericRange => "numericRange",
                SliderPositionError::InvalidPosition => "invalidPosition",
                SliderPositionError::Adjustment(_) => "adjustment",
            };
            assert_eq!(actual, error, "{name}");
        } else {
            let result = result.expect(name);
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case["expected"],
                "{name}"
            );
            let next = input.layout.resolve(result.value()).expect(name);
            let stationary = resolve_slider_position(SliderPositionInput {
                layout: &next,
                desired_origin: origin(&next),
                enabled: true,
                read_only: false,
            })
            .unwrap();
            assert!(stationary.accepted());
            assert!(!stationary.changed());
            assert_eq!(stationary.value(), result.value());
        }
    }
}
#[test]
fn axis_mirroring_and_positive_scaling_preserve_numeric_intent() {
    let source = corpus();
    let mut verified_cases = 0;
    for case in source["cases"].as_array().unwrap().iter().filter(|case| {
        let name = case["name"].as_str().unwrap();
        name.starts_with("horizontal ") || name.starts_with("vertical ")
    }) {
        let mut input: Fixture = serde_json::from_value(case["input"].clone()).unwrap();
        let value = resolve_slider_value(&input.layout.value).unwrap();
        input.layout.layout_direction = match input.layout.layout_direction {
            LayoutDirection::Ltr => LayoutDirection::Rtl,
            LayoutDirection::Rtl => LayoutDirection::Ltr,
        };
        if input.layout.orientation == SliderOrientation::Horizontal {
            input.desired_origin = input.layout.allocation_size.width
                - input.layout.thumb_size.width
                - input.desired_origin;
        }
        for scale in [1.0, 2.0] {
            input.layout.allocation_size.width *= scale;
            input.layout.allocation_size.height *= scale;
            input.layout.thumb_size.width *= scale;
            input.layout.thumb_size.height *= scale;
            input.layout.track_thickness *= scale;
            input.layout.insets.start *= scale;
            input.layout.insets.end *= scale;
            input.layout.insets.top *= scale;
            input.layout.insets.bottom *= scale;
            input.desired_origin *= scale;
            let layout = input.layout.resolve(&value).unwrap();
            let result = resolve_slider_position(SliderPositionInput {
                layout: &layout,
                desired_origin: input.desired_origin,
                enabled: true,
                read_only: false,
            })
            .unwrap();
            assert_eq!(serde_json::to_value(result).unwrap(), case["expected"]);
        }
        verified_cases += 1;
    }
    assert_eq!(verified_cases, 56);
}
#[test]
fn current_layout_and_permission_are_required_at_each_delivery() {
    let source = corpus();
    let mut input: Fixture = serde_json::from_value(source["cases"][0]["input"].clone()).unwrap();
    for (minimum, maximum, value, target) in [
        (-10.0, 30.0, 0.0, 10.0),
        (0.0, 100.0, 25.0, 50.0),
        (-100.0, 0.0, -25.0, -50.0),
    ] {
        input.layout.value = SliderValue::try_new(minimum, maximum, value).unwrap();
        let current = resolve_slider_value(&input.layout.value).unwrap();
        let layout = input.layout.resolve(&current).unwrap();
        for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)] {
            let result = resolve_slider_position(SliderPositionInput {
                layout: &layout,
                desired_origin: 72.0,
                enabled,
                read_only,
            })
            .unwrap();
            assert_eq!(result.accepted(), enabled && !read_only);
            assert_eq!(result.changed(), enabled && !read_only);
            assert_eq!(
                result.value().value().value(),
                if enabled && !read_only { target } else { value }
            );
            assert_eq!(result.value().value().minimum(), minimum);
            assert_eq!(result.value().value().maximum(), maximum);
        }
    }
}
#[test]
fn stationary_reprojection_cannot_publish_numeric_mutation() {
    let source = corpus();
    let case = source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case["name"]
                .as_str()
                .unwrap()
                .starts_with("numeric mutation would leave thumb stationary")
        })
        .unwrap();
    let input: Fixture = serde_json::from_value(case["input"].clone()).unwrap();
    let current = resolve_slider_value(&input.layout.value).unwrap();
    let layout = input.layout.resolve(&current).unwrap();
    let adjacent =
        resolve_slider_value(&SliderValue::try_new(0.0, 1.0, 0.5080512701502198).unwrap()).unwrap();
    assert_ne!(adjacent.value(), current.value());
    assert_ne!(input.desired_origin, origin(&layout));
    assert_eq!(
        origin(&input.layout.resolve(&adjacent).unwrap()),
        origin(&layout)
    );
    assert!(matches!(
        resolve_slider_position(SliderPositionInput {
            layout: &layout,
            desired_origin: input.desired_origin,
            enabled: true,
            read_only: false,
        }),
        Err(SliderPositionError::NumericRange)
    ));
}
#[test]
fn nonfinite_positions_fail_even_when_unavailable() {
    let source = corpus();
    let input: Fixture = serde_json::from_value(source["cases"][0]["input"].clone()).unwrap();
    let current = resolve_slider_value(&input.layout.value).unwrap();
    let layout = input.layout.resolve(&current).unwrap();
    for desired_origin in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)] {
            let error = resolve_slider_position(SliderPositionInput {
                layout: &layout,
                desired_origin,
                enabled,
                read_only,
            })
            .unwrap_err();
            assert!(matches!(error, SliderPositionError::InvalidPosition));
            assert!(std::error::Error::source(&error).is_none());
            assert!(error.to_string().contains("finite"));
        }
    }
}
