use resina_model::SliderAppearance;
use serde_json::{Value, json};

fn appearance() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/appearance/slider-appearance.json"
    ))
    .unwrap()
}

#[test]
fn appearance_requires_a_named_document() {
    let valid = appearance();
    let result: SliderAppearance = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(serde_json::to_value(result).unwrap(), valid);
    let positional = json!([valid["schemaVersion"], valid["track"], valid["thumb"]]);
    assert!(serde_json::from_value::<SliderAppearance>(positional.clone()).is_err());
    assert!(serde_json::from_str::<SliderAppearance>(&positional.to_string()).is_err());
}

#[test]
fn every_part_family_and_response_requires_named_members() {
    let valid = appearance();
    let families = ["cast", "frost", "elastomer"];
    let phases = [
        "hover",
        "pressed",
        "dragging",
        "disabled",
        "readOnly",
        "readOnlyHover",
    ];
    let reject = |document: Value| {
        assert!(serde_json::from_value::<SliderAppearance>(document.clone()).is_err());
        assert!(serde_json::from_str::<SliderAppearance>(&document.to_string()).is_err());
    };
    for part in ["track", "thumb"] {
        let mut document = valid.clone();
        document[part] = json!(families.map(|family| valid[part][family].clone()));
        reject(document);
        for family in families {
            let mut document = valid.clone();
            document[part][family] = json!(phases.map(|phase| valid[part][family][phase].clone()));
            reject(document);
            for phase in phases {
                let mut document = valid.clone();
                let response = &valid[part][family][phase];
                document[part][family][phase] =
                    json!([response["bodyMix"], response["depthScale"]]);
                reject(document);
            }
        }
    }
}

#[test]
fn public_vectors_agree_with_model_admission() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/appearance/slider-appearance-vectors.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let document = &case["document"];
        let raw = serde_json::from_str::<SliderAppearance>(&document.to_string());
        let value = serde_json::from_value::<SliderAppearance>(document.clone());
        if let Some(expected) = case.get("expected") {
            assert_eq!(
                serde_json::to_value(raw.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
            assert_eq!(
                serde_json::to_value(value.unwrap()).unwrap(),
                *expected,
                "{}",
                case["name"]
            );
        } else {
            assert!(case["error"].is_string());
            assert!(raw.is_err(), "{}", case["name"]);
            assert!(value.is_err(), "{}", case["name"]);
        }
    }
}

fn render_record(document: &Value, segments: &[&str], record: &str) -> String {
    if segments.is_empty() {
        return record.into();
    }
    let members: Vec<_> = document
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            let value = if name == segments[0] {
                render_record(value, &segments[1..], record)
            } else {
                value.to_string()
            };
            format!("{}:{value}", serde_json::to_string(name).unwrap())
        })
        .collect();
    format!("{{{}}}", members.join(","))
}

#[test]
fn every_record_preserves_strict_named_members() {
    let baseline = appearance();
    let expected: SliderAppearance = serde_json::from_value(baseline.clone()).unwrap();
    let mut pointers = vec![String::new()];
    for part in ["track", "thumb"] {
        pointers.push(format!("/{part}"));
        for family in ["cast", "frost", "elastomer"] {
            pointers.push(format!("/{part}/{family}"));
            for phase in [
                "hover",
                "pressed",
                "dragging",
                "disabled",
                "readOnly",
                "readOnlyHover",
            ] {
                pointers.push(format!("/{part}/{family}/{phase}"));
            }
        }
    }
    for pointer in pointers {
        let object = baseline.pointer(&pointer).unwrap();
        let segments: Vec<_> = if pointer.is_empty() {
            vec![]
        } else {
            pointer[1..].split('/').collect()
        };
        let source = |members: &[String]| {
            render_record(&baseline, &segments, &format!("{{{}}}", members.join(",")))
        };
        let members: Vec<_> = object
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
        let mut reordered = original.clone();
        reordered.reverse();
        assert_eq!(
            serde_json::from_str::<SliderAppearance>(&source(&reordered)).unwrap(),
            expected
        );
        for (index, (name, member)) in members.iter().enumerate() {
            let value = &object[name];
            let escaped = format!("\"\\u{:04x}{}\":{value}", name.as_bytes()[0], &name[1..]);
            let mut changed = original.clone();
            changed[index] = escaped.clone();
            assert_eq!(
                serde_json::from_str::<SliderAppearance>(&source(&changed)).unwrap(),
                expected
            );
            for duplicate in [member, &escaped] {
                changed[index] = format!("{member},{duplicate}");
                assert!(
                    serde_json::from_str::<SliderAppearance>(&source(&changed)).is_err(),
                    "duplicate {pointer}/{name}"
                );
            }
            changed = original.clone();
            changed.remove(index);
            assert!(
                serde_json::from_str::<SliderAppearance>(&source(&changed)).is_err(),
                "missing {pointer}/{name}"
            );
            changed = original.clone();
            changed[index] = format!("{}:null", serde_json::to_string(name).unwrap());
            assert!(
                serde_json::from_str::<SliderAppearance>(&source(&changed)).is_err(),
                "null {pointer}/{name}"
            );
        }
        let mut unknown = original.clone();
        unknown.push("\"extra\":false".into());
        assert!(
            serde_json::from_str::<SliderAppearance>(&source(&unknown)).is_err(),
            "unknown {pointer}"
        );
        for record in ["null", "true", "1", "\"record\"", "[]"] {
            let source = render_record(&baseline, &segments, record);
            assert!(
                serde_json::from_str::<SliderAppearance>(&source).is_err(),
                "{pointer} {record}"
            );
        }
    }
}
