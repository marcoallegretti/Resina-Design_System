use resina_color::{OpaqueSrgbRange, SrgbFallback, resolve_srgb_fallback};
use resina_model::SurfaceIntent;
use resina_resolver::{
    EdgeContrastError, FocusIndicatorError, resolve_edge_contrast,
    resolve_edge_contrast_over_ranges, resolve_focus_indicator,
    resolve_focus_indicator_over_ranges, resolve_headless_source,
};
use serde_json::{Value, json};

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/color/common-contrast-cases.json"
    ))
    .unwrap()
}

fn ranges(case: &Value) -> Vec<OpaqueSrgbRange> {
    case["backgrounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|range| {
            OpaqueSrgbRange::try_new(
                resolve_srgb_fallback(&range["lower"]).unwrap(),
                resolve_srgb_fallback(&range["upper"]).unwrap(),
            )
            .unwrap()
        })
        .collect()
}

fn focus_inputs(
    preferred: &Value,
    fallback: &Value,
) -> (SurfaceIntent, resina_resolver::HeadlessResolution) {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    request["tokens"]["palette"]["opaque"]["$value"] = preferred.clone();
    request["tokens"]["palette"]["opaqueAlt"]["$value"] = fallback.clone();
    request["opaqueColorAssignments"]["roles"]["focus"] = json!("palette.opaque");
    request["opaqueColorAssignments"]["roles"]["outline.strong"] = json!("palette.opaqueAlt");
    let context = resolve_headless_source(&request.to_string()).unwrap();
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut intent = bindings[0]["document"].clone();
    intent["states"]["states"] = json!(["rest", "focused", "selected"]);
    (serde_json::from_value(intent).unwrap(), context)
}

#[test]
fn public_cases_require_one_candidate_across_every_edge_and_focus_region() {
    let cases = cases();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let preferred = resolve_srgb_fallback(&case["preferred"]).unwrap();
        let fallback = resolve_srgb_fallback(&case["fallback"]).unwrap();
        let ranges = ranges(&case);
        let edge = resolve_edge_contrast_over_ranges(&preferred, &fallback, &ranges, 3.0);
        let (intent, context) = focus_inputs(&case["preferred"], &case["fallback"]);
        let focus = resolve_focus_indicator_over_ranges(&intent, &context, &ranges);
        if let Some(expected) = case.get("expected") {
            let edge = edge.unwrap();
            let focus = focus.unwrap();
            let mut reordered = ranges.clone();
            reordered.reverse();
            reordered.extend(ranges.clone());
            assert_eq!(
                resolve_edge_contrast_over_ranges(&preferred, &fallback, &reordered, 3.0).unwrap(),
                edge
            );
            assert_eq!(
                resolve_focus_indicator_over_ranges(&intent, &context, &reordered).unwrap(),
                focus
            );
            let fallback_selected = expected["candidate"] == "fallback";
            let selected = if fallback_selected {
                &fallback
            } else {
                &preferred
            };
            for (color, ratio, applied) in [
                (edge.color(), edge.contrast_ratio(), edge.fallback_applied()),
                (
                    focus.color(),
                    focus.contrast_ratio(),
                    focus.fallback_applied(),
                ),
            ] {
                assert_eq!(color, selected, "{}", case["name"]);
                assert_eq!(applied, fallback_selected);
                assert!((ratio - expected["contrastRatio"].as_f64().unwrap()).abs() < 1e-12);
            }
            let edge_json = serde_json::to_value(edge).unwrap();
            let focus_json = serde_json::to_value(focus).unwrap();
            assert_eq!(
                edge_json["colorRole"],
                if fallback_selected {
                    "outline.strong"
                } else {
                    "outline"
                }
            );
            assert_eq!(
                focus_json["colorRole"],
                if fallback_selected {
                    "outline.strong"
                } else {
                    "focus"
                }
            );
            assert_eq!(
                focus_json["binding"]["states"],
                serde_json::to_value(intent.states()).unwrap()
            );
            assert_eq!(focus_json["strokeWidth"], 2.0);
            assert_eq!(focus_json["gap"], 2.0);
        } else if case["error"] == "empty" {
            assert!(matches!(edge, Err(EdgeContrastError::EmptyAdjacentRanges)));
            assert!(matches!(
                focus,
                Err(FocusIndicatorError::EmptySurroundingRanges)
            ));
        } else {
            let Err(EdgeContrastError::InsufficientContrast { outline, strong }) = edge else {
                panic!("{}", case["name"])
            };
            let Err(FocusIndicatorError::InsufficientContrast {
                focus,
                outline_strong,
            }) = focus
            else {
                panic!("{}", case["name"])
            };
            assert!(
                (outline - case["candidateBounds"]["preferred"].as_f64().unwrap()).abs() < 1e-12
            );
            assert!((strong - case["candidateBounds"]["fallback"].as_f64().unwrap()).abs() < 1e-12);
            assert_eq!(focus, outline);
            assert_eq!(outline_strong, strong);
        }
    }
}

#[test]
fn every_uniform_region_preserves_complete_single_color_results() {
    for case in cases() {
        let preferred = resolve_srgb_fallback(&case["preferred"]).unwrap();
        let fallback = resolve_srgb_fallback(&case["fallback"]).unwrap();
        let (intent, context) = focus_inputs(&case["preferred"], &case["fallback"]);
        for range in ranges(&case) {
            for background in [range.lower(), range.upper()] {
                let uniform =
                    OpaqueSrgbRange::try_new(background.clone(), background.clone()).unwrap();
                assert_eq!(
                    resolve_edge_contrast_over_ranges(
                        &preferred,
                        &fallback,
                        std::slice::from_ref(&uniform),
                        3.0,
                    )
                    .map_err(|error| error.to_string()),
                    resolve_edge_contrast(&preferred, &fallback, background, 3.0)
                        .map_err(|error| error.to_string())
                );
                assert_eq!(
                    resolve_focus_indicator_over_ranges(&intent, &context, &[uniform])
                        .map_err(|error| error.to_string()),
                    resolve_focus_indicator(&intent, &context, background)
                        .map_err(|error| error.to_string())
                );
            }
        }
    }
}

fn gray(value: f64) -> SrgbFallback {
    resolve_srgb_fallback(
        &json!({"colorSpace":"srgb", "components":[value,value,value], "alpha":1}),
    )
    .unwrap()
}

#[test]
fn invalid_threshold_and_candidates_fail_before_empty_regions() {
    let black = gray(0.0);
    let white = gray(1.0);
    let translucent = black.with_alpha(0.5).unwrap();
    assert!(matches!(
        resolve_edge_contrast_over_ranges(&translucent, &white, &[], f64::NAN),
        Err(EdgeContrastError::InvalidMinimumContrast)
    ));
    assert!(matches!(
        resolve_edge_contrast_over_ranges(&translucent, &white, &[], 3.0),
        Err(EdgeContrastError::TranslucentOutline)
    ));
    assert!(matches!(
        resolve_edge_contrast_over_ranges(&black, &translucent, &[], 3.0),
        Err(EdgeContrastError::TranslucentStrongOutline)
    ));
    let white_region = OpaqueSrgbRange::try_new(white.clone(), white).unwrap();
    assert_eq!(
        resolve_edge_contrast_over_ranges(&black, &black, &[white_region], 21.0)
            .unwrap()
            .contrast_ratio(),
        21.0
    );
}

#[test]
fn absent_focus_fails_before_empty_surroundings() {
    let (intent, context) = focus_inputs(
        &serde_json::to_value(gray(0.0)).unwrap(),
        &serde_json::to_value(gray(1.0)).unwrap(),
    );
    let mut source = serde_json::to_value(intent).unwrap();
    source["states"]["states"] = json!(["rest"]);
    let unfocused: SurfaceIntent = serde_json::from_value(source).unwrap();
    assert!(matches!(
        resolve_focus_indicator_over_ranges(&unfocused, &context, &[]),
        Err(FocusIndicatorError::NotFocused)
    ));
}
