#[path = "support/diagnostics.rs"]
mod diagnostics;

use resina_model::{PhysicalBounds, PhysicalVector, SurfaceSize};
use resina_resolver::{
    HitRegionError, HitRegionInput, resolve_hit_region, resolve_hit_region_source,
};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-request.json"
    ))
    .unwrap()
}

#[test]
fn public_placements_and_failures_match_exactly() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut input = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *input.pointer_mut(change["path"].as_str().unwrap()).unwrap() = change["value"].clone();
        }
        let result = resolve_hit_region_source(&input.to_string());
        if let Some(expected) = case.get("expected") {
            let actual = serde_json::to_value(result.unwrap()).unwrap();
            assert_eq!(actual["schemaVersion"], expected["schemaVersion"]);
            for field in [
                "/bounds/x",
                "/bounds/y",
                "/bounds/width",
                "/bounds/height",
                "/minimumSize/width",
                "/minimumSize/height",
            ] {
                assert_eq!(
                    actual.pointer(field).unwrap().as_f64(),
                    expected.pointer(field).unwrap().as_f64(),
                    "{} {field}",
                    case["name"]
                );
            }
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(&diagnostics::expected("hit_region", &case["name"])),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn bounds_containment_is_closed_and_preserves_endpoint_residuals() {
    let mut request = request();
    let origin = 9_007_199_254_740_992.0;
    request["visualBounds"] = serde_json::json!({"x":origin,"y":0,"width":24,"height":24});
    request["availableBounds"] = serde_json::json!({"x":origin,"y":0,"width":1024,"height":1024});
    let hit = resolve_hit_region_source(&request.to_string()).unwrap();
    assert!(hit.contains_bounds(hit.bounds()).unwrap());
    for (width, contained) in [(24.0_f64.next_down(), true), (24.0_f64.next_up(), false)] {
        assert_eq!(
            hit.contains_bounds(PhysicalBounds {
                width,
                ..hit.bounds()
            })
            .unwrap(),
            contained
        );
    }
    assert!(
        !hit.contains(PhysicalVector {
            x: origin + 24.0,
            y: 0.0
        })
        .unwrap()
    );
    assert!(matches!(
        hit.contains_bounds(PhysicalBounds {
            width: f64::NAN,
            ..hit.bounds()
        }),
        Err(HitRegionError::InvalidBounds("contained bounds"))
    ));
}

#[test]
fn adjacent_targets_have_one_owner_at_shared_edges() {
    let first = resolve_hit_region_source(&request().to_string()).unwrap();
    let mut second = request();
    second["visualBounds"]["x"] = 24.into();
    second["occupiedRegions"] = serde_json::json!([first.bounds()]);
    let second = resolve_hit_region_source(&second.to_string()).unwrap();
    for (x, owners) in [
        (22.0_f64.next_down(), (true, false)),
        (22.0, (false, true)),
        (22.0_f64.next_up(), (false, true)),
    ] {
        let point = PhysicalVector { x, y: 0.0 };
        assert_eq!(
            (
                first.contains(point).unwrap(),
                second.contains(point).unwrap()
            ),
            owners
        );
    }
    assert!(first.contains(PhysicalVector { x: -2.0, y: -2.0 }).unwrap());
    assert!(!first.contains(PhysicalVector { x: 0.0, y: 22.0 }).unwrap());
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for point in [
            PhysicalVector { x: value, y: 0.0 },
            PhysicalVector { x: 0.0, y: value },
        ] {
            assert!(matches!(
                first.contains(point),
                Err(HitRegionError::InvalidPoint)
            ));
        }
    }
}

#[test]
fn membership_matches_independent_exact_arithmetic_vectors() {
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-membership-vectors.json"
    ))
    .unwrap();
    for vector in vectors {
        let mut input = request();
        input["visualBounds"] = vector["bounds"].clone();
        input["availableBounds"] =
            serde_json::json!({"x":-1_000_000,"y":-1_000_000,"width":2_000_000,"height":2_000_000});
        let hit = resolve_hit_region_source(&input.to_string()).unwrap();
        for point in vector["points"].as_array().unwrap() {
            let point_value = PhysicalVector {
                x: point["x"].as_f64().unwrap(),
                y: point["y"].as_f64().unwrap(),
            };
            assert_eq!(
                hit.contains(point_value).unwrap(),
                point["expected"].as_bool().unwrap(),
                "{} {point}",
                vector["name"]
            );
        }
    }
}

#[test]
fn typed_inputs_cannot_bypass_finite_or_extent_validation() {
    let environment = serde_json::from_value(request()["environment"].clone()).unwrap();
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let result = resolve_hit_region(HitRegionInput {
            environment: &environment,
            visual_bounds: PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width,
                height: 20.0,
            },
            available_bounds: PhysicalBounds {
                x: -32.0,
                y: -32.0,
                width: 100.0,
                height: 100.0,
            },
            component_minimum: SurfaceSize {
                width: 1.0,
                height: 1.0,
            },
            occupied_regions: &[],
        });
        assert!(matches!(
            result,
            Err(HitRegionError::InvalidBounds("visualBounds"))
        ));
    }
}

#[test]
fn finite_rounded_endpoint_cannot_hide_overflow() {
    let environment = serde_json::from_value(request()["environment"].clone()).unwrap();
    let origin = f64::MAX.next_down();
    let width = (f64::MAX - origin) * 1.25;
    assert_eq!(origin + width, f64::MAX);
    let result = resolve_hit_region(HitRegionInput {
        environment: &environment,
        visual_bounds: PhysicalBounds {
            x: origin,
            y: 0.0,
            width,
            height: 24.0,
        },
        available_bounds: PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: f64::MAX,
            height: 100.0,
        },
        component_minimum: SurfaceSize {
            width: 1.0,
            height: 1.0,
        },
        occupied_regions: &[],
    });
    assert!(matches!(
        result,
        Err(HitRegionError::NumericRange("visualBounds"))
    ));
}

#[test]
fn strict_cli_emits_no_partial_geometry() {
    let base = request().to_string();
    let mut unknown = request();
    unknown["visualBounds"]["extra"] = true.into();
    for source in [
        base.replacen(
            "\"schemaVersion\":\"0.1.0\"",
            "\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"",
            1,
        ),
        unknown.to_string(),
        base.replace("\"width\":20", "\"width\":1e999"),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-hit-region"))
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn cli_rejects_invalid_utf8_and_distinguishes_usage_errors() {
    let binary = env!("CARGO_BIN_EXE_resina-hit-region");
    let output = Command::new(binary).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
    let mut child = Command::new(binary)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&[0xff]).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn positional_source_records_fail_before_geometry_publication() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-cases.json"
    ))
    .unwrap();
    let mut requests = Vec::new();
    for case in cases.iter().filter(|case| {
        let name = case["name"].as_str().unwrap();
        name.starts_with("positional ") || name.starts_with("later positional ")
    }) {
        let mut input = request();
        for change in case["requestChanges"].as_array().unwrap() {
            *input.pointer_mut(change["path"].as_str().unwrap()).unwrap() = change["value"].clone();
        }
        requests.push(input);
    }
    assert_eq!(requests.len(), 5);
    let base = request();
    requests.push(Value::Array(
        [
            "schemaVersion",
            "environment",
            "visualBounds",
            "availableBounds",
            "componentMinimum",
            "occupiedRegions",
        ]
        .map(|field| base[field].clone())
        .into(),
    ));
    for input in requests {
        let source = input.to_string();
        assert!(matches!(
            resolve_hit_region_source(&source),
            Err(HitRegionError::InvalidShape(_))
        ));
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-hit-region"))
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn escaped_geometry_members_preserve_resolution_and_duplicate_rejection() {
    let source = request().to_string();
    let expected = resolve_hit_region_source(&source).unwrap();
    for (field, escaped) in [
        ("visualBounds", "\\u0076isualBounds"),
        ("availableBounds", "\\u0061vailableBounds"),
        ("componentMinimum", "\\u0063omponentMinimum"),
        ("occupiedRegions", "\\u006fccupiedRegions"),
    ] {
        let escaped = source.replace(&format!("\"{field}\""), &format!("\"{escaped}\""));
        assert_eq!(resolve_hit_region_source(&escaped).unwrap(), expected);
    }
    let duplicate = source.replacen("\"x\":0", "\"x\":0,\"\\u0078\":0", 1);
    assert!(
        resolve_hit_region_source(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate JSON member")
    );
}
