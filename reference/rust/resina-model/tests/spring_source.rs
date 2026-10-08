use resina_model::{SpringDynamics, SpringParameters, SpringState};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn record<T: DeserializeOwned + Serialize>(source: &str, fields: &[&str]) {
    let baseline: Value = serde_json::from_str(source).unwrap();
    let expected = serde_json::to_value(serde_json::from_str::<T>(source).unwrap()).unwrap();
    assert_eq!(expected, baseline);
    let positional = Value::Array(
        fields
            .iter()
            .map(|field| baseline[*field].clone())
            .collect(),
    );
    for invalid in [
        positional,
        json!([]),
        Value::Null,
        json!(true),
        json!(1),
        json!("record"),
    ] {
        assert!(serde_json::from_str::<T>(&invalid.to_string()).is_err());
        assert!(serde_json::from_value::<T>(invalid).is_err());
    }
    let members: Vec<_> = fields
        .iter()
        .map(|field| format!("\"{field}\":{}", baseline[*field]))
        .collect();
    for (index, field) in fields.iter().enumerate() {
        let escaped = format!(
            "\"\\u{:04x}{}\":{}",
            field.as_bytes()[0],
            &field[1..],
            baseline[*field]
        );
        let mut changed = members.clone();
        changed[index] = escaped.clone();
        let source = format!("{{{}}}", changed.join(","));
        assert_eq!(
            serde_json::to_value(serde_json::from_str::<T>(&source).unwrap()).unwrap(),
            expected
        );
        for duplicate in [&members[index], &escaped] {
            changed[index] = format!("{},{}", members[index], duplicate);
            let source = format!("{{{}}}", changed.join(","));
            let error = serde_json::from_str::<T>(&source).err().unwrap();
            assert!(error.to_string().contains("duplicate"), "{error}");
        }
    }
}

#[test]
fn spring_parameters_require_objects_without_changing_members() {
    record::<SpringParameters>(
        r#"{"schemaVersion":"0.1.0","mass":1.0,"stiffness":4.0,"damping":0.0,"initialVelocity":-2.0,"positionThreshold":0.001,"velocityThreshold":0.001}"#,
        &[
            "schemaVersion",
            "mass",
            "stiffness",
            "damping",
            "initialVelocity",
            "positionThreshold",
            "velocityThreshold",
        ],
    );
}

#[test]
fn spring_dynamics_require_objects_without_changing_members() {
    record::<SpringDynamics>(
        r#"{"schemaVersion":"0.1.0","mass":1.0,"stiffness":4.0,"damping":0.0,"positionThreshold":0.001,"velocityThreshold":0.001}"#,
        &[
            "schemaVersion",
            "mass",
            "stiffness",
            "damping",
            "positionThreshold",
            "velocityThreshold",
        ],
    );
}

#[test]
fn spring_states_require_objects_without_changing_members() {
    record::<SpringState>(
        r#"{"position":3.0,"velocity":-2.0}"#,
        &["position", "velocity"],
    );
}
