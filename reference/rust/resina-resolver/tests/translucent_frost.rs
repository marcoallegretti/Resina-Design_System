use resina_resolver::{
    FrostAppearance, FrostFallbackReason, resolve_opaque_surface_source,
    resolve_surface_paint_source, resolve_translucent_frost_source,
};
use serde_json::{Value, json};

fn baseline() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/translucent-frost-request.json"
    ))
    .unwrap()
}

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/translucent-frost-cases.json"
    ))
    .unwrap()
}

fn set(document: &mut Value, pointer: &str, value: Value) {
    *document.pointer_mut(pointer).unwrap() = value;
}

fn remove(document: &mut Value, pointer: &str) {
    let (parent, member) = pointer.rsplit_once('/').unwrap();
    document
        .pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(member);
}

fn case_request(case: &Value) -> Value {
    let mut request = baseline();
    for change in case["requestChanges"].as_array().unwrap() {
        set(
            &mut request,
            change["path"].as_str().unwrap(),
            change["value"].clone(),
        );
    }
    for pointer in case["omitMembers"].as_array().into_iter().flatten() {
        remove(&mut request, pointer.as_str().unwrap());
    }
    request
}

fn resolve(request: &Value) -> Result<Value, String> {
    resolve_translucent_frost_source(&request.to_string())
        .map(|ir| serde_json::to_value(ir).unwrap())
        .map_err(|error| error.to_string())
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

#[test]
fn public_cases_select_the_stated_appearance_representation_and_reason() {
    for case in cases() {
        let name = case["name"].as_str().unwrap();
        let result = resolve(&case_request(&case));
        if let Some(message) = case["errorContains"].as_str() {
            let error = result.unwrap_err();
            if case["requestSchemaValid"] == true {
                assert!(error.contains(message), "{name}: {error}");
            }
            continue;
        }
        let result = result.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(result["appearance"], case["expectedAppearance"], "{name}");
        assert_eq!(
            result["frostRepresentation"], case["expectedRepresentation"],
            "{name}"
        );
        assert_eq!(
            result.get("fallbackReason"),
            case.get("expectedFallbackReason"),
            "{name}"
        );
        let focus = result.get("focus").or(result["paint"].get("focus"));
        assert_eq!(focus.is_some(), case["expectedFocus"] == true, "{name}");
    }
}

#[test]
fn typed_result_exposes_the_same_decision() {
    let retained = resolve_translucent_frost_source(&baseline().to_string()).unwrap();
    assert!(matches!(
        retained.appearance(),
        FrostAppearance::Translucent { .. }
    ));
    let mut request = baseline();
    set(&mut request, "/backdrop/lower/components", json!([0, 0, 0]));
    let fallen = resolve_translucent_frost_source(&request.to_string()).unwrap();
    assert!(matches!(
        fallen.appearance(),
        FrostAppearance::Opaque {
            fallback_reason: FrostFallbackReason::Legibility,
            ..
        }
    ));
}

#[test]
fn retained_content_contrast_is_a_lower_bound_over_the_whole_interval() {
    let mut request = baseline();
    let (lower, upper) = (0.6, 0.95);
    set(
        &mut request,
        "/backdrop/lower/components",
        json!([lower, lower, lower]),
    );
    set(
        &mut request,
        "/backdrop/upper/components",
        json!([upper, upper, upper]),
    );
    let result = resolve(&request).unwrap();
    assert_eq!(result["appearance"], "translucent");
    let body = &result["body"];
    let bound = number(&body["contentContrastRatio"]);
    let pigment = &body["pigment"]["body"];
    let alpha = number(&pigment["alpha"]);
    let foreground: Vec<f64> = body["foreground"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(number)
        .collect();
    let luminance = |c: [f64; 3]| {
        let [r, g, b] = c.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let text = luminance([foreground[0], foreground[1], foreground[2]]);
    let tint: Vec<f64> = pigment["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(number)
        .collect();
    let steps = 200;
    let mut least = f64::INFINITY;
    for step in 0..=steps {
        let under = lower + (upper - lower) * step as f64 / steps as f64;
        let flat = [0, 1, 2].map(|i| tint[i] * alpha + under * (1.0 - alpha));
        let lit = luminance(flat);
        least = least.min((lit.max(text) + 0.05) / (lit.min(text) + 0.05));
    }
    assert!(bound <= least + 1e-12, "{bound} exceeds sampled {least}");
    assert!(bound >= 4.5);
}

#[test]
fn a_busy_interval_is_certified_by_its_worst_end_not_its_midpoint() {
    let mut request = baseline();
    set(
        &mut request,
        "/backdrop/lower/components",
        json!([0.0, 0.0, 0.0]),
    );
    set(
        &mut request,
        "/backdrop/upper/components",
        json!([1.0, 1.0, 1.0]),
    );
    let wide = resolve(&request).unwrap();
    assert_eq!(wide["fallbackReason"], "legibility");
    set(
        &mut request,
        "/backdrop/lower/components",
        json!([0.5, 0.5, 0.5]),
    );
    set(
        &mut request,
        "/backdrop/upper/components",
        json!([0.5, 0.5, 0.5]),
    );
    let midpoint = resolve(&request).unwrap();
    assert_eq!(midpoint["appearance"], "translucent");
}

#[test]
fn selection_precedes_legibility_and_reports_its_reason() {
    let mut request = baseline();
    set(
        &mut request,
        "/theme/environment/accessibilityPreferences/reducedTransparency",
        json!(true),
    );
    set(
        &mut request,
        "/backdrop/lower/components",
        json!([0.0, 0.0, 0.0]),
    );
    let result = resolve(&request).unwrap();
    assert_eq!(result["fallbackReason"], "selection");
    assert_eq!(result["paint"]["body"]["contentFallbackApplied"], false);
}

#[test]
fn opaque_selection_is_the_existing_opaque_surface_unchanged() {
    let mut request = baseline();
    set(
        &mut request,
        "/theme/environment/accessibilityPreferences/highContrast",
        json!(true),
    );
    let result = resolve(&request).unwrap();
    let uniform = |color: &Value| json!({"lower": color, "upper": color});
    let adjacent = request["adjacentRanges"][0]["lower"].clone();
    set(&mut request, "/adjacentRanges", json!([uniform(&adjacent)]));
    let result_uniform = resolve(&request).unwrap();
    assert_eq!(result_uniform["paint"]["body"], result["paint"]["body"]);

    let mut opaque = json!({
        "schemaVersion": "0.1.0",
        "postTreatmentBackdrop": request["backdrop"]["lower"],
        "adjacentColor": adjacent,
    });
    for member in [
        "theme",
        "surface",
        "size",
        "appearance",
        "foregroundRole",
        "minimumContentContrast",
        "minimumEdgeContrast",
    ] {
        opaque[member] = request[member].clone();
    }
    let reference =
        serde_json::to_value(resolve_opaque_surface_source(&opaque.to_string()).unwrap()).unwrap();
    assert_eq!(result_uniform["paint"]["body"], reference);
}

#[test]
fn geometry_and_light_are_the_opaque_surface_at_every_size_and_scale() {
    for (width, height) in [(20.0, 12.0), (64.0, 32.0), (320.0, 200.0)] {
        for scale in [1.0, 1.5, 2.0] {
            let mut request = baseline();
            set(
                &mut request,
                "/size",
                json!({"width": width, "height": height}),
            );
            set(&mut request, "/theme/environment/scale", json!(scale));
            let translucent = resolve(&request).unwrap();
            assert_eq!(translucent["appearance"], "translucent");
            let mut opaque = request.clone();
            set(
                &mut opaque,
                "/theme/environment/accessibilityPreferences/reducedTransparency",
                json!(true),
            );
            let selected = resolve(&opaque).unwrap();
            let reference = &selected["paint"]["body"];
            let body = &translucent["body"];
            for member in [
                "geometry",
                "lighting",
                "edgeWidth",
                "highlightWidth",
                "form",
            ] {
                assert_eq!(body[member], reference[member], "{width}x{height}@{scale}");
            }
            let hierarchy = |value: &Value| {
                (
                    value["pigment"]["profile"].clone(),
                    value["pigment"]["materialFamily"].clone(),
                )
            };
            assert_eq!(hierarchy(body), hierarchy(reference));
        }
    }
}

#[test]
fn translucent_pigments_keep_the_opaque_law_and_the_body_alpha() {
    let result = resolve(&baseline()).unwrap();
    let pigment = &result["body"]["pigment"];
    let profile = &pigment["profile"];
    let shade = number(&profile["sideShade"]);
    let lift = number(&profile["highlightLift"]);
    let alpha = number(&pigment["body"]["alpha"]);
    assert!(alpha > 0.0 && alpha < 1.0);
    for channel in 0..3 {
        let base = number(&pigment["body"]["components"][channel]);
        assert!(
            (number(&pigment["side"]["components"][channel]) - base * (1.0 - shade)).abs() < 1e-12
        );
        assert!(
            (number(&pigment["highlight"]["components"][channel]) - (lift + base * (1.0 - lift)))
                .abs()
                < 1e-12
        );
    }
    assert_eq!(pigment["side"]["alpha"], pigment["body"]["alpha"]);
    assert_eq!(pigment["highlight"]["alpha"], pigment["body"]["alpha"]);
}

#[test]
fn diffusion_is_published_only_where_a_backdrop_is_diffused() {
    let mut request = baseline();
    for (blur, shaped, effect, present) in [
        (true, true, true, true),
        (true, false, true, true),
        (false, false, true, false),
        (true, true, false, false),
    ] {
        let capabilities = "/theme/environment/rendererCapabilities/";
        set(
            &mut request,
            &format!("{capabilities}backdropBlur"),
            json!(blur),
        );
        set(
            &mut request,
            &format!("{capabilities}shapedBackdrop"),
            json!(shaped),
        );
        set(
            &mut request,
            &format!("{capabilities}backdropEffect"),
            json!(effect),
        );
        let result = resolve(&request).unwrap();
        assert_eq!(result["body"].get("diffusion").is_some(), present);
        if present {
            assert_eq!(result["body"]["diffusion"]["radius"], 12.0);
        }
    }
}

#[test]
fn the_edge_is_selected_against_every_adjacent_region() {
    let mut request = baseline();
    let region = |level: f64| {
        json!({
            "lower": {"colorSpace": "srgb", "components": [level, level, level], "alpha": 1},
            "upper": {"colorSpace": "srgb", "components": [level, level, level], "alpha": 1},
        })
    };
    set(
        &mut request,
        "/adjacentRanges",
        json!([region(1.0), region(0.5)]),
    );
    let both = resolve(&request).unwrap();
    set(
        &mut request,
        "/adjacentRanges",
        json!([region(0.5), region(1.0)]),
    );
    let reordered = resolve(&request).unwrap();
    assert_eq!(both, reordered);
    assert_eq!(both["body"]["edge"]["colorRole"], "outline.strong");
    set(&mut request, "/adjacentRanges", json!([region(1.0)]));
    let bright = resolve(&request).unwrap();
    assert_eq!(bright["body"]["edge"]["colorRole"], "outline");
}

#[test]
fn request_members_are_strict() {
    let source = |request: &Value| resolve_translucent_frost_source(&request.to_string());
    for member in baseline().as_object().unwrap().keys() {
        if member == "surroundingRanges" {
            continue;
        }
        let mut request = baseline();
        remove(&mut request, &format!("/{member}"));
        assert!(source(&request).is_err(), "{member}");
    }
    for (pointer, value) in [
        ("/foregroundRole", json!({"content.primary": null})),
        ("/diffusion", json!(null)),
        ("/diffusion", json!([12])),
        ("/backdrop", json!(null)),
        ("/backdrop", json!([])),
        ("/surroundingRanges", json!(null)),
        ("/surroundingRanges", json!([])),
        ("/adjacentRanges", json!([])),
        ("/adjacentRanges", json!({})),
    ] {
        let mut request = baseline();
        set(&mut request, pointer, value);
        assert!(source(&request).is_err(), "{pointer}");
    }
    let mut unknown = baseline();
    unknown["unexpected"] = json!(true);
    assert!(source(&unknown).is_err());
    assert!(resolve_translucent_frost_source("[]").is_err());
    let duplicate = baseline().to_string().replacen(
        "\"diffusion\":{\"radius\":12}",
        "\"diffusion\":{\"radius\":12,\"radius\":9}",
        1,
    );
    assert!(resolve_translucent_frost_source(&duplicate).is_err());
}

fn color_at(level: f64) -> Value {
    json!({"colorSpace": "srgb", "components": [level, level, level], "alpha": 1})
}

fn point(level: f64) -> Value {
    json!({"lower": color_at(level), "upper": color_at(level)})
}

fn surface_paint_equivalent(request: &Value) -> Value {
    let mut body = json!({
        "schemaVersion": "0.1.0",
        "postTreatmentBackdrop": request["backdrop"]["lower"],
        "adjacentColor": request["adjacentRanges"][0]["lower"],
    });
    for member in [
        "theme",
        "surface",
        "size",
        "appearance",
        "foregroundRole",
        "minimumContentContrast",
        "minimumEdgeContrast",
    ] {
        body[member] = request[member].clone();
    }
    let mut paint = json!({"schemaVersion": "0.1.0", "body": body});
    if let Some(surrounding) = request.get("surroundingRanges") {
        paint["surroundingColor"] = surrounding[0]["lower"].clone();
    }
    serde_json::to_value(resolve_surface_paint_source(&paint.to_string()).unwrap()).unwrap()
}

fn drop_retained_representation(paint: &mut Value) {
    if let Some(binding) = paint.pointer_mut("/focus/indicator/binding") {
        let binding = binding.as_object_mut().unwrap();
        binding.remove("frostRepresentation");
        binding.remove("frostPortableBody");
    }
}

#[test]
fn the_opaque_branch_is_the_surface_paint_for_both_reasons() {
    for (reason, level, high_contrast) in [("selection", 0.9, true), ("legibility", 0.1, false)] {
        for states in [
            json!(["rest"]),
            json!(["focused"]),
            json!(["rest", "focused"]),
        ] {
            let mut request = baseline();
            set(
                &mut request,
                "/theme/environment/accessibilityPreferences/highContrast",
                json!(high_contrast),
            );
            set(&mut request, "/surface/states/states", states.clone());
            set(&mut request, "/backdrop", point(level));
            set(&mut request, "/adjacentRanges", json!([point(0.95)]));
            set(&mut request, "/surroundingRanges", json!([point(0.95)]));
            if states == json!(["rest"]) {
                remove(&mut request, "/surroundingRanges");
            }
            let result = resolve(&request).unwrap();
            assert_eq!(result["fallbackReason"], reason, "{states}");
            let mut paint = result["paint"].clone();
            let mut reference = surface_paint_equivalent(&request);
            assert_eq!(paint["body"], reference["body"], "{reason} {states}");
            if paint.get("focus").is_some() {
                let binding = &paint["focus"]["indicator"]["binding"];
                assert_eq!(binding["frostRepresentation"], "opaqueDimensional");
                assert_eq!(binding["frostPortableBody"], binding["opaqueColorFallback"]);
                drop_retained_representation(&mut paint);
                drop_retained_representation(&mut reference);
            }
            assert_eq!(paint, reference, "{reason} {states}");
        }
    }
}

#[test]
fn a_ring_retains_the_representation_the_body_resolved_to() {
    let mut request = baseline();
    set(&mut request, "/surface/states/states", json!(["focused"]));
    let kept = resolve(&request).unwrap();
    assert_eq!(
        kept["focus"]["indicator"]["binding"]["frostRepresentation"],
        kept["frostRepresentation"]
    );
    set(&mut request, "/backdrop", point(0.1));
    let fallen = resolve(&request).unwrap();
    assert_eq!(fallen["fallbackReason"], "legibility");
    assert_eq!(fallen["frostRepresentation"], "opaqueDimensional");
    assert_eq!(
        fallen["paint"]["focus"]["indicator"]["binding"]["frostRepresentation"],
        "opaqueDimensional"
    );
    assert_eq!(
        fallen["paint"]["body"]["frostRepresentation"],
        fallen["paint"]["focus"]["indicator"]["binding"]["frostRepresentation"]
    );
}

#[test]
fn each_backdrop_end_can_bind_the_certificate() {
    let mut request = baseline();
    set(&mut request, "/foregroundRole", json!("surface.base"));
    set(
        &mut request,
        "/backdrop",
        json!({"lower": color_at(0.0), "upper": color_at(0.2)}),
    );
    set(&mut request, "/minimumContentContrast", json!(1.0));
    let light = resolve(&request).unwrap();
    let bound = number(&light["body"]["contentContrastRatio"]);
    let channels = |value: &Value| -> [f64; 3] {
        let list = value["components"].as_array().unwrap();
        [number(&list[0]), number(&list[1]), number(&list[2])]
    };
    let luminance = |c: [f64; 3]| {
        let [r, g, b] = c.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let foreground = luminance(channels(&light["body"]["foreground"]));
    let against = |end: &str| {
        let backdrop = luminance(channels(&light["body"]["backdrop"]["compositedBody"][end]));
        (foreground.max(backdrop) + 0.05) / (foreground.min(backdrop) + 0.05)
    };
    assert!((against("upper") - bound).abs() < 1e-12);
    assert!(bound < against("lower"));
    set(
        &mut request,
        "/minimumContentContrast",
        json!((bound + against("lower")) / 2.0),
    );
    assert!(resolve(&request).unwrap_err().contains("insufficient"));
    set(&mut request, "/minimumContentContrast", json!(bound - 1e-9));
    assert_eq!(resolve(&request).unwrap()["appearance"], "translucent");

    set(
        &mut request,
        "/backdrop",
        json!({"lower": color_at(0.0), "upper": color_at(1.0)}),
    );
    set(&mut request, "/minimumContentContrast", json!(1.0));
    let crossing = resolve(&request).unwrap();
    assert_eq!(crossing["body"]["contentContrastRatio"], 1.0);
}
