use resina_environment::LayoutDirection;
use resina_resolver::{SliderOrientation, resolve_slider_snapshot};
use serde_json::{Value, json};
use std::{convert::Infallible, fs::OpenOptions, io::Write};

#[path = "common/slider_snapshot.rs"]
mod fixture;
use fixture::{Fixture, FixtureConfig, size};

fn resolve_fixture(
    family: &str,
    direction: LayoutDirection,
    orientation: SliderOrientation,
    pose: &str,
    text_scale: f64,
) -> Fixture {
    Fixture::resolve(
        FixtureConfig {
            family,
            direction,
            orientation,
            enabled: !matches!(pose, "disabled" | "disabledFocused"),
            focused: matches!(pose, "focused" | "disabledFocused" | "readOnly" | "preview"),
            read_only: pose == "readOnly",
            preview: pose == "preview",
            tracking: 0.25,
            text_scale,
            text: "Volume",
            label_maximum_size: size(100.0, 64.0),
        },
        |input| {
            Ok::<_, Infallible>(size(
                80.0,
                input.typography.font_size() * input.typography.line_height(),
            ))
        },
    )
}

fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12,
                "{path}"
            );
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, expected) in b {
                compare(a.get(key).unwrap(), expected, &format!("{path}/{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (index, (actual, expected)) in a.iter().zip(b).enumerate() {
                compare(actual, expected, &format!("{path}/{index}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn complete_snapshot_matches_authored_portable_record() {
    let f = resolve_fixture(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        "focused",
        1.0,
    );
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/slider-snapshot-ir.json"
    ))
    .unwrap();
    let actual = serde_json::to_value(&snapshot).unwrap();
    compare(&actual, &expected, "");
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(encoded, serde_json::to_string(&snapshot).unwrap());
    assert_eq!(actual, serde_json::from_str::<Value>(&encoded).unwrap());
}

#[test]
fn complete_export_matrix_preserves_checked_channels() {
    let mut records = Vec::new();
    for family in ["cast", "frost", "elastomer"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for orientation in [SliderOrientation::Horizontal, SliderOrientation::Vertical] {
                for pose in [
                    "rest",
                    "focused",
                    "disabled",
                    "disabledFocused",
                    "readOnly",
                    "preview",
                ] {
                    for text_scale in [1.0, 2.0] {
                        let f = resolve_fixture(family, direction, orientation, pose, text_scale);
                        let snapshot = resolve_slider_snapshot(f.input()).unwrap();
                        let actual = serde_json::to_value(&snapshot).unwrap();
                        assert_eq!(actual["schemaVersion"], "0.1.0");
                        assert_eq!(
                            actual["presentation"],
                            serde_json::to_value(&f.presentation).unwrap()
                        );
                        assert_eq!(actual["layout"], serde_json::to_value(f.layout).unwrap());
                        assert_eq!(actual["track"], serde_json::to_value(&f.track).unwrap());
                        assert_eq!(actual["thumb"], serde_json::to_value(&f.thumb).unwrap());
                        assert_eq!(actual["label"], serde_json::to_value(&f.label).unwrap());
                        assert_eq!(
                            actual["accessibility"]["value"]["value"],
                            actual["presentation"]["visible"]["value"]
                        );
                        if pose == "preview" {
                            assert_ne!(
                                actual["presentation"]["committed"],
                                actual["presentation"]["visible"]
                            );
                        }
                        records.push(json!({"config":{"family":family,"direction":direction,
                            "orientation":orientation,"pose":pose,"textScale":text_scale},"snapshot":actual}));
                    }
                }
            }
        }
    }
    if let Some(path) = std::env::var_os("RESINA_SLIDER_SNAPSHOT_CAPTURE") {
        let bytes = serde_json::to_vec(&records).unwrap();
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }
}

#[test]
fn exported_record_retains_its_resolved_context() {
    let f = resolve_fixture(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        "focused",
        1.0,
    );
    let mut environment = f.environment.clone();
    let mut foreground = f.black.clone();
    let mut description = String::from("Output level");
    let mut input = f.input();
    input.environment = &environment;
    input.label_foreground = &foreground;
    input.description = Some(&description);
    let snapshot = resolve_slider_snapshot(input).unwrap();
    let before = serde_json::to_string(&snapshot).unwrap();
    foreground = f.white.clone();
    description.clear();
    let mut changed = serde_json::to_value(&environment).unwrap();
    changed["accessibilityPreferences"]["reducedMotion"] = json!(false);
    environment = serde_json::from_value(changed).unwrap();
    assert_eq!(foreground, f.white);
    assert!(description.is_empty());
    assert!(!environment.accessibility_preferences().reduced_motion);
    assert_eq!(before, serde_json::to_string(&snapshot).unwrap());
}
