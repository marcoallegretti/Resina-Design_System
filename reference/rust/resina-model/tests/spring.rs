use resina_model::SpringParameters;
use serde_json::Value;

#[test]
fn invalid_coefficients_are_rejected_by_typed_and_json_inputs() {
    for index in 0..6 {
        let invalid = if index == 2 {
            vec![-1.0, f64::NAN, f64::INFINITY]
        } else if index == 3 {
            vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY]
        } else {
            vec![0.0, -1.0, f64::NAN, f64::INFINITY]
        };
        for value in invalid {
            let mut fields = [1.0, 1.0, 2.0, 0.0, 0.001, 0.001];
            fields[index] = value;
            assert!(
                SpringParameters::try_new(
                    fields[0], fields[1], fields[2], fields[3], fields[4], fields[5]
                )
                .is_err()
            );
        }
    }
    let valid = SpringParameters::try_new(1.0, 4.0, 0.0, -2.0, 0.001, 0.001).unwrap();
    let json = serde_json::to_value(&valid).unwrap();
    assert_eq!(
        serde_json::from_value::<SpringParameters>(json.clone()).unwrap(),
        valid
    );
    for key in [
        "schemaVersion",
        "mass",
        "stiffness",
        "damping",
        "initialVelocity",
        "positionThreshold",
        "velocityThreshold",
    ] {
        let mut missing = json.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<SpringParameters>(missing).is_err());
    }
    for (key, value) in [
        ("schemaVersion", Value::from("0.2.0")),
        ("unknown", Value::from(0)),
        ("mass", Value::from(0)),
        ("damping", Value::from(-1)),
    ] {
        let mut invalid = json.clone();
        invalid[key] = value;
        assert!(serde_json::from_value::<SpringParameters>(invalid).is_err());
    }
}

#[test]
fn dynamics_and_states_are_validated_without_borrowing_input() {
    use resina_model::{SpringDynamics, SpringState};
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(SpringState::try_new(invalid, 0.0).is_err());
        assert!(SpringState::try_new(0.0, invalid).is_err());
        assert!(SpringDynamics::try_new(invalid, 1.0, 0.0, 1e-6, 1e-6).is_err());
        assert!(SpringDynamics::try_new(1.0, invalid, 0.0, 1e-6, 1e-6).is_err());
        assert!(SpringDynamics::try_new(1.0, 1.0, invalid, 1e-6, 1e-6).is_err());
        assert!(SpringDynamics::try_new(1.0, 1.0, 0.0, invalid, 1e-6).is_err());
        assert!(SpringDynamics::try_new(1.0, 1.0, 0.0, 1e-6, invalid).is_err());
    }
    let source = serde_json::json!({"schemaVersion":"0.1.0","mass":1.0,"stiffness":4.0,"damping":4.0,"positionThreshold":1e-6,"velocityThreshold":1e-6});
    let dynamics: SpringDynamics = serde_json::from_str(&source.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&dynamics).unwrap(), source);
    for version in [serde_json::json!("0.2.0"), serde_json::Value::Null] {
        let mut invalid = source.clone();
        invalid["schemaVersion"] = version;
        assert!(serde_json::from_value::<SpringDynamics>(invalid).is_err());
    }
    let mut missing = source.clone();
    missing.as_object_mut().unwrap().remove("schemaVersion");
    assert!(serde_json::from_value::<SpringDynamics>(missing).is_err());
    let mut extra = source;
    extra["initialVelocity"] = serde_json::json!(0);
    assert!(serde_json::from_value::<SpringDynamics>(extra).is_err());
}
