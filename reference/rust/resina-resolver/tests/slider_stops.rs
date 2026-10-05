use resina_model::SliderValue;
use resina_resolver::{
    SliderStopAdjustment, SliderStopInput, SliderStops, SliderStopsError, SliderTieBreak,
    resolve_slider_stop_adjustment, resolve_slider_value,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Domain {
    minimum: f64,
    maximum: f64,
    values: Vec<f64>,
}
impl Domain {
    fn resolve(&self) -> Result<SliderStops, SliderStopsError> {
        SliderStops::try_new(self.minimum, self.maximum, &self.values)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DomainCase {
    name: String,
    input: Domain,
    expected: Option<Value>,
    error: Option<String>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Action {
    SetValue {
        value: f64,
    },
    Nearest {
        value: f64,
        #[serde(rename = "tieBreak")]
        tie_break: SliderTieBreak,
    },
    Increase {
        count: u64,
    },
    Decrease {
        count: u64,
    },
    Minimum,
    Maximum,
}
impl Action {
    fn resolve(self) -> SliderStopAdjustment {
        match self {
            Self::SetValue { value } => SliderStopAdjustment::SetValue(value),
            Self::Nearest { value, tie_break } => {
                SliderStopAdjustment::Nearest { value, tie_break }
            }
            Self::Increase { count } => SliderStopAdjustment::Increase(count),
            Self::Decrease { count } => SliderStopAdjustment::Decrease(count),
            Self::Minimum => SliderStopAdjustment::Minimum,
            Self::Maximum => SliderStopAdjustment::Maximum,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AdjustmentCase {
    name: String,
    domain: Domain,
    current: SliderValue,
    enabled: bool,
    read_only: bool,
    adjustment: Action,
    expected: Option<Value>,
    error: Option<String>,
}
fn error_name(error: &SliderStopsError) -> &'static str {
    match error {
        SliderStopsError::InvalidBounds(_) => "invalidBounds",
        SliderStopsError::TooFewStops => "tooFewStops",
        SliderStopsError::InvalidStop { .. } => "invalidStop",
        SliderStopsError::UnorderedStops { .. } => "unorderedStops",
        SliderStopsError::EndpointMismatch => "endpointMismatch",
        SliderStopsError::CurrentBounds => "currentBounds",
        SliderStopsError::CurrentNotAllowed => "currentNotAllowed",
        SliderStopsError::InvalidCount => "invalidCount",
        SliderStopsError::InvalidTarget => "invalidTarget",
        SliderStopsError::TargetNotAllowed => "targetNotAllowed",
        SliderStopsError::NumericRange => "numericRange",
        SliderStopsError::Value { .. } => "value",
        SliderStopsError::Adjustment(_) => "adjustment",
    }
}
#[test]
fn public_domains_match_complete_checked_values_and_diagnostics() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-stops-cases.json"
    ))
    .unwrap();
    assert_eq!(corpus["schemaVersion"], "0.1.0");
    let cases: Vec<DomainCase> = serde_json::from_value(corpus["cases"].clone()).unwrap();
    assert_eq!(cases.len(), 20);
    for case in cases {
        let result = case.input.resolve();
        if let Some(error) = case.error {
            assert!(case.expected.is_none());
            assert_eq!(error_name(&result.unwrap_err()), error, "{}", case.name);
        } else {
            let result = result.unwrap_or_else(|e| panic!("{}: {e}", case.name));
            assert_eq!(
                serde_json::to_value(&result).unwrap(),
                case.expected.unwrap(),
                "{}",
                case.name
            );
            assert_eq!(result.values().len(), case.input.values.len());
            for (actual, expected) in result.values().iter().zip(case.input.values) {
                assert_eq!(actual.value().value(), expected);
            }
        }
    }
}
#[test]
fn public_adjustments_match_complete_results_and_live_permission() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-stop-adjustment-cases.json"
    ))
    .unwrap();
    assert_eq!(corpus["schemaVersion"], "0.1.0");
    let cases: Vec<AdjustmentCase> = serde_json::from_value(corpus["cases"].clone()).unwrap();
    assert_eq!(cases.len(), 131);
    for case in cases {
        let stops = case.domain.resolve().unwrap();
        let before = stops.clone();
        let current = resolve_slider_value(&case.current).unwrap();
        let result = resolve_slider_stop_adjustment(SliderStopInput {
            stops: &stops,
            current: &current,
            enabled: case.enabled,
            read_only: case.read_only,
            adjustment: case.adjustment.resolve(),
        });
        if let Some(error) = case.error {
            assert!(case.expected.is_none());
            assert_eq!(error_name(&result.unwrap_err()), error, "{}", case.name);
        } else {
            let result = result.unwrap_or_else(|e| panic!("{}: {e}", case.name));
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case.expected.unwrap(),
                "{}",
                case.name
            );
            assert!(stops.values().contains(result.value()));
            assert_eq!(result.accepted(), case.enabled && !case.read_only);
            assert_eq!(result.changed(), result.value() != &current);
        }
        assert_eq!(stops, before);
    }
}
#[test]
fn typed_nonfinite_inputs_fail_before_permission_without_mutation() {
    let stops = SliderStops::try_new(0.0, 1.0, &[0.0, 0.5, 1.0]).unwrap();
    let current = *stops.values().get(1).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            SliderStops::try_new(bad, 1.0, &[0.0, 1.0]),
            Err(SliderStopsError::InvalidBounds(_))
        ));
        assert!(matches!(
            SliderStops::try_new(0.0, bad, &[0.0, 1.0]),
            Err(SliderStopsError::InvalidBounds(_))
        ));
        let error = SliderStops::try_new(0.0, 1.0, &[0.0, bad, 1.0]).unwrap_err();
        assert!(matches!(
            error,
            SliderStopsError::InvalidStop { index: 1, .. }
        ));
        assert!(error.to_string().contains("stop 1"));
        for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)] {
            for adjustment in [
                SliderStopAdjustment::SetValue(bad),
                SliderStopAdjustment::Nearest {
                    value: bad,
                    tie_break: SliderTieBreak::Lower,
                },
            ] {
                let error = resolve_slider_stop_adjustment(SliderStopInput {
                    stops: &stops,
                    current: &current,
                    enabled,
                    read_only,
                    adjustment,
                })
                .unwrap_err();
                assert!(matches!(error, SliderStopsError::InvalidTarget));
                assert!(std::error::Error::source(&error).is_none());
                assert!(error.to_string().contains("finite"));
            }
        }
    }
    assert_eq!(current.value().value(), 0.5);
    assert_eq!(stops.values().len(), 3);
    let error =
        SliderStops::try_new(0.0, f64::MAX, &[0.0, f64::from_bits(1), f64::MAX]).unwrap_err();
    assert!(matches!(error, SliderStopsError::Value { index: 1, .. }));
    assert!(error.to_string().contains("stop 1"));
    assert!(std::error::Error::source(&error).is_some());
}
#[test]
fn step_replay_never_drifts_off_declared_fractional_values() {
    let declared = [0.0, 0.1, 0.3, 0.7, 1.0];
    let stops = SliderStops::try_new(0.0, 1.0, &declared).unwrap();
    let mut current = stops.values()[0];
    for expected in declared.into_iter().skip(1).chain([1.0]) {
        let result = resolve_slider_stop_adjustment(SliderStopInput {
            stops: &stops,
            current: &current,
            enabled: true,
            read_only: false,
            adjustment: SliderStopAdjustment::Increase(1),
        })
        .unwrap();
        assert_eq!(result.value().value().value(), expected);
        current = *result.value();
    }
    for expected in declared.into_iter().rev().skip(1).chain([0.0]) {
        let result = resolve_slider_stop_adjustment(SliderStopInput {
            stops: &stops,
            current: &current,
            enabled: true,
            read_only: false,
            adjustment: SliderStopAdjustment::Decrease(1),
        })
        .unwrap();
        assert_eq!(result.value().value().value(), expected);
        current = *result.value();
    }
    assert_eq!(current, stops.values()[0]);
}
#[test]
fn nearest_is_monotonic_and_preserves_exact_allowed_values() {
    let stops = SliderStops::try_new(-10.0, 30.0, &[-10.0, -2.0, 0.0, 7.0, 30.0]).unwrap();
    for tie_break in [SliderTieBreak::Lower, SliderTieBreak::Higher] {
        let mut previous = -10.0;
        for number in -80..=240 {
            let target = f64::from(number) * 0.125;
            let result = resolve_slider_stop_adjustment(SliderStopInput {
                stops: &stops,
                current: &stops.values()[2],
                enabled: true,
                read_only: false,
                adjustment: SliderStopAdjustment::Nearest {
                    value: target,
                    tie_break,
                },
            })
            .unwrap();
            let actual = result.value().value().value();
            assert!(actual >= previous);
            if let Some(stop) = stops.values().iter().find(|v| v.value().value() == target) {
                assert_eq!(result.value(), stop);
            }
            previous = actual;
        }
    }
}
