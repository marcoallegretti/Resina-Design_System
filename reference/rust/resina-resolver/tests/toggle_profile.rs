use resina_environment::{LayoutDirection, SafeArea};
use resina_resolver::{ToggleLayoutInput, resolve_toggle_layout, resolve_toggle_profile_source};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const CASES: &str =
    include_str!("../../../../conformance/layout/toggle-profile-resolution-cases.json");
const TOKENS: &str = include_str!("../../../../tokens/components/toggle.json");
const PROFILE: &str = include_str!("../../../../definitions/layout/toggle.json");
const MATRIX: usize = 32;
const RESOLVED: usize = MATRIX + 8;

fn cases() -> Vec<Value> {
    serde_json::from_str(CASES).unwrap()
}

fn case(name: &str) -> Value {
    cases()
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap()
}

fn same(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => actual.as_f64() == expected.as_f64(),
        (Value::Object(actual), Value::Object(expected)) => {
            actual.len() == expected.len()
                && expected
                    .iter()
                    .all(|(key, value)| actual.get(key).is_some_and(|found| same(found, value)))
        }
        _ => actual == expected,
    }
}

#[test]
fn public_cases_resolve_to_their_expected_parts_or_diagnostic() {
    let mut resolved = 0;
    for case in cases() {
        let result = resolve_toggle_profile_source(&case["request"].to_string());
        match case.get("expected") {
            Some(expected) => {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                assert!(same(&actual, expected), "{}: {actual}", case["name"]);
                resolved += 1;
            }
            None => {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains(case["errorContains"].as_str().unwrap()),
                    "{}: {error}",
                    case["name"]
                );
            }
        }
    }
    assert_eq!(resolved, RESOLVED);
}

#[test]
fn authored_profile_and_tokens_are_the_ones_the_matrix_uses() {
    let authored: Value = serde_json::from_str(TOKENS).unwrap();
    let profile: Value = serde_json::from_str(PROFILE).unwrap();
    let tokens = resina_tokens::resolve_token_source(TOKENS).unwrap();
    let mut referenced = BTreeSet::new();
    for parts in profile["parts"].as_object().unwrap().values() {
        let mut paths = vec![parts["thumbInset"].as_str().unwrap()];
        for size in ["trackSize", "thumbSize"] {
            for axis in ["width", "height"] {
                paths.push(parts[size][axis].as_str().unwrap());
            }
        }
        for path in paths {
            assert_eq!(tokens[path].token_type, "dimension", "{path}");
            assert_eq!(tokens[path].value["unit"], "px", "{path}");
            referenced.insert(path.to_owned());
        }
    }
    assert_eq!(referenced, tokens.keys().cloned().collect());
    for case in cases().iter().take(MATRIX) {
        assert_eq!(case["request"]["tokens"], authored, "{}", case["name"]);
        assert_eq!(case["request"]["profile"], profile, "{}", case["name"]);
    }
}

#[test]
fn text_scale_and_input_never_change_the_parts() {
    for density in ["compact", "standard", "comfortable", "immersive"] {
        let mut request = case(&format!("{density} text 1 fine ltr"))["request"].clone();
        let baseline = resolve_toggle_profile_source(&request.to_string()).unwrap();
        for scale in [0.5, 1.5, 2.0, 3.0, 1e308] {
            for inputs in [
                json!(["finePointer", "hover", "keyboard"]),
                json!(["coarsePointer", "keyboard"]),
                json!(["directTouch", "keyboard"]),
            ] {
                request["environment"]["textScale"] = json!(scale);
                request["environment"]["inputCapabilities"] = inputs;
                assert_eq!(
                    resolve_toggle_profile_source(&request.to_string()).unwrap(),
                    baseline,
                    "{density} {scale}"
                );
            }
        }
    }
}

#[test]
fn resolved_parts_have_positive_travel_inside_the_track_for_both_directions_and_selections() {
    for density in ["compact", "standard", "comfortable", "immersive"] {
        for (direction, suffix) in [(LayoutDirection::Ltr, "ltr"), (LayoutDirection::Rtl, "rtl")] {
            let profile = resolve_toggle_profile_source(
                &case(&format!("{density} text 1 fine {suffix}"))["request"].to_string(),
            )
            .unwrap();
            assert_eq!(profile.layout_direction(), direction);
            let insets: &SafeArea = profile.insets();
            for checked in [false, true] {
                let layout = resolve_toggle_layout(ToggleLayoutInput {
                    track_size: profile.track_size(),
                    thumb_size: profile.thumb_size(),
                    insets,
                    layout_direction: direction,
                    checked,
                })
                .unwrap();
                let track = layout.track_bounds();
                let (off, on) = (layout.off_thumb_bounds(), layout.on_thumb_bounds());
                assert_ne!(off.x, on.x, "{density} {suffix}");
                for thumb in [off, on] {
                    assert!(
                        thumb.x >= insets.start.min(insets.end),
                        "{density} {suffix}"
                    );
                    assert!(thumb.x + thumb.width <= track.width - insets.start.min(insets.end));
                    assert!(
                        thumb.y >= insets.top
                            && thumb.y + thumb.height <= track.height - insets.bottom
                    );
                }
            }
        }
    }
}
