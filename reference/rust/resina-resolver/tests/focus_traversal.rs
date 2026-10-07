#[path = "support/diagnostics.rs"]
mod diagnostics;

use resina_resolver::{
    FocusDirection, FocusTarget, FocusTraversalError, FocusTraversalInput, resolve_focus_traversal,
    resolve_focus_traversal_source,
};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn public_cases_match_exact_results_and_diagnostics() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/focus-traversal-cases.json"
    ))
    .unwrap();
    for case in cases {
        let result = resolve_focus_traversal_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(&diagnostics::expected("focus_traversal", &case["name"])),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn every_small_scope_matches_directional_distance_ordering() {
    for length in 0..=6 {
        let ids: Vec<_> = (0..length).map(|index| format!("target-{index}")).collect();
        for mask in 0..1 << length {
            let targets: Vec<_> = ids
                .iter()
                .enumerate()
                .map(|(index, id)| FocusTarget {
                    id,
                    eligible: mask & (1 << index) != 0,
                })
                .collect();
            for current in std::iter::once(None).chain((0..length).map(Some)) {
                for direction in [FocusDirection::Forward, FocusDirection::Backward] {
                    for wrap in [false, true] {
                        let expected = (0..length)
                            .filter(|&index| targets[index].eligible)
                            .filter_map(|index| {
                                let distance = match (direction, current) {
                                    (FocusDirection::Forward, None) => index,
                                    (FocusDirection::Backward, None) => length - 1 - index,
                                    (FocusDirection::Forward, Some(current)) if index > current => {
                                        index - current
                                    }
                                    (FocusDirection::Backward, Some(current))
                                        if index < current =>
                                    {
                                        current - index
                                    }
                                    (FocusDirection::Forward, Some(current)) if wrap => {
                                        length - current + index
                                    }
                                    (FocusDirection::Backward, Some(current)) if wrap => {
                                        current + length - index
                                    }
                                    _ => return None,
                                };
                                Some((distance, index))
                            })
                            .min()
                            .map(|(_, index)| ids[index].as_str());
                        let result = resolve_focus_traversal(FocusTraversalInput {
                            targets: &targets,
                            current: current.map(|index| ids[index].as_str()),
                            direction,
                            wrap,
                        })
                        .unwrap();
                        assert_eq!(
                            result.target_id(),
                            expected,
                            "length={length} mask={mask} current={current:?} direction={direction:?} wrap={wrap}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_targets_fail_even_after_an_eligible_candidate() {
    for (targets, duplicate) in [
        (
            [
                FocusTarget {
                    id: "first",
                    eligible: true,
                },
                FocusTarget {
                    id: "first",
                    eligible: false,
                },
            ],
            true,
        ),
        (
            [
                FocusTarget {
                    id: "first",
                    eligible: true,
                },
                FocusTarget {
                    id: "",
                    eligible: false,
                },
            ],
            false,
        ),
    ] {
        let error = resolve_focus_traversal(FocusTraversalInput {
            targets: &targets,
            current: None,
            direction: FocusDirection::Forward,
            wrap: false,
        })
        .unwrap_err();
        if duplicate {
            assert!(matches!(error, FocusTraversalError::DuplicateId(_)));
        } else {
            assert!(matches!(error, FocusTraversalError::EmptyId(1)));
        }
    }
}

#[test]
fn cli_rejects_duplicate_input_invalid_utf8_and_usage() {
    let binary = env!("CARGO_BIN_EXE_resina-focus-traversal");
    let request = br#"{"schemaVersion":"0.1.0","targets":[{"id":"first","eligible":true,"eligible":false}],"current":null,"direction":"forward","wrap":false}"#;
    for source in [request.as_slice(), &[0xff]] {
        let mut child = Command::new(binary)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(source).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let output = Command::new(binary).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("Usage:"));
}
