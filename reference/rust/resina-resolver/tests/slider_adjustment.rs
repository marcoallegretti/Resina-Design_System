use resina_model::SliderValue;
use resina_resolver::{
    SliderAdjustment as A, SliderAdjustmentError as E, SliderAdjustmentInput, SliderValueIr,
    resolve_slider_adjustment, resolve_slider_value,
};
fn value(minimum: f64, maximum: f64, value: f64) -> SliderValueIr {
    resolve_slider_value(&SliderValue::try_new(minimum, maximum, value).unwrap()).unwrap()
}
#[test]
fn explicit_intents_keep_bounds_and_distinguish_accepted_from_changed() {
    let current = value(-10.0, 30.0, 0.0);
    for (adjustment, target, changed) in [
        (A::SetValue(12.0), 12.0, true),
        (A::SetValue(0.0), 0.0, false),
        (A::Increase(5.0), 5.0, true),
        (A::Decrease(5.0), -5.0, true),
        (A::Increase(f64::MAX), 30.0, true),
        (A::Decrease(f64::MAX), -10.0, true),
        (A::Minimum, -10.0, true),
        (A::Maximum, 30.0, true),
    ] {
        let result = resolve_slider_adjustment(SliderAdjustmentInput {
            current: &current,
            enabled: true,
            read_only: false,
            adjustment,
        })
        .unwrap();
        assert!(result.accepted());
        assert_eq!(result.changed(), changed);
        assert_eq!(result.value().value().value(), target);
        assert_eq!(result.value().value().minimum(), -10.0);
        assert_eq!(result.value().value().maximum(), 30.0);
    }
    for (current, adjustment) in [
        (value(0.0, 1.0, 0.0), A::Decrease(1.0)),
        (value(0.0, 1.0, 1.0), A::Increase(1.0)),
    ] {
        let result = resolve_slider_adjustment(SliderAdjustmentInput {
            current: &current,
            enabled: true,
            read_only: false,
            adjustment,
        })
        .unwrap();
        assert!(result.accepted());
        assert!(!result.changed());
        assert_eq!(result.value(), &current);
    }
}
#[test]
fn live_availability_and_read_only_block_stale_action_delivery() {
    let mut current = value(0.0, 100.0, 25.0);
    let first = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &current,
        enabled: true,
        read_only: false,
        adjustment: A::Increase(10.0),
    })
    .unwrap();
    current = *first.value();
    for (enabled, read_only) in [(false, false), (true, true), (false, true)] {
        for adjustment in [
            A::SetValue(50.0),
            A::Increase(10.0),
            A::Decrease(10.0),
            A::Minimum,
            A::Maximum,
        ] {
            let result = resolve_slider_adjustment(SliderAdjustmentInput {
                current: &current,
                enabled,
                read_only,
                adjustment,
            })
            .unwrap();
            assert!(!result.accepted());
            assert!(!result.changed());
            assert_eq!(result.value(), &current);
        }
    }
    current = value(0.0, 100.0, 70.0);
    let next = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &current,
        enabled: true,
        read_only: false,
        adjustment: A::Decrease(10.0),
    })
    .unwrap();
    assert_eq!(next.value().value().value(), 60.0);
}
#[test]
fn malformed_intents_fail_even_when_unavailable() {
    let current = value(0.0, 1.0, 0.5);
    for (enabled, read_only) in [(true, false), (false, false), (true, true)] {
        for amount in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for adjustment in [A::Increase(amount), A::Decrease(amount)] {
                assert!(matches!(
                    resolve_slider_adjustment(SliderAdjustmentInput {
                        current: &current,
                        enabled,
                        read_only,
                        adjustment
                    }),
                    Err(E::InvalidAmount)
                ));
            }
        }
        for target in [-1.0, 2.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                resolve_slider_adjustment(SliderAdjustmentInput {
                    current: &current,
                    enabled,
                    read_only,
                    adjustment: A::SetValue(target)
                }),
                Err(E::InvalidValue(_))
            ));
        }
    }
}
#[test]
fn numeric_loss_is_diagnostic_not_an_invisible_change_or_false_endpoint() {
    for (current, adjustment) in [
        (value(-1.0, 1.0, -2.0f64.powi(-54)), A::Increase(1.0)),
        (value(-1.0, 1.0, 2.0f64.powi(-54)), A::Decrease(1.0)),
        (value(0.0, 2.0, 1.0), A::Increase(f64::from_bits(1))),
        (value(0.0, 2.0, 1.0), A::Decrease(f64::from_bits(1))),
        (
            value(0.0, 1.0, f64::from_bits(1.0f64.to_bits() - 1)),
            A::Increase(1e-16),
        ),
        (
            value(1.0, 2.0, f64::from_bits(1.0f64.to_bits() + 1)),
            A::Decrease(1.8e-16),
        ),
    ] {
        assert!(matches!(
            resolve_slider_adjustment(SliderAdjustmentInput {
                current: &current,
                enabled: true,
                read_only: false,
                adjustment
            }),
            Err(E::NumericRange)
        ));
    }
    let current = value(-f64::MAX, f64::MAX, -f64::MAX);
    let result = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &current,
        enabled: true,
        read_only: false,
        adjustment: A::Increase(f64::MAX),
    })
    .unwrap();
    assert_eq!(result.value().value().value(), 0.0);
    assert_eq!(result.value().progress(), 0.5);
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum CaseAdjustment {
    SetValue { value: f64 },
    Increase { amount: f64 },
    Decrease { amount: f64 },
    Minimum,
    Maximum,
}
impl From<CaseAdjustment> for A {
    fn from(value: CaseAdjustment) -> Self {
        match value {
            CaseAdjustment::SetValue { value } => Self::SetValue(value),
            CaseAdjustment::Increase { amount } => Self::Increase(amount),
            CaseAdjustment::Decrease { amount } => Self::Decrease(amount),
            CaseAdjustment::Minimum => Self::Minimum,
            CaseAdjustment::Maximum => Self::Maximum,
        }
    }
}
#[test]
fn public_vectors_verify_complete_results_and_diagnostic_categories() {
    let document: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-adjustment-cases.json"
    ))
    .unwrap();
    for case in document["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let current: SliderValue = serde_json::from_value(case["current"].clone()).unwrap();
        let current = resolve_slider_value(&current).unwrap();
        let adjustment: CaseAdjustment =
            serde_json::from_value(case["adjustment"].clone()).unwrap();
        let result = resolve_slider_adjustment(SliderAdjustmentInput {
            current: &current,
            enabled: case["enabled"].as_bool().unwrap(),
            read_only: case["readOnly"].as_bool().unwrap(),
            adjustment: adjustment.into(),
        });
        if let Some(error) = case["error"].as_str() {
            let actual = match result.expect_err(name) {
                E::InvalidValue(_) => "invalidValue",
                E::InvalidAmount => "invalidAmount",
                E::NumericRange => "numericRange",
                E::Value(_) => "value",
            };
            assert_eq!(actual, error, "{name}");
        } else {
            let actual = serde_json::to_value(result.expect(name)).unwrap();
            assert_eq!(actual, case["expected"], "{name}");
        }
    }
}
