use resina_model::SliderValue;
use resina_resolver::{
    SliderAdjustmentError, SliderKey, SliderKeyError, SliderKeyInput, SliderKeyOutcome,
    SliderKeyPolicy, SliderKeySteps, SliderStops, SliderStopsError, SliderTieBreak,
    SliderValuePolicy, resolve_slider_key, resolve_slider_value,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[path = "common/slider_value_policy.rs"]
mod value_policy;

fn explicit_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Steps {
    Continuous {
        step: f64,
        #[serde(deserialize_with = "explicit_option")]
        page: Option<f64>,
    },
    Stops {
        step: u64,
        #[serde(deserialize_with = "explicit_option")]
        page: Option<u64>,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy {
    steps: Steps,
    increase_on_right: bool,
    increase_on_up: bool,
}
impl Policy {
    fn resolve(&self) -> Result<SliderKeyPolicy, SliderKeyError> {
        let steps = match self.steps {
            Steps::Continuous { step, page } => SliderKeySteps::Continuous { step, page },
            Steps::Stops { step, page } => SliderKeySteps::Stops { step, page },
        };
        SliderKeyPolicy::try_new(steps, self.increase_on_right, self.increase_on_up)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    current: SliderValue,
    value_policy: value_policy::Policy,
    key_policy: Policy,
    key: SliderKey,
    enabled: bool,
    read_only: bool,
    focused: bool,
    expected: Option<Value>,
    error: Option<String>,
}
fn error_name(error: &SliderKeyError) -> &'static str {
    match error {
        SliderKeyError::InvalidStep(_) => "invalidStep",
        SliderKeyError::InvalidPage => "invalidPage",
        SliderKeyError::PolicyMismatch => "policyMismatch",
        SliderKeyError::ValuePolicy(_) => "valuePolicy",
        SliderKeyError::Adjustment(_) => "adjustment",
    }
}

#[test]
fn public_cases_verify_complete_policy_results_and_numeric_regressions() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-key-cases.json"
    ))
    .unwrap();
    assert_eq!(source["schemaVersion"], "0.1.0");
    let cases = source["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 759);
    for raw in cases {
        let case: Case = serde_json::from_value(raw.clone()).unwrap();
        let current = resolve_slider_value(&case.current).unwrap();
        let domain = case.value_policy.resolve();
        let result = case.key_policy.resolve().and_then(|policy| {
            let mut expected_policy = raw["keyPolicy"].clone();
            expected_policy["schemaVersion"] = "0.1.0".into();
            assert_eq!(
                serde_json::to_value(policy).unwrap(),
                expected_policy,
                "{}",
                case.name
            );
            resolve_slider_key(SliderKeyInput {
                current: &current,
                value_policy: &domain,
                key_policy: &policy,
                key: case.key,
                enabled: case.enabled,
                read_only: case.read_only,
                focused: case.focused,
            })
        });
        if let Some(error) = case.error {
            assert_eq!(error_name(&result.unwrap_err()), error, "{}", case.name);
            assert!(case.expected.is_none());
        } else {
            let result = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case.expected.unwrap(),
                "{}",
                case.name
            );
            assert_eq!(
                result.commit().is_some(),
                result.outcome() == SliderKeyOutcome::Adjusted
            );
            if let Some(commit) = result.commit() {
                assert!(commit.accepted());
                assert_eq!(commit.value(), result.value());
                assert_eq!(commit.changed(), result.value() != &current);
            } else {
                assert_eq!(result.value(), &current);
            }
        }
    }
}

#[test]
fn quantum_validation_rejects_nonfinite_and_nonpositive_configuration() {
    for step in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -0.0, -1.0] {
        let error =
            SliderKeyPolicy::try_new(SliderKeySteps::Continuous { step, page: None }, true, true)
                .unwrap_err();
        assert!(matches!(error, SliderKeyError::InvalidStep(_)));
        assert!(error.to_string().contains("step"));
        assert!(std::error::Error::source(&error).is_none());
    }
    for page in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        -1.0,
        2.5,
        2.0,
    ] {
        let error = SliderKeyPolicy::try_new(
            SliderKeySteps::Continuous {
                step: 2.5,
                page: Some(page),
            },
            true,
            true,
        )
        .unwrap_err();
        assert!(matches!(error, SliderKeyError::InvalidPage));
        assert!(error.to_string().contains("greater than step"));
    }
    for steps in [
        SliderKeySteps::Stops {
            step: 0,
            page: None,
        },
        SliderKeySteps::Stops {
            step: 1,
            page: Some(1),
        },
    ] {
        assert!(SliderKeyPolicy::try_new(steps, true, true).is_err());
    }
}

#[test]
fn missing_page_policy_is_not_silently_deserialized_as_no_page() {
    for kind in ["continuous", "stops"] {
        let raw = serde_json::json!({"steps":{"kind":kind,"step":1},"increaseOnRight":true,"increaseOnUp":true});
        assert!(serde_json::from_value::<Policy>(raw).is_err());
    }
}

#[test]
fn repeated_delivery_uses_live_current_and_permission_without_stop_drift() {
    let domain = SliderValuePolicy::Stops {
        stops: SliderStops::try_new(0.0, 1.0, &[0.0, 0.1, 0.3, 0.7, 1.0]).unwrap(),
        tie_break: SliderTieBreak::Higher,
    };
    let policy = SliderKeyPolicy::try_new(
        SliderKeySteps::Stops {
            step: 1,
            page: None,
        },
        true,
        true,
    )
    .unwrap();
    let mut current = resolve_slider_value(&SliderValue::try_new(0.0, 1.0, 0.0).unwrap()).unwrap();
    for expected in [0.1, 0.3, 0.7, 1.0, 1.0] {
        let result = resolve_slider_key(SliderKeyInput {
            current: &current,
            value_policy: &domain,
            key_policy: &policy,
            key: SliderKey::ArrowRight,
            enabled: true,
            read_only: false,
            focused: true,
        })
        .unwrap();
        assert_eq!(result.value().value().value(), expected);
        current = *result.value();
    }
    current = resolve_slider_value(&SliderValue::try_new(0.0, 1.0, 0.3).unwrap()).unwrap();
    for (enabled, read_only, focused) in [
        (false, false, true),
        (true, true, true),
        (true, false, false),
    ] {
        let result = resolve_slider_key(SliderKeyInput {
            current: &current,
            value_policy: &domain,
            key_policy: &policy,
            key: SliderKey::ArrowRight,
            enabled,
            read_only,
            focused,
        })
        .unwrap();
        assert_eq!(result.value(), &current);
        assert!(result.commit().is_none());
        assert_eq!(result.outcome(), SliderKeyOutcome::Unavailable);
    }
    let result = resolve_slider_key(SliderKeyInput {
        current: &current,
        value_policy: &domain,
        key_policy: &policy,
        key: SliderKey::ArrowRight,
        enabled: true,
        read_only: false,
        focused: true,
    })
    .unwrap();
    assert_eq!(result.value().value().value(), 0.7);
}

#[test]
fn errors_preserve_domain_and_numeric_causes_without_partial_commit() {
    let domain = SliderValuePolicy::Stops {
        stops: SliderStops::try_new(0.0, 1.0, &[0.0, 1.0]).unwrap(),
        tie_break: SliderTieBreak::Lower,
    };
    let policy = SliderKeyPolicy::try_new(
        SliderKeySteps::Stops {
            step: 1,
            page: None,
        },
        true,
        true,
    )
    .unwrap();
    let current = resolve_slider_value(&SliderValue::try_new(0.0, 1.0, 0.5).unwrap()).unwrap();
    let error = resolve_slider_key(SliderKeyInput {
        current: &current,
        value_policy: &domain,
        key_policy: &policy,
        key: SliderKey::PageUp,
        enabled: false,
        read_only: true,
        focused: false,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        SliderKeyError::ValuePolicy(SliderStopsError::CurrentNotAllowed)
    ));
    assert!(std::error::Error::source(&error).is_some());

    let current =
        resolve_slider_value(&SliderValue::try_new(-1e300, 1e300, 1e200).unwrap()).unwrap();
    let policy = SliderKeyPolicy::try_new(
        SliderKeySteps::Continuous {
            step: 1.0,
            page: None,
        },
        true,
        true,
    )
    .unwrap();
    let error = resolve_slider_key(SliderKeyInput {
        current: &current,
        value_policy: &SliderValuePolicy::Continuous,
        key_policy: &policy,
        key: SliderKey::ArrowRight,
        enabled: true,
        read_only: false,
        focused: true,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        SliderKeyError::Adjustment(SliderAdjustmentError::NumericRange)
    ));
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(current.value().value(), 1e200);
}
