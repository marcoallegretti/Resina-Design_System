use resina_model::{CornerRadius, LogicalCornerRadii, PhysicalVector};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

fn radii() -> Value {
    json!({
        "topStart": {"x": 3.0, "y": 4.0},
        "topEnd": {"x": 5.0, "y": 6.0},
        "bottomEnd": {"x": 7.0, "y": 8.0},
        "bottomStart": {"x": 9.0, "y": 10.0}
    })
}

fn accepted<T: DeserializeOwned + Serialize + Debug>(source: &str, expected: &Value) {
    for result in [
        serde_json::from_str::<T>(source),
        serde_json::from_value::<T>(serde_json::from_str(source).unwrap()),
    ] {
        assert_eq!(serde_json::to_value(result.unwrap()).unwrap(), *expected);
    }
}

fn rejected<T: DeserializeOwned + Debug>(document: &Value, diagnostic: &str) {
    for result in [
        serde_json::from_value::<T>(document.clone()),
        serde_json::from_str::<T>(&document.to_string()),
    ] {
        let error = result.unwrap_err().to_string();
        assert!(error.contains(diagnostic), "{document}: {error}");
    }
}

fn record<T: DeserializeOwned + Serialize + Debug>(
    document: Value,
    fields: &[&str],
    diagnostic: &str,
) {
    accepted::<T>(&document.to_string(), &document);
    let positional: Vec<_> = fields
        .iter()
        .map(|field| document[*field].clone())
        .collect();
    let mut extended = positional.clone();
    extended.push(json!(false));
    for shape in [
        json!(positional),
        json!(extended),
        json!([]),
        json!(null),
        json!(true),
        json!(1),
        json!("record"),
    ] {
        rejected::<T>(&shape, diagnostic);
    }
    let members: Vec<_> = fields
        .iter()
        .map(|field| format!("\"{field}\":{}", document[*field]))
        .collect();
    let source = |members: &[String]| format!("{{{}}}", members.join(","));
    let mut reversed = members.clone();
    reversed.reverse();
    accepted::<T>(&source(&reversed), &document);
    for (index, field) in fields.iter().enumerate() {
        let escaped = format!(
            "\"\\u{:04x}{}\":{}",
            field.as_bytes()[0],
            &field[1..],
            document[*field]
        );
        let mut changed = members.clone();
        changed[index] = escaped.clone();
        accepted::<T>(&source(&changed), &document);
        for duplicate in [&members[index], &escaped] {
            changed[index] = format!("{},{}", members[index], duplicate);
            let error = serde_json::from_str::<T>(&source(&changed)).unwrap_err();
            assert!(error.to_string().contains("duplicate field"), "{error}");
        }
        let mut missing = document.clone();
        missing.as_object_mut().unwrap().remove(*field);
        rejected::<T>(&missing, "missing field");
        let mut wrong = document.clone();
        wrong[*field] = json!(null);
        rejected::<T>(&wrong, "invalid type");
    }
    let mut unknown = document;
    unknown["extra"] = json!(false);
    rejected::<T>(&unknown, "unknown field");
}

#[test]
fn physical_vectors_require_named_objects() {
    record::<PhysicalVector>(
        json!({"x": 3.0, "y": -4.0}),
        &["x", "y"],
        "physical vector object",
    );
}

#[test]
fn corner_radii_require_named_objects() {
    record::<CornerRadius>(
        json!({"x": 3.0, "y": 4.0}),
        &["x", "y"],
        "corner radius object",
    );
}

#[test]
fn logical_radii_require_named_objects_at_both_levels() {
    record::<LogicalCornerRadii>(
        radii(),
        &["topStart", "topEnd", "bottomEnd", "bottomStart"],
        "logical corner radii object",
    );
    for field in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        for shape in [
            json!([3, 4]),
            json!([]),
            json!(null),
            json!(true),
            json!(1),
            json!("corner"),
        ] {
            let mut document = radii();
            document[field] = shape;
            rejected::<LogicalCornerRadii>(&document, "corner radius object");
        }
        let source = radii().to_string();
        for component in ["x", "y"] {
            let member = format!("\"{component}\":{}", radii()[field][component]);
            let corner = radii()[field].to_string();
            for duplicate in [
                component.to_string(),
                format!("\\u{:04x}", component.as_bytes()[0]),
            ] {
                let changed = corner.replace(
                    &member,
                    &format!("{member},\"{duplicate}\":{}", radii()[field][component]),
                );
                let changed = source.replace(
                    &format!("\"{field}\":{corner}"),
                    &format!("\"{field}\":{changed}"),
                );
                let error = serde_json::from_str::<LogicalCornerRadii>(&changed).unwrap_err();
                assert!(error.to_string().contains("duplicate field"), "{error}");
            }
        }
    }
}

#[test]
fn record_decoding_preserves_numerical_validation_ownership() {
    accepted::<CornerRadius>(r#"{"x":-3.0,"y":4.0}"#, &json!({"x": -3.0, "y": 4.0}));
    accepted::<PhysicalVector>(r#"{"x":-3.0,"y":4.0}"#, &json!({"x": -3.0, "y": 4.0}));
    for source in [r#"{"x":1e999,"y":4}"#, r#"{"x":3,"y":1e999}"#] {
        assert!(serde_json::from_str::<CornerRadius>(source).is_err());
        assert!(serde_json::from_str::<PhysicalVector>(source).is_err());
    }
}
