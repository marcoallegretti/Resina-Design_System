use resina_model::{
    MaterialFamily, SliderAppearance, SliderPart, SliderPhase, StateSet, resolve_slider_phase,
};
use serde_json::{Value, json};

fn appearance_source() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/appearance/slider-appearance.json"
    ))
    .unwrap()
}

#[test]
fn every_part_family_and_phase_selects_its_complete_authored_response() {
    let source = appearance_source();
    let appearance: SliderAppearance = serde_json::from_value(source.clone()).unwrap();
    assert_eq!(serde_json::to_value(&appearance).unwrap(), source);
    for (part_name, part) in [("track", SliderPart::Track), ("thumb", SliderPart::Thumb)] {
        for (family_name, family) in [
            ("cast", MaterialFamily::Cast),
            ("frost", MaterialFamily::Frost),
            ("elastomer", MaterialFamily::Elastomer),
        ] {
            for (phase_name, phase) in [
                ("hover", SliderPhase::Hover),
                ("pressed", SliderPhase::Pressed),
                ("dragging", SliderPhase::Dragging),
                ("disabled", SliderPhase::Disabled),
                ("readOnly", SliderPhase::ReadOnly),
                ("readOnlyHover", SliderPhase::ReadOnlyHover),
            ] {
                assert_eq!(
                    serde_json::to_value(appearance.response_for(part, family, phase).unwrap())
                        .unwrap(),
                    source[part_name][family_name][phase_name],
                    "{part_name}/{family_name}/{phase_name}"
                );
            }
            assert_eq!(
                serde_json::to_value(
                    appearance
                        .response_for(part, family, SliderPhase::Rest)
                        .unwrap()
                )
                .unwrap(),
                json!({"bodyMix": 0.0, "depthScale": 1.0})
            );
        }
        for phase in [
            SliderPhase::Rest,
            SliderPhase::Hover,
            SliderPhase::Pressed,
            SliderPhase::Dragging,
            SliderPhase::Disabled,
            SliderPhase::ReadOnly,
            SliderPhase::ReadOnlyHover,
        ] {
            assert_eq!(
                appearance
                    .response_for(part, MaterialFamily::Gel, phase)
                    .unwrap_err(),
                "Gel cannot be a persistent slider part material"
            );
        }
    }
}

#[test]
fn invalid_unselected_profiles_cannot_enter_the_model() {
    let source = appearance_source();
    for part in ["track", "thumb"] {
        let mut missing = source.clone();
        missing.as_object_mut().unwrap().remove(part);
        assert!(serde_json::from_value::<SliderAppearance>(missing).is_err());
        for family in ["cast", "frost", "elastomer"] {
            let mut missing = source.clone();
            missing[part].as_object_mut().unwrap().remove(family);
            assert!(serde_json::from_value::<SliderAppearance>(missing).is_err());
            for phase in [
                "hover",
                "pressed",
                "dragging",
                "disabled",
                "readOnly",
                "readOnlyHover",
            ] {
                let mut missing = source.clone();
                missing[part][family].as_object_mut().unwrap().remove(phase);
                assert!(serde_json::from_value::<SliderAppearance>(missing).is_err());
                for (key, value) in [
                    ("bodyMix", json!(-1.01)),
                    ("bodyMix", json!(1.01)),
                    ("depthScale", json!(-0.01)),
                    ("depthScale", json!(1.01)),
                    ("bodyMix", json!(null)),
                    ("depthScale", json!("NaN")),
                    ("unknown", json!(0)),
                ] {
                    let mut invalid = source.clone();
                    invalid[part][family][phase][key] = value;
                    assert!(
                        serde_json::from_value::<SliderAppearance>(invalid).is_err(),
                        "{part}/{family}/{phase}/{key}"
                    );
                }
            }
        }
    }
    for pointer in ["", "/track", "/thumb", "/track/cast", "/thumb/elastomer"] {
        let mut invalid = source.clone();
        invalid
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(0));
        assert!(serde_json::from_value::<SliderAppearance>(invalid).is_err());
    }
    let mut invalid = source;
    invalid["schemaVersion"] = json!("0.2.0");
    assert!(serde_json::from_value::<SliderAppearance>(invalid).is_err());
}

#[test]
fn public_phase_cases_check_coherence_precedence_and_independent_focus() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-phase-cases.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 133);
    for case in cases {
        let states: StateSet = serde_json::from_value(case["states"].clone()).unwrap();
        let before = serde_json::to_value(&states).unwrap();
        let result = resolve_slider_phase(&states, case["readOnly"].as_bool().unwrap());
        if let Some(error) = case["error"].as_str() {
            assert_eq!(result.unwrap_err(), error, "{}", case["name"]);
        } else {
            assert_eq!(
                serde_json::to_value(result.unwrap()).unwrap(),
                case["expected"],
                "{}",
                case["name"]
            );
        }
        assert_eq!(serde_json::to_value(&states).unwrap(), before);
    }
}
