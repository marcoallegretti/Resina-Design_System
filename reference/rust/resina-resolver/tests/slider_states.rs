use resina_model::SliderValue;
use resina_resolver::{
    SliderEditSession, SliderKey, SliderKeyInput, SliderKeyPolicy, SliderKeySteps,
    SliderPointerState, SliderPresentation, SliderStatesError, SliderStatesInput, SliderStops,
    SliderTieBreak, SliderValuePolicy, resolve_slider_key, resolve_slider_states,
    resolve_slider_value,
};
use serde_json::Value;

#[test]
fn public_idle_cases_verify_complete_states_and_diagnostics() {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-states-cases.json"
    ))
    .unwrap();
    let value = resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 0.0).unwrap()).unwrap();
    let presentation =
        SliderPresentation::try_new(&value, "r0", &SliderValuePolicy::Continuous, None).unwrap();
    let pointer = SliderPointerState::idle();
    let mut checked = 0;
    for case in cases["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["pointerTrace"].is_null())
    {
        let result = resolve_slider_states(SliderStatesInput {
            presentation: &presentation,
            pointer: &pointer,
            enabled: case["enabled"].as_bool().unwrap(),
            read_only: case["readOnly"].as_bool().unwrap(),
            focused: case["focused"].as_bool().unwrap(),
            hovered: case["hovered"].as_bool().unwrap(),
            key_pressed: case["keyPressed"].as_bool().unwrap(),
        });
        if let Some(error) = case["error"].as_str() {
            let actual = match result.unwrap_err() {
                SliderStatesError::UnavailableHold => "unavailableHold",
                SliderStatesError::UnfocusedKeyHold => "unfocusedKeyHold",
                error => panic!("unexpected state error: {error}"),
            };
            assert_eq!(actual, error, "{}", case["name"]);
        } else {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                case["expected"],
                "{}",
                case["name"]
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 32);
}

#[test]
fn unrelated_edit_cannot_claim_pointer_feedback_even_with_unchanged_visible_value() {
    let value = resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 0.0).unwrap()).unwrap();
    let policy = SliderValuePolicy::Continuous;
    let edit = SliderEditSession::begin(&value, "r0", &policy).unwrap();
    let presentation = SliderPresentation::try_new(&value, "r0", &policy, Some(&edit)).unwrap();
    let pointer = SliderPointerState::idle();
    assert_eq!(presentation.visible(), presentation.committed());
    assert!(matches!(
        resolve_slider_states(SliderStatesInput {
            presentation: &presentation,
            pointer: &pointer,
            enabled: false,
            read_only: true,
            focused: false,
            hovered: false,
            key_pressed: false,
        }),
        Err(SliderStatesError::IncoherentPresentation)
    ));
    assert!(presentation.editing());
    assert!(pointer.edit().is_none());
}

#[test]
fn endpoint_key_noop_keeps_press_feedback_until_key_release() {
    let value = resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 30.0).unwrap()).unwrap();
    let pointer = SliderPointerState::idle();
    for (policy, steps) in [
        (
            SliderValuePolicy::Continuous,
            SliderKeySteps::Continuous {
                step: 2.5,
                page: None,
            },
        ),
        (
            SliderValuePolicy::Stops {
                stops: SliderStops::try_new(-10.0, 30.0, &[-10.0, 0.0, 30.0]).unwrap(),
                tie_break: SliderTieBreak::Lower,
            },
            SliderKeySteps::Stops {
                step: 1,
                page: None,
            },
        ),
    ] {
        let key_policy = SliderKeyPolicy::try_new(steps, true, true).unwrap();
        let key = resolve_slider_key(SliderKeyInput {
            current: &value,
            value_policy: &policy,
            key_policy: &key_policy,
            key: SliderKey::ArrowRight,
            enabled: true,
            read_only: false,
            focused: true,
        })
        .unwrap();
        assert!(key.commit().unwrap().accepted());
        assert!(!key.commit().unwrap().changed());
        let presentation = SliderPresentation::try_new(key.value(), "r0", &policy, None).unwrap();
        for (key_pressed, signals) in [
            (true, vec!["focused", "pressed"]),
            (false, vec!["rest", "focused"]),
        ] {
            let states = resolve_slider_states(SliderStatesInput {
                presentation: &presentation,
                pointer: &pointer,
                enabled: true,
                read_only: false,
                focused: true,
                hovered: false,
                key_pressed,
            })
            .unwrap();
            assert_eq!(
                serde_json::to_value(states).unwrap(),
                serde_json::json!({"schemaVersion":"0.1.0", "states":signals})
            );
            assert_eq!(presentation.visible(), &value);
        }
    }
}
