use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderValue, SurfaceSize};
use resina_resolver::{
    HitRegionInput, SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    SliderValueIr, resolve_hit_region, resolve_slider_layout, resolve_slider_value,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureInput {
    allocation_size: SurfaceSize,
    thumb_size: SurfaceSize,
    track_thickness: f64,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    value: SliderValue,
}
impl FixtureInput {
    fn resolve(
        &self,
        value: &SliderValueIr,
    ) -> Result<SliderLayoutIr, resina_resolver::SliderLayoutError> {
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
#[test]
fn public_vectors_verify_complete_geometry_and_numeric_rejection() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    assert_eq!(source["schemaVersion"], "0.1.0");
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let input: FixtureInput = serde_json::from_value(case["input"].clone()).unwrap();
        let value = resolve_slider_value(&input.value).unwrap();
        let result = input.resolve(&value);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(format!("{:?}", result.unwrap_err()), error, "{name}");
        } else {
            let result = result.unwrap();
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case["expected"],
                "{name}"
            );
            assert_eq!(result.orientation(), input.orientation);
            assert_eq!(result.layout_direction(), input.layout_direction);
            assert_eq!(result.minimum_position(), input.minimum_position);
            assert_eq!(result.value(), &value);
            if value.progress() == 0.0 {
                assert_eq!(result.thumb_bounds(), result.minimum_thumb_bounds());
            }
            if value.progress() == 1.0 {
                assert_eq!(result.thumb_bounds(), result.maximum_thumb_bounds());
            }
        }
    }
}
#[test]
fn mirrors_scaling_and_target_reservation_preserve_complete_anatomy() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    let environment_source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let environment = serde_json::from_value(
        environment_source["surface"]["body"]["theme"]["environment"].clone(),
    )
    .unwrap();
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case.get("expected").is_some())
    {
        let mut input: FixtureInput = serde_json::from_value(case["input"].clone()).unwrap();
        let value = resolve_slider_value(&input.value).unwrap();
        let original = input.resolve(&value).unwrap();
        input.layout_direction = match input.layout_direction {
            LayoutDirection::Ltr => LayoutDirection::Rtl,
            LayoutDirection::Rtl => LayoutDirection::Ltr,
        };
        let mirrored = input.resolve(&value).unwrap();
        for (a, b) in [
            (original.track_bounds(), mirrored.track_bounds()),
            (
                original.minimum_thumb_bounds(),
                mirrored.minimum_thumb_bounds(),
            ),
            (
                original.maximum_thumb_bounds(),
                mirrored.maximum_thumb_bounds(),
            ),
            (original.thumb_bounds(), mirrored.thumb_bounds()),
        ] {
            assert_eq!(b.x, input.allocation_size.width - a.x - a.width);
            assert_eq!((b.y, b.width, b.height), (a.y, a.width, a.height));
        }
        input.allocation_size.width *= 2.0;
        input.allocation_size.height *= 2.0;
        input.thumb_size.width *= 2.0;
        input.thumb_size.height *= 2.0;
        input.track_thickness *= 2.0;
        input.insets.start *= 2.0;
        input.insets.end *= 2.0;
        input.insets.top *= 2.0;
        input.insets.bottom *= 2.0;
        let scaled = input.resolve(&value).unwrap();
        for (a, b) in [
            (mirrored.track_bounds(), scaled.track_bounds()),
            (
                mirrored.minimum_thumb_bounds(),
                scaled.minimum_thumb_bounds(),
            ),
            (
                mirrored.maximum_thumb_bounds(),
                scaled.maximum_thumb_bounds(),
            ),
            (mirrored.thumb_bounds(), scaled.thumb_bounds()),
        ] {
            assert_eq!(
                (b.x, b.y, b.width, b.height),
                (a.x * 2.0, a.y * 2.0, a.width * 2.0, a.height * 2.0)
            );
        }
        let target = resolve_hit_region(HitRegionInput {
            environment: &environment,
            visual_bounds: scaled.allocation_bounds(),
            available_bounds: resina_model::PhysicalBounds {
                x: -64.0,
                y: -64.0,
                width: input.allocation_size.width + 128.0,
                height: input.allocation_size.height + 128.0,
            },
            component_minimum: SurfaceSize {
                width: 24.0,
                height: 24.0,
            },
            occupied_regions: &[],
        })
        .unwrap();
        for bounds in [
            scaled.track_bounds(),
            scaled.minimum_thumb_bounds(),
            scaled.maximum_thumb_bounds(),
            scaled.thumb_bounds(),
        ] {
            assert!(target.contains_bounds(bounds).unwrap());
        }
        for progress_value in [-10.0, 0.0, 10.0, 20.0, 30.0] {
            let value =
                resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, progress_value).unwrap())
                    .unwrap();
            let next = input.resolve(&value).unwrap();
            assert_eq!(next.allocation_bounds(), scaled.allocation_bounds());
            assert_eq!(next.track_bounds(), scaled.track_bounds());
            assert_eq!(next.minimum_thumb_bounds(), scaled.minimum_thumb_bounds());
            assert_eq!(next.maximum_thumb_bounds(), scaled.maximum_thumb_bounds());
            assert!(target.contains_bounds(next.thumb_bounds()).unwrap());
        }
    }
}
#[test]
fn nonfinite_authored_geometry_has_no_resolved_default() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-layout-cases.json"
    ))
    .unwrap();
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..9 {
            let mut input: FixtureInput =
                serde_json::from_value(source["cases"][0]["input"].clone()).unwrap();
            let value = resolve_slider_value(&input.value).unwrap();
            match field {
                0 => input.allocation_size.width = number,
                1 => input.allocation_size.height = number,
                2 => input.thumb_size.width = number,
                3 => input.thumb_size.height = number,
                4 => input.track_thickness = number,
                5 => input.insets.start = number,
                6 => input.insets.end = number,
                7 => input.insets.top = number,
                8 => input.insets.bottom = number,
                _ => unreachable!(),
            }
            let error = input.resolve(&value).unwrap_err();
            assert_eq!(
                error,
                if field < 5 {
                    resina_resolver::SliderLayoutError::InvalidSize
                } else {
                    resina_resolver::SliderLayoutError::InvalidInsets
                }
            );
        }
    }
}
