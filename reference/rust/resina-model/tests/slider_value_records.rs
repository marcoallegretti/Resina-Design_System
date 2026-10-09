use resina_model::SliderValue;
use serde_json::{Value, json};

fn named_value() -> Value {
    json!({"schemaVersion":"0.1.0","minimum":0.0,"maximum":100.0,"value":50.0})
}

#[test]
fn bounded_values_require_named_records() {
    for (minimum, maximum, value) in [
        (0.0, 100.0, 50.0),
        (-10.0, 10.0, -5.0),
        (0.0, 1.0, 0.0),
        (0.0, 1.0, 1.0),
    ] {
        let named =
            json!({"schemaVersion":"0.1.0","minimum":minimum,"maximum":maximum,"value":value});
        let expected = SliderValue::try_new(minimum, maximum, value).unwrap();
        assert_eq!(
            serde_json::from_value::<SliderValue>(named.clone()).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::from_str::<SliderValue>(&named.to_string()).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(expected).unwrap(), named);
        let positional = json!(["0.1.0", minimum, maximum, value]);
        assert!(serde_json::from_value::<SliderValue>(positional.clone()).is_err());
        assert!(serde_json::from_str::<SliderValue>(&positional.to_string()).is_err());
    }
}

#[test]
fn named_members_remain_strict_and_order_independent() {
    let baseline = named_value();
    let expected: SliderValue = serde_json::from_value(baseline.clone()).unwrap();
    let members: Vec<_> = baseline
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                format!("{}:{value}", serde_json::to_string(name).unwrap()),
            )
        })
        .collect();
    let original: Vec<_> = members.iter().map(|(_, member)| member.clone()).collect();
    let source = |members: &[String]| format!("{{{}}}", members.join(","));
    let mut reordered = original.clone();
    reordered.reverse();
    assert_eq!(
        serde_json::from_str::<SliderValue>(&source(&reordered)).unwrap(),
        expected
    );
    for (index, (name, member)) in members.iter().enumerate() {
        let escaped = format!(
            "\"\\u{:04x}{}\":{}",
            name.as_bytes()[0],
            &name[1..],
            baseline[name]
        );
        let mut changed = original.clone();
        changed[index] = escaped.clone();
        assert_eq!(
            serde_json::from_str::<SliderValue>(&source(&changed)).unwrap(),
            expected
        );
        for duplicate in [member, &escaped] {
            changed[index] = format!("{member},{duplicate}");
            assert!(serde_json::from_str::<SliderValue>(&source(&changed)).is_err());
        }
        changed = original.clone();
        changed.remove(index);
        assert!(serde_json::from_str::<SliderValue>(&source(&changed)).is_err());
        changed = original.clone();
        changed[index] = format!("{}:null", serde_json::to_string(name).unwrap());
        assert!(serde_json::from_str::<SliderValue>(&source(&changed)).is_err());
    }
    let mut unknown = original;
    unknown.push("\"extra\":false".into());
    assert!(serde_json::from_str::<SliderValue>(&source(&unknown)).is_err());
    for source in ["null", "true", "1", "\"record\"", "[]"] {
        assert!(serde_json::from_str::<SliderValue>(source).is_err());
    }
}

#[test]
fn numeric_and_version_constraints_are_preserved() {
    for (field, value) in [
        ("schemaVersion", json!("9.9.9")),
        ("schemaVersion", json!(1)),
        ("minimum", json!(100.0)),
        ("maximum", json!(0.0)),
        ("value", json!(-0.01)),
        ("value", json!(100.01)),
    ] {
        let mut document = named_value();
        document[field] = value;
        assert!(serde_json::from_value::<SliderValue>(document.clone()).is_err());
        assert!(serde_json::from_str::<SliderValue>(&document.to_string()).is_err());
    }
    for field in ["minimum", "maximum", "value"] {
        for value in [json!("1"), json!(true), json!([]), json!({}), json!(null)] {
            let mut document = named_value();
            document[field] = value;
            assert!(serde_json::from_value::<SliderValue>(document.clone()).is_err());
            assert!(serde_json::from_str::<SliderValue>(&document.to_string()).is_err());
        }
        let raw = named_value()
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| {
                format!(
                    "{}:{}",
                    serde_json::to_string(name).unwrap(),
                    if name == field {
                        "1e400".into()
                    } else {
                        value.to_string()
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        assert!(serde_json::from_str::<SliderValue>(&format!("{{{raw}}}")).is_err());
    }
    for source in [
        r#"{"schemaVersion":"0.1.0","minimum":0,"maximum":9007199254740993,"value":0}"#,
        r#"{"schemaVersion":"0.1.0","minimum":-9007199254740993,"maximum":0,"value":0}"#,
        r#"{"schemaVersion":"0.1.0","minimum":0,"maximum":18014398509481984,"value":9007199254740993}"#,
    ] {
        assert!(serde_json::from_str::<SliderValue>(source).is_err());
        assert!(
            serde_json::from_value::<SliderValue>(serde_json::from_str::<Value>(source).unwrap())
                .is_err()
        );
    }
    let exact: SliderValue = serde_json::from_str(r#"{"schemaVersion":"0.1.0","minimum":0,"maximum":18014398509481984,"value":9007199254740992}"#).unwrap();
    assert_eq!(exact.maximum(), 18014398509481984.0);
    assert_eq!(exact.value(), 9007199254740992.0);
}

#[test]
fn public_positional_request_cannot_enter_the_checked_model() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-value-cases.json"
    ))
    .unwrap();
    let case = cases
        .iter()
        .find(|case| case["name"] == "positional value record")
        .unwrap();
    assert_eq!(case["requestSchemaValid"], false);
    assert!(serde_json::from_value::<SliderValue>(case["request"].clone()).is_err());
    assert!(serde_json::from_str::<SliderValue>(&case["request"].to_string()).is_err());
}
