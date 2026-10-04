use resina_resolver::{resolve_command_paint_source, resolve_toggle_part_paint_source};
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
    assert!(ir.paint().body().content_contrast_ratio() >= 4.5);
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
