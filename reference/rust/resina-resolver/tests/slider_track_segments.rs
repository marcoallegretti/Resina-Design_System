use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalBounds, SliderValue, SurfaceSize};
use resina_resolver::{
    SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    SliderTrackSegmentsError, resolve_slider_layout, resolve_slider_track_segments,
    resolve_slider_value,
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
        .unwrap()
    }
}

// Scaled to integers, the vector coordinates compare and add without rounding.
fn exact(value: f64) -> i128 {
    let scaled = value * 2f64.powi(70);
    assert!(
        scaled.fract() == 0.0 && scaled.abs() < 2f64.powi(120),
        "{value}"
    );
    scaled as i128
}

fn main_span(bounds: PhysicalBounds, horizontal: bool) -> (i128, i128) {
    let (origin, extent) = if horizontal {
        (bounds.x, bounds.width)
    } else {
        (bounds.y, bounds.height)
    };
    (exact(origin), exact(origin) + exact(extent))
}

#[test]
fn public_vectors_split_the_track_around_the_thumb_clearance() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-track-segments-cases.json"
    ))
    .unwrap();
    assert_eq!(source["schemaVersion"], "0.1.0");
    let mut checked = 0;
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let layout: LayoutFixture = serde_json::from_value(case["layout"].clone()).unwrap();
        let layout = layout.resolve();
        let clearance = case["clearance"].as_f64().unwrap();
        let result = resolve_slider_track_segments(&layout, clearance);
        if let Some(error) = case["error"].as_str() {
            assert_eq!(format!("{:?}", result.unwrap_err()), error, "{name}");
            continue;
        }
        let result = result.unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            case["expected"],
            "{name}"
        );
        let horizontal = layout.orientation() == SliderOrientation::Horizontal;
        let (track_start, track_end) = main_span(layout.track_bounds(), horizontal);
        let (thumb_start, thumb_end) = main_span(layout.thumb_bounds(), horizontal);
        let segments: Vec<_> = [result.active(), result.inactive()]
            .into_iter()
            .flatten()
            .collect();
        for segment in &segments {
            let (start, end) = main_span(*segment, horizontal);
            assert!(start < end, "{name}");
            assert!(start >= track_start && end <= track_end, "{name}");
            let clearance = exact(clearance);
            assert!(
                end <= thumb_start - clearance || start >= thumb_end + clearance,
                "{name}"
            );
        }
        if let [first, second] = segments[..] {
            let (a, b) = (main_span(first, horizontal), main_span(second, horizontal));
            assert!(a.1 <= b.0 || b.1 <= a.0, "{name}");
        }
        checked += 1;
    }
    assert_eq!(checked, 218);
}

#[test]
fn nonfinite_clearance_is_rejected() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/slider-track-segments-cases.json"
    ))
    .unwrap();
    let layout: LayoutFixture =
        serde_json::from_value(source["cases"][0]["layout"].clone()).unwrap();
    let layout = layout.resolve();
    for clearance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            resolve_slider_track_segments(&layout, clearance).unwrap_err(),
            SliderTrackSegmentsError::InvalidClearance
        );
    }
}
