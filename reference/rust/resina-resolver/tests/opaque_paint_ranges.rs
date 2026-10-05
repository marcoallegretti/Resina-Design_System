use resina_color::{OpaqueSrgbRange, composite_srgb_over_opaque, resolve_srgb_fallback};
use resina_model::PhysicalVector;
use resina_resolver::{OpaqueSurfaceIr, resolve_opaque_surface_source};
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-paint-range-cases.json"
    ))
    .unwrap()
}

fn surface(case: &Value) -> OpaqueSurfaceIr {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let family = case["family"].as_str().unwrap();
    let mut theme: Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["materialAssignments"]["surface"]["transient"] = case["family"].clone();
    for token in ["base", "opaque", "opaqueAlt"] {
        theme["tokens"]["palette"][token]["$value"]["components"] = case["body"].clone();
    }
    theme["tokens"]["radius"]["0"]["$value"]["value"] = case["radius"].clone();
    request["theme"]["themeSource"] = json!(theme.to_string());
    request["surface"]["materialRole"] = json!("surface.transient");
    request["surface"]["form"]["elevation"] = case["elevation"].clone();
    request["appearance"]["bands"][family]["highlightWidth"] = case["highlightWidth"].clone();
    request["appearance"]["pigmentProfiles"]["profiles"][family] = json!({
        "sideShade":case["sideShade"], "highlightLift":case["highlightLift"]
    });
    request["minimumContentContrast"] = json!(1);
    resolve_opaque_surface_source(&request.to_string()).unwrap()
}

fn covered(ranges: &[OpaqueSrgbRange], channels: [f64; 3]) -> bool {
    ranges.iter().any(|range| {
        channels
            .into_iter()
            .zip(range.lower().components())
            .zip(range.upper().components())
            .all(|((value, lower), upper)| lower <= value && value <= upper)
    })
}

#[test]
fn public_cases_keep_flat_regions_separate_and_suppress_absent_modulation() {
    for case in cases() {
        let ir = surface(&case);
        let ranges = ir.paint_color_ranges();
        assert_eq!(
            ranges.len(),
            case["expectedCount"].as_u64().unwrap() as usize,
            "{}",
            case["name"]
        );
        assert_eq!(ranges[0].lower(), ir.pigment().body());
        assert_eq!(ranges[0].lower(), ranges[0].upper());
        assert_eq!(ranges[1].lower(), ir.edge().color());
        assert_eq!(ranges[1].lower(), ranges[1].upper());
        if case["elevation"] == "raised" {
            assert_eq!(ranges[2].lower(), ir.pigment().side());
            assert_eq!(ranges[2].lower(), ranges[2].upper());
        }
        if case["highlightWidth"].as_f64().unwrap() > 0.0
            && case["highlightLift"].as_f64().unwrap() > 0.0
        {
            let highlight = ranges.last().unwrap();
            for (actual, expected) in highlight
                .upper()
                .components()
                .into_iter()
                .zip(case["expectedHighlight"].as_array().unwrap())
            {
                assert!((actual - expected.as_f64().unwrap()).abs() < 2e-15);
            }
        }
        assert_eq!(ranges, ir.paint_color_ranges());
    }
}

#[test]
fn derived_ranges_cover_actual_square_and_rounded_paint_samples() {
    for case in cases() {
        let ir = surface(&case);
        let ranges = ir.paint_color_ranges();
        let bounds = ir.geometry().silhouette().bounds().unwrap();
        let mut painted = 0;
        for y in 0..48 {
            for x in 0..64 {
                let point = PhysicalVector {
                    x: bounds.x + bounds.width * (x as f64 + 0.37) / 64.0,
                    y: bounds.y + bounds.height * (y as f64 + 0.29) / 48.0,
                };
                if let Some(color) = ir.sample_paint(point).unwrap() {
                    assert!(
                        covered(&ranges, color.components()),
                        "{}: {point:?}",
                        case["name"]
                    );
                    painted += 1;
                }
            }
        }
        assert!(painted > 1000);
    }
}

#[test]
fn highlight_ranges_cover_weight_sweeps_and_adjacent_represented_weights() {
    let white = resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[1,1,1]})).unwrap();
    for case in cases().into_iter().filter(|case| {
        case["highlightWidth"].as_f64().unwrap() > 0.0
            && case["highlightLift"].as_f64().unwrap() > 0.0
    }) {
        let ir = surface(&case);
        let ranges = ir.paint_color_ranges();
        let highlight = [ranges.last().unwrap().clone()];
        let lift = case["highlightLift"].as_f64().unwrap();
        for index in 0..=4096 {
            let weight = index as f64 / 4096.0;
            for weight in [
                weight.next_down().max(0.0),
                weight,
                weight.next_up().min(1.0),
            ] {
                let color = composite_srgb_over_opaque(
                    &white.with_alpha(lift * weight).unwrap(),
                    ir.pigment().body(),
                )
                .unwrap();
                assert!(
                    covered(&highlight, color.components()),
                    "{}: {weight}",
                    case["name"]
                );
                let alternate = ir
                    .pigment()
                    .body()
                    .components()
                    .map(|body| (1.0 - body).mul_add(lift * weight, body));
                assert!(
                    covered(&highlight, alternate),
                    "{}: alternate {weight}",
                    case["name"]
                );
            }
        }
    }
}

#[test]
fn numerical_bounds_cover_subnormal_coefficients_and_gamut_neighbors() {
    let white = resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[1,1,1]})).unwrap();
    for body in [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        0.04045,
        0.5_f64.next_up(),
        1.0_f64.next_down(),
    ] {
        for lift in [
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            f64::EPSILON,
            0.5,
            1.0_f64.next_down(),
        ] {
            let mut case = cases()[0].clone();
            case["body"] = json!([body, 1.0 - body, 0.5]);
            case["highlightLift"] = json!(lift);
            let ir = surface(&case);
            let ranges = ir.paint_color_ranges();
            let highlight = [ranges.last().unwrap().clone()];
            for weight in [
                0.0,
                f64::from_bits(1),
                f64::MIN_POSITIVE,
                f64::EPSILON,
                0.5_f64.next_down(),
                0.5,
                0.5_f64.next_up(),
                1.0_f64.next_down(),
                1.0,
            ] {
                let color = composite_srgb_over_opaque(
                    &white.with_alpha(lift * weight).unwrap(),
                    ir.pigment().body(),
                )
                .unwrap();
                assert!(
                    covered(&highlight, color.components()),
                    "body {body}, lift {lift}, weight {weight}"
                );
            }
        }
    }
}
