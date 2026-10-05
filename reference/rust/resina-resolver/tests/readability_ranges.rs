use resina_color::{OpaqueSrgbRange, SrgbFallback, resolve_srgb_fallback};
use resina_model::{ColorRole, SurfaceIntent};
use resina_resolver::{
    HeadlessResolution, resolve_frost_surface_readability_over_ranges,
    resolve_frost_surface_readability_source, resolve_headless_source,
    resolve_surface_readability_over_ranges, resolve_surface_readability_source,
};
use serde_json::{Value, json};
use std::error::Error;

fn baseline() -> Value {
    let mut resolution: Value = serde_json::from_str(include_str!(
        "../../../../conformance/headless/valid-request.json"
    ))
    .unwrap();
    resolution["colorAssignments"]["roles"]["content.primary"] = json!("palette.opaqueAlt");
    resolution["opaqueColorAssignments"]["roles"]["outline.strong"] = json!("palette.opaqueAlt");
    let bindings: Value = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/binding-vectors.json"
    ))
    .unwrap();
    let mut surface = bindings[0]["document"].clone();
    surface["treatmentStack"]["treatments"] = json!(["none"]);
    json!({"schemaVersion":"0.1.0", "scenario":{"schemaVersion":"0.4.0", "resolution":resolution,"surface":surface},
           "foregroundRole":"content.primary", "postTreatmentBackdrop":{"colorSpace":"srgb","components":[1,1,1],"alpha":1},
           "adjacentColor":{"colorSpace":"srgb","components":[1,1,1],"alpha":1},
           "minimumContentContrast":3,"minimumEdgeContrast":3})
}

fn inputs(request: &Value) -> (SurfaceIntent, HeadlessResolution, SrgbFallback, ColorRole) {
    (
        serde_json::from_value(request["scenario"]["surface"].clone()).unwrap(),
        resolve_headless_source(&request["scenario"]["resolution"].to_string()).unwrap(),
        resolve_srgb_fallback(&request["postTreatmentBackdrop"]).unwrap(),
        serde_json::from_value(request["foregroundRole"].clone()).unwrap(),
    )
}

#[test]
fn public_frost_cases_preserve_complete_uniform_source_results_and_failures() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/frost-readability-cases.json"
    ))
    .unwrap();
    let mut checked = 0;
    for case in cases
        .iter()
        .filter(|case| case["requestSchemaValid"] == true)
    {
        let mut request = baseline();
        for change in case["changes"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let (intent, context, backdrop, foreground) = inputs(&request);
        let adjacent = resolve_srgb_fallback(&request["adjacentColor"]).unwrap();
        let ranges = [OpaqueSrgbRange::try_new(adjacent.clone(), adjacent).unwrap()];
        let content_minimum = request["minimumContentContrast"].as_f64().unwrap();
        let edge_minimum = request["minimumEdgeContrast"].as_f64().unwrap();
        let actual = resolve_frost_surface_readability_over_ranges(
            &intent,
            &context,
            foreground,
            &backdrop,
            &ranges,
            content_minimum,
            edge_minimum,
        )
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|e| e.to_string());
        let expected = resolve_frost_surface_readability_source(&request.to_string())
            .map(|value| serde_json::to_value(value).unwrap())
            .map_err(|e| e.to_string());
        assert_eq!(actual, expected, "{}", case["name"]);
        let actual = resolve_surface_readability_over_ranges(
            &intent,
            &context,
            foreground,
            Some(&backdrop),
            &ranges,
            content_minimum,
            edge_minimum,
        )
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|e| e.to_string());
        let expected = resolve_surface_readability_source(&request.to_string())
            .map(|value| serde_json::to_value(value).unwrap())
            .map_err(|e| e.to_string());
        assert_eq!(actual, expected, "{}", case["name"]);
        checked += 1;
    }
    assert!(checked >= 8);
}

#[test]
fn public_surface_cases_preserve_uniform_results_and_validation_order() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/surfaces/readability-cases.json"
    ))
    .unwrap();
    let mut checked = 0;
    for case in cases
        .iter()
        .filter(|case| case["requestSchemaValid"] == true)
    {
        let mut request = baseline();
        request["scenario"]["surface"]["states"]["states"] = json!(["rest"]);
        request["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]["content.primary"] =
            json!("palette.opaqueAlt");
        for change in case["changes"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let (intent, context, backdrop, foreground) = inputs(&request);
        let backdrop = if case["omitBackdrop"] == true {
            request
                .as_object_mut()
                .unwrap()
                .remove("postTreatmentBackdrop");
            None
        } else {
            Some(&backdrop)
        };
        let adjacent = resolve_srgb_fallback(&request["adjacentColor"]).unwrap();
        let ranges = [OpaqueSrgbRange::try_new(adjacent.clone(), adjacent).unwrap()];
        let actual = resolve_surface_readability_over_ranges(
            &intent,
            &context,
            foreground,
            backdrop,
            &ranges,
            request["minimumContentContrast"].as_f64().unwrap(),
            request["minimumEdgeContrast"].as_f64().unwrap(),
        )
        .map(|value| serde_json::to_value(value).unwrap())
        .map_err(|error| error.to_string());
        let expected = resolve_surface_readability_source(&request.to_string())
            .map(|value| serde_json::to_value(value).unwrap())
            .map_err(|error| error.to_string());
        assert_eq!(actual, expected, "{}", case["name"]);
        checked += 1;
    }
    assert_eq!(checked, 43);
}

#[test]
fn common_range_cases_replay_through_every_persistent_family_and_frost_fallback() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/common-contrast-cases.json"
    ))
    .unwrap();
    let mut checked = 0;
    for family in ["cast", "frost", "elastomer"] {
        for (reduced_transparency, translucent_surfaces) in
            [(false, true), (true, true), (false, false)]
        {
            for case in &cases {
                let mut request = baseline();
                let resolution = &mut request["scenario"]["resolution"];
                resolution["materialAssignments"]["surface"]["chrome"] = json!(family);
                resolution["tokens"]["palette"]["opaque"]["$value"] = case["preferred"].clone();
                resolution["tokens"]["palette"]["opaqueAlt"]["$value"] = case["fallback"].clone();
                resolution["opaqueColorAssignments"]["roles"]["outline"] = json!("palette.opaque");
                resolution["environment"]["accessibilityPreferences"]["reducedTransparency"] =
                    json!(reduced_transparency);
                resolution["environment"]["rendererCapabilities"]["translucentSurfaces"] =
                    json!(translucent_surfaces);
                let (intent, context, backdrop, foreground) = inputs(&request);
                let ranges: Vec<_> = case["backgrounds"]
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
                    .collect();
                let result = resolve_surface_readability_over_ranges(
                    &intent,
                    &context,
                    foreground,
                    Some(&backdrop),
                    &ranges,
                    1.0,
                    3.0,
                );
                if let Some(expected) = case.get("expected") {
                    let result = result.unwrap();
                    let selected = if expected["candidate"] == "preferred" {
                        &case["preferred"]
                    } else {
                        &case["fallback"]
                    };
                    assert_eq!(
                        result.edge().color(),
                        &resolve_srgb_fallback(selected).unwrap()
                    );
                    assert!(
                        (result.edge().contrast_ratio()
                            - expected["contrastRatio"].as_f64().unwrap())
                        .abs()
                            < 1e-12
                    );
                    assert_eq!(result.binding().states(), intent.states());
                    if family == "frost" {
                        let frost = resolve_frost_surface_readability_over_ranges(
                            &intent, &context, foreground, &backdrop, &ranges, 1.0, 3.0,
                        )
                        .unwrap();
                        assert_eq!(result.edge(), frost.edge());
                        assert_eq!(result.body(), frost.legibility().body());
                        assert_eq!(
                            result.composited_body(),
                            frost.legibility().composited_body()
                        );
                        assert_eq!(
                            result.frost_representation(),
                            Some(frost.legibility().representation())
                        );
                        if reduced_transparency || !translucent_surfaces {
                            assert_eq!(
                                serde_json::to_value(result.frost_representation()).unwrap(),
                                "opaqueDimensional"
                            );
                        }
                    } else {
                        assert!(result.frost_representation().is_none());
                    }
                } else {
                    let error = result.unwrap_err();
                    let diagnostic = if case["error"] == "empty" {
                        "must not be empty"
                    } else {
                        "contrast is insufficient"
                    };
                    assert!(error.to_string().contains(diagnostic));
                    assert!(error.source().is_some());
                    if family == "frost" {
                        let error = resolve_frost_surface_readability_over_ranges(
                            &intent, &context, foreground, &backdrop, &ranges, 1.0, 3.0,
                        )
                        .unwrap_err();
                        assert!(error.to_string().contains(diagnostic));
                        assert!(error.source().is_some());
                    }
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 72);
}

#[test]
fn content_failure_precedes_incompatible_nonempty_edge_regions() {
    let request = baseline();
    let (intent, context, backdrop, foreground) = inputs(&request);
    let black = resolve_srgb_fallback(&json!({"colorSpace":"srgb", "components":[0,0,0]})).unwrap();
    let white = resolve_srgb_fallback(&json!({"colorSpace":"srgb", "components":[1,1,1]})).unwrap();
    let ranges = [
        OpaqueSrgbRange::try_new(black.clone(), black).unwrap(),
        OpaqueSrgbRange::try_new(white.clone(), white).unwrap(),
    ];
    let error = resolve_surface_readability_over_ranges(
        &intent,
        &context,
        foreground,
        Some(&backdrop),
        &ranges,
        21.0,
        3.0,
    )
    .unwrap_err();
    assert!(error.to_string().contains("Frost contrast is insufficient"));
    assert!(error.source().unwrap().source().is_some());
    let readable = resolve_surface_readability_source(&request.to_string()).unwrap();
    assert!(readable.content_fallback_applied());
    let error = resolve_surface_readability_over_ranges(
        &intent,
        &context,
        foreground,
        Some(&backdrop),
        &ranges,
        3.0,
        3.0,
    )
    .unwrap_err();
    assert!(error.to_string().contains("edge contrast is insufficient"));
    assert!(error.source().unwrap().source().is_some());
}
