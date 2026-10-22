#[path = "support/diagnostics.rs"]
mod diagnostics;

use resina_resolver::{
    ImperceptibleResponse, TogglePartPaintError, resolve_command_paint_source,
    resolve_toggle_part_paint_source,
};
use serde_json::{Value, json};

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-paint-request.json"
    ))
    .unwrap()
}

#[test]
fn checked_selection_preserves_every_channel_and_single_track_navigation() {
    let names = ["rest", "hover", "focused", "pressed", "checked", "disabled"];
    for family in ["cast", "frost", "elastomer"] {
        for part in ["track", "thumb"] {
            for bits in 1..64 {
                let states: Vec<_> = names
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| bits & (1 << index) != 0)
                    .map(|(_, state)| *state)
                    .collect();
                let mut r = request();
                r["part"] = json!(part);
                r["surface"]["body"]["surface"]["states"]["states"] = json!(states);
                let mut theme: Value = serde_json::from_str(
                    r["surface"]["body"]["theme"]["themeSource"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
                theme["materialAssignments"]["control"]["interactive"] = json!(family);
                r["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
                if family == "frost" {
                    r["surface"]["body"]["postTreatmentBackdrop"] =
                        json!({"colorSpace":"srgb","components":[1,1,1],"alpha":1});
                }
                let ir = resolve_toggle_part_paint_source(&r.to_string()).unwrap();
                let encoded = serde_json::to_value(&ir).unwrap();
                let checked = states.contains(&"checked");
                assert_eq!(encoded["checked"], checked);
                assert_eq!(encoded["part"], part);
                assert_eq!(encoded["paint"]["body"]["states"]["states"], json!(states));
                assert_eq!(
                    encoded["paint"]["body"]["colorRole"],
                    if checked { "selection" } else { "surface.base" }
                );
                let phase = ["disabled", "pressed", "hover"]
                    .into_iter()
                    .find(|s| states.contains(s))
                    .unwrap_or("rest");
                assert_eq!(encoded["phase"], phase);
                let response = if phase == "rest" {
                    json!({"bodyMix":0.0,"depthScale":1.0})
                } else {
                    r["interactionAppearance"]["profiles"][family][phase].clone()
                };
                let mix = response["bodyMix"].as_f64().unwrap();
                let base = if checked {
                    [0.35, 0.65, 0.85]
                } else {
                    [0.8, 0.7, 0.6]
                };
                for (actual, channel) in ir
                    .paint()
                    .body()
                    .face()
                    .pigment()
                    .body()
                    .components()
                    .into_iter()
                    .zip(base)
                {
                    let expected = if mix < 0.0 {
                        channel * (1.0 + mix)
                    } else {
                        channel + (1.0 - channel) * mix
                    };
                    assert!(
                        (actual - expected).abs() <= 1e-12,
                        "{family} {part} {states:?}: actual {actual} expected {expected}: {encoded}"
                    );
                }
                assert_eq!(
                    ir.paint().focus().is_some(),
                    part == "track" && states.contains(&"focused")
                );
                if let Some(focus) = ir.paint().focus() {
                    assert_eq!(
                        focus.geometry().silhouette(),
                        ir.paint().body().geometry().silhouette()
                    );
                }
            }
        }
    }
}

#[test]
fn checked_does_not_enter_ordinary_command_paint() {
    let mut r: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    r["surface"]["body"]["surface"]["states"]["states"] = json!(["rest", "checked"]);
    assert!(
        resolve_command_paint_source(&r.to_string())
            .unwrap_err()
            .to_string()
            .contains("command paint supports only")
    );
}

#[test]
fn checked_frost_uses_actual_capabilities_and_legibility_fallback() {
    let mut r = request();
    let mut theme: Value = serde_json::from_str(
        r["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    theme["materialAssignments"]["control"]["interactive"] = json!("frost");
    r["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
    let env = &mut r["surface"]["body"]["theme"]["environment"];
    env["accessibilityPreferences"]["highContrast"] = json!(false);
    env["accessibilityPreferences"]["reducedTransparency"] = json!(false);
    env["rendererCapabilities"]["translucentSurfaces"] = json!(true);
    r["surface"]["body"]["postTreatmentBackdrop"] =
        json!({"colorSpace":"srgb","components":[0,0,0],"alpha":1});
    r["surface"]["body"]["surface"]["states"]["states"] = json!(["focused", "pressed", "checked"]);
    r["surface"]["body"]["minimumContentContrast"] = json!(1);
    assert!(
        resolve_toggle_part_paint_source(&r.to_string())
            .unwrap_err()
            .to_string()
            .contains("resolved opaque body")
    );
    r["surface"]["body"]["minimumContentContrast"] = json!(4.5);
    let ir = resolve_toggle_part_paint_source(&r.to_string()).unwrap();
    assert!(ir.paint().body().content_fallback_applied());
    assert!(ir.paint().body().face().content_contrast_ratio() >= 4.5);
    assert!(ir.checked());
    assert!(ir.paint().focus().is_some());
    assert_eq!(
        serde_json::to_value(ir.paint().body()).unwrap()["frostRepresentation"],
        "opaqueDimensional"
    );
    assert_eq!(
        r["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedTransparency"],
        false
    );
}

#[test]
fn navigation_owner_requires_surroundings_without_losing_thumb_focus_state() {
    let mut r = request();
    r["surface"]["body"]["surface"]["states"]["states"] = json!(["focused", "checked"]);
    r["surface"]
        .as_object_mut()
        .unwrap()
        .remove("surroundingColor");
    assert!(
        resolve_toggle_part_paint_source(&r.to_string())
            .unwrap_err()
            .to_string()
            .contains("requires surroundingColor")
    );
    r["part"] = json!("thumb");
    let ir = resolve_toggle_part_paint_source(&r.to_string()).unwrap();
    assert!(ir.paint().focus().is_none());
    assert_eq!(
        serde_json::to_value(ir.paint().body()).unwrap()["states"]["states"],
        json!(["focused", "checked"])
    );
}

#[test]
fn embedded_track_responses_are_perceivable_only_through_body_color() {
    let resolve = |checked: bool, response: Value| {
        let mut request = request();
        request["part"] = json!("track");
        request["surface"]["body"]["surface"]["form"]["elevation"] = json!("embedded");
        let states = if checked {
            json!(["hover", "checked"])
        } else {
            json!(["hover"])
        };
        request["surface"]["body"]["surface"]["states"]["states"] = states;
        request["interactionAppearance"]["profiles"]["elastomer"]["hover"] = response;
        resolve_toggle_part_paint_source(&request.to_string())
    };
    for checked in [false, true] {
        resolve(checked, json!({"bodyMix": -0.1, "depthScale": 1})).unwrap();
        let Err(TogglePartPaintError::Imperceptible(ImperceptibleResponse {
            depth_change, ..
        })) = resolve(checked, json!({"bodyMix": 0, "depthScale": 2}))
        else {
            panic!("a depth-only response cannot change an embedded track");
        };
        assert_eq!(depth_change, 0.0);
        assert!(matches!(
            resolve(checked, json!({"bodyMix": -0.03, "depthScale": 1})),
            Err(TogglePartPaintError::Imperceptible(_))
        ));
    }
}

#[test]
fn every_authored_response_of_the_bound_family_is_validated_in_any_state() {
    for part in ["track", "thumb"] {
        let mut request = request();
        request["part"] = json!(part);
        request["surface"]["body"]["surface"]["states"]["states"] = json!(["rest", "checked"]);
        request["interactionAppearance"]["profiles"]["elastomer"]["hover"] =
            json!({"bodyMix": 0.06, "depthScale": 1});
        assert!(matches!(
            resolve_toggle_part_paint_source(&request.to_string()),
            Err(TogglePartPaintError::Imperceptible(ImperceptibleResponse {
                phase: resina_model::CommandPhase::Hover,
                ..
            }))
        ));
    }
}

#[test]
fn profiles_are_validated_against_both_role_bodies_in_either_selection() {
    for checked in [false, true] {
        let mut request = request();
        request["part"] = json!("track");
        request["surface"]["body"]["surface"]["form"]["elevation"] = json!("embedded");
        request["surface"]["body"]["surface"]["states"]["states"] = if checked {
            json!(["rest", "checked"])
        } else {
            json!(["rest"])
        };
        request["interactionAppearance"]["profiles"]["elastomer"]["hover"] =
            json!({"bodyMix": -0.0701, "depthScale": 1});
        assert!(
            matches!(
                resolve_toggle_part_paint_source(&request.to_string()),
                Err(TogglePartPaintError::Imperceptible(_))
            ),
            "checked={checked}: the checked role body perceives too little of the hover"
        );
        request["interactionAppearance"]["profiles"]["elastomer"]["hover"] =
            json!({"bodyMix": -0.0783, "depthScale": 1});
        resolve_toggle_part_paint_source(&request.to_string()).unwrap();
    }
}

#[test]
fn public_toggle_part_cases_keep_failures_diagnostic() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-paint-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *request
                .pointer_mut(change["path"].as_str().unwrap())
                .unwrap() = change["value"].clone();
        }
        let result = resolve_toggle_part_paint_source(&request.to_string());
        if let Some(expected) = case.get("expectedPhase") {
            assert_eq!(
                serde_json::to_value(result.unwrap().phase()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(&diagnostics::expected("toggle_part_paint", &case["name"])),
                "{}: {error}",
                case["name"]
            );
        }
    }
}

fn admitting(mut request: Value) -> Value {
    let environment = &mut request["surface"]["body"]["theme"]["environment"];
    environment["accessibilityPreferences"]["highContrast"] = json!(false);
    environment["accessibilityPreferences"]["reducedMotion"] = json!(false);
    environment["rendererCapabilities"]["dynamicLighting"] = json!(true);
    environment["qualityPolicy"] = json!("balanced");
    request
}

#[test]
fn interaction_light_is_admitted_only_in_the_hover_and_pressed_phases() {
    let names = ["rest", "hover", "focused", "pressed", "checked", "disabled"];
    for part in ["track", "thumb"] {
        for bits in 1..64 {
            let states: Vec<_> = names
                .iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, state)| *state)
                .collect();
            let mut request = admitting(request());
            request["part"] = json!(part);
            request["surface"]["body"]["surface"]["states"]["states"] = json!(states);
            let lit = serde_json::to_value(
                resolve_toggle_part_paint_source(&request.to_string()).unwrap(),
            )
            .unwrap();
            let phase = lit["phase"].as_str().unwrap();
            let light = &lit["paint"]["body"]["face"]["light"];
            assert_eq!(light["ambient"], json!(0.25), "{part} {states:?}");
            assert_eq!(
                light["interaction"].is_object(),
                matches!(phase, "hover" | "pressed"),
                "{part} {states:?}"
            );
            request["light"] = json!({"ambient": null, "interaction": null});
            let mut unlit = serde_json::to_value(
                resolve_toggle_part_paint_source(&request.to_string()).unwrap(),
            )
            .unwrap();
            assert_eq!(
                unlit["paint"]["body"]["face"]["light"],
                json!({"keyDirection": light["keyDirection"], "ambient": 0.0, "interaction": null})
            );
            let mut lit = lit;
            for paint in [&mut lit, &mut unlit] {
                paint["paint"]["body"]["face"]
                    .as_object_mut()
                    .unwrap()
                    .remove("light");
            }
            assert_eq!(
                lit, unlit,
                "{part} {states:?}: light changes only face.light"
            );
        }
    }
}

#[test]
fn each_part_paints_its_own_relief_profile_without_state_or_family_selection() {
    let relief = request()["relief"].clone();
    for (part, profile) in [("track", "toggleTrack"), ("thumb", "toggleThumb")] {
        for family in ["cast", "frost", "elastomer"] {
            for states in [
                json!(["rest"]),
                json!(["pressed", "checked"]),
                json!(["disabled"]),
            ] {
                let mut request = request();
                request["part"] = json!(part);
                request["surface"]["body"]["surface"]["states"]["states"] = states;
                let mut theme: Value = serde_json::from_str(
                    request["surface"]["body"]["theme"]["themeSource"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
                theme["materialAssignments"]["control"]["interactive"] = json!(family);
                request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
                if family == "frost" {
                    request["surface"]["body"]["postTreatmentBackdrop"] =
                        json!({"colorSpace":"srgb","components":[1,1,1],"alpha":1});
                }
                let ir = serde_json::to_value(
                    resolve_toggle_part_paint_source(&request.to_string()).unwrap(),
                )
                .unwrap();
                let painted = &ir["paint"]["body"]["face"]["surface"]["profile"];
                assert_eq!(painted["kind"], relief[profile]["kind"], "{part} {family}");
                for member in ["bodyThickness", "amplitude", "shoulderWidth"] {
                    assert_eq!(
                        painted[member].as_f64(),
                        relief[profile][member].as_f64(),
                        "{part} {family} {member}"
                    );
                }
            }
        }
    }
}

#[test]
fn suppressed_channel_sources_are_still_validated() {
    for (states, high_contrast) in [("rest", false), ("disabled", false), ("hover", true)] {
        let mut request = admitting(request());
        request["surface"]["body"]["surface"]["states"]["states"] = json!([states]);
        request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["highContrast"] =
            json!(high_contrast);
        let source = request.to_string();
        for (member, escaped) in [
            (r#""ambient":0.25"#, r#""ambient":"#),
            (r#""strength":0.5"#, r#""strength":"#),
        ] {
            assert!(source.contains(member), "{member}");
            let plain = member.split(':').next().unwrap().to_owned() + ":";
            for (name, literal, valid) in [
                (plain, "-1e-400", false),
                (escaped.to_owned(), "-1e-400", false),
                (escaped.to_owned(), "-0e-400", true),
                (escaped.to_owned(), "1e-400", true),
            ] {
                let changed = source.replacen(member, &(name + literal), 1);
                let result = resolve_toggle_part_paint_source(&changed);
                assert_eq!(result.is_ok(), valid, "{states} {member} {literal}");
                if !valid {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("nonnegative before numeric rounding")
                    );
                }
            }
        }
    }
}

fn published_requests() -> [(&'static str, Value); 2] {
    let mut track = admitting(request());
    track["surface"]["body"]["surface"]["states"]["states"] =
        json!(["hover", "focused", "checked"]);
    let mut thumb = admitting(request());
    thumb["part"] = json!("thumb");
    thumb["surface"]["body"]["size"] = json!({"width": 16, "height": 16});
    thumb["surface"]["body"]["surface"]["states"]["states"] = json!(["pressed", "checked"]);
    [("track", track), ("thumb", thumb)]
}

#[test]
fn published_toggle_part_paint_ir_is_the_reference_result_of_the_public_requests() {
    let published: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-paint-ir.json"
    ))
    .unwrap();
    for (part, request) in published_requests() {
        let resolved =
            serde_json::to_value(resolve_toggle_part_paint_source(&request.to_string()).unwrap())
                .unwrap();
        assert_eq!(resolved, published[part], "{part}");
    }
}

#[test]
fn disabled_parts_keep_the_guarded_foreground_that_they_do_not_draw() {
    for part in ["track", "thumb"] {
        for high_contrast in [false, true] {
            let mut request = request();
            request["part"] = json!(part);
            request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["highContrast"] =
                json!(high_contrast);
            request["surface"]["body"]["surface"]["states"]["states"] = json!(["rest"]);
            let rest = serde_json::to_value(
                resolve_toggle_part_paint_source(&request.to_string()).unwrap(),
            )
            .unwrap();
            request["surface"]["body"]["surface"]["states"]["states"] =
                json!(["disabled", "checked"]);
            let disabled = serde_json::to_value(
                resolve_toggle_part_paint_source(&request.to_string()).unwrap(),
            )
            .unwrap();
            let foreground = &disabled["paint"]["body"]["face"]["foreground"];
            assert_eq!(foreground, &rest["paint"]["body"]["face"]["foreground"]);
            assert_eq!(foreground["components"], json!([0.0, 0.0, 0.0]));
            assert!(
                disabled["paint"]["body"]["face"]["contentContrastRatio"]
                    .as_f64()
                    .unwrap()
                    > 4.5
            );
        }
    }
}
