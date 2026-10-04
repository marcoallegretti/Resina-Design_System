use resina_model::SliderValue;
use resina_resolver::{SliderValueError, resolve_slider_value, resolve_slider_value_source};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn public_cases_match_authored_values_progress_and_diagnostics() {
    let cases: Vec<Box<serde_json::value::RawValue>> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-value-cases.json"
    ))
    .unwrap();
    #[derive(serde::Deserialize)]
    struct CaseRequest<'a> {
        #[serde(borrow)]
        request: &'a serde_json::value::RawValue,
    }
    for source in cases {
        let case: Value = serde_json::from_str(source.get()).unwrap();
        let request: CaseRequest<'_> = serde_json::from_str(source.get()).unwrap();
        let result = resolve_slider_value_source(request.request.get());
        if let Some(expected) = case.get("expected") {
            let result = result.unwrap();
            for (actual, key) in [
                (result.value().minimum(), "minimum"),
                (result.value().maximum(), "maximum"),
                (result.value().value(), "value"),
            ] {
                assert_eq!(
                    actual,
                    expected[key].as_f64().unwrap(),
                    "{}: {key}",
                    case["name"]
                );
            }
            let target = expected["progress"].as_f64().unwrap();
            if target == 0.0 || target == 1.0 {
                assert_eq!(result.progress(), target);
            } else {
                let ulp = f64::from_bits(target.to_bits() + 1) - target;
                assert!(
                    (result.progress() - target).abs() <= 4.0 * ulp,
                    "{}: progress",
                    case["name"]
                );
                assert!(result.progress() > 0.0 && result.progress() < 1.0);
            }
            let input: SliderValue = serde_json::from_value(case["request"].clone()).unwrap();
            assert_eq!(resolve_slider_value(&input).unwrap(), result);
            assert_eq!(
                serde_json::to_value(result).unwrap()["schemaVersion"],
                "0.1.0"
            );
        } else {
            let error = result.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}
#[test]
fn progression_is_monotonic_under_translation_and_positive_scaling() {
    for (minimum, maximum) in [
        (-100.0, 100.0),
        (1e300, 2e300),
        (-1e308, 1e308),
        (-1e-300, 1e-300),
    ] {
        let mut previous = -1.0;
        for index in 0..=100 {
            let fraction = f64::from(index) / 100.0;
            let value = if index == 0 {
                minimum
            } else if index == 100 {
                maximum
            } else {
                minimum * (1.0 - fraction) + maximum * fraction
            };
            let result =
                resolve_slider_value(&SliderValue::try_new(minimum, maximum, value).unwrap())
                    .unwrap();
            assert!(result.progress() >= previous);
            assert!((result.progress() - fraction).abs() < 1e-14);
            previous = result.progress();
        }
    }
}
#[test]
fn representability_failure_never_silently_becomes_an_endpoint() {
    let input = SliderValue::try_new(0.0, 1e308, f64::from_bits(1)).unwrap();
    assert!(matches!(
        resolve_slider_value(&input),
        Err(SliderValueError::NumericRange)
    ));
    let input = SliderValue::try_new(-1e308, 1.0, 0.0).unwrap();
    assert!(matches!(
        resolve_slider_value(&input),
        Err(SliderValueError::NumericRange)
    ));
}
#[test]
fn source_and_cli_reject_ambiguous_or_invalid_requests_without_output() {
    for field in ["schemaVersion", "minimum", "maximum", "value"] {
        let source = format!(
            r#"{{"schemaVersion":"0.1.0","minimum":0,"maximum":1,"value":0.5,"{field}":0}}"#
        );
        assert!(matches!(
            resolve_slider_value_source(&source),
            Err(SliderValueError::Parse(_))
        ));
    }
    for source in [
        r#"{"schemaVersion":"0.1.0","minimum":0,"maximum":1,"value":2}"#.as_bytes(),
        b"\xff",
        b"{}",
        b"{",
        b"{\"schemaVersion\":\"0.1.0\",\"minimum\":0,\"maximum\":1e999,\"value\":0}",
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-slider-value"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_resina-slider-value"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
