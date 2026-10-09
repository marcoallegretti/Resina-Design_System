use resina_model::{CommandAppearance, CommandPhase, CommandResponse, MaterialFamily};
use serde_json::{Value, json};

fn appearance() -> Value {
    serde_json::from_str(include_str!(
        "../../../../definitions/command-appearance-light.json"
    ))
    .unwrap()
}

#[test]
fn response_requires_named_members() {
    let valid = json!({"bodyMix":0.125,"depthScale":0.5});
    let response: CommandResponse = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(response.body_mix(), 0.125);
    assert_eq!(response.depth_scale(), 0.5);
    assert_eq!(serde_json::to_value(response).unwrap(), valid);
    for source in ["[0.125,0.5]", "[]", "null", "true", "1", "\"response\""] {
        assert!(
            serde_json::from_str::<CommandResponse>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn appearance_requires_a_named_document() {
    for source in [
        include_str!("../../../../definitions/command-appearance-light.json"),
        include_str!("../../../../definitions/command-appearance-dark.json"),
    ] {
        let valid: Value = serde_json::from_str(source).unwrap();
        let result: CommandAppearance = serde_json::from_value(valid.clone()).unwrap();
        let serialized = serde_json::to_value(&result).unwrap();
        assert!(serialized.is_object());
        assert_eq!(
            serde_json::from_value::<CommandAppearance>(serialized).unwrap(),
            result
        );
        assert!(
            serde_json::from_value::<CommandAppearance>(json!([
                valid["schemaVersion"],
                valid["profiles"]
            ]))
            .is_err()
        );
    }
}

#[test]
fn every_family_and_phase_requires_named_members() {
    let valid = appearance();
    let families = ["cast", "frost", "elastomer"];
    let phases = ["hover", "pressed", "disabled"];
    let mut positional = valid.clone();
    positional["profiles"] = json!(families.map(|family| valid["profiles"][family].clone()));
    assert!(serde_json::from_value::<CommandAppearance>(positional).is_err());
    for family in families {
        let mut document = valid.clone();
        document["profiles"][family] =
            json!(phases.map(|phase| valid["profiles"][family][phase].clone()));
        assert!(
            serde_json::from_value::<CommandAppearance>(document).is_err(),
            "{family}"
        );
        for phase in phases {
            let mut document = valid.clone();
            let response = &valid["profiles"][family][phase];
            document["profiles"][family][phase] =
                json!([response["bodyMix"], response["depthScale"]]);
            assert!(
                serde_json::from_value::<CommandAppearance>(document).is_err(),
                "{family}/{phase}"
            );
        }
    }
}

#[test]
fn response_validation_and_lookup_preserve_the_contract() {
    for (mix, depth) in [(-1.0, 0.0), (0.0, 1.0), (1.0, 0.5)] {
        let response: CommandResponse =
            serde_json::from_value(json!({"bodyMix":mix,"depthScale":depth})).unwrap();
        assert_eq!(response, CommandResponse::try_new(mix, depth).unwrap());
    }
    for (mix, depth) in [(-1.01, 0.5), (1.01, 0.5), (0.0, -0.01), (0.0, 1.01)] {
        assert!(
            serde_json::from_value::<CommandResponse>(json!({"bodyMix":mix,"depthScale":depth}))
                .is_err()
        );
    }
    let valid = appearance();
    let model: CommandAppearance = serde_json::from_value(valid.clone()).unwrap();
    for (name, family) in [
        ("cast", MaterialFamily::Cast),
        ("frost", MaterialFamily::Frost),
        ("elastomer", MaterialFamily::Elastomer),
    ] {
        for (name_phase, phase) in [
            ("hover", CommandPhase::Hover),
            ("pressed", CommandPhase::Pressed),
            ("disabled", CommandPhase::Disabled),
        ] {
            let response = model.response_for(family, phase).unwrap();
            assert_eq!(
                response.body_mix(),
                valid["profiles"][name][name_phase]["bodyMix"]
                    .as_f64()
                    .unwrap()
            );
            assert_eq!(
                response.depth_scale(),
                valid["profiles"][name][name_phase]["depthScale"]
                    .as_f64()
                    .unwrap()
            );
        }
        assert_eq!(
            model.response_for(family, CommandPhase::Rest).unwrap(),
            CommandResponse::try_new(0.0, 1.0).unwrap()
        );
    }
    assert!(
        model
            .response_for(MaterialFamily::Gel, CommandPhase::Rest)
            .is_err()
    );
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
    let expected: CommandAppearance = serde_json::from_value(baseline.clone()).unwrap();
    let mut pointers = vec![String::new(), "/profiles".into()];
    for family in ["cast", "frost", "elastomer"] {
        pointers.push(format!("/profiles/{family}"));
        for phase in ["hover", "pressed", "disabled"] {
            pointers.push(format!("/profiles/{family}/{phase}"));
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
            serde_json::from_str::<CommandAppearance>(&source(&reordered)).unwrap(),
            expected
        );
        for (index, (name, member)) in members.iter().enumerate() {
            let value = &object[name];
            let escaped = format!("\"\\u{:04x}{}\":{value}", name.as_bytes()[0], &name[1..]);
            let mut changed = original.clone();
            changed[index] = escaped.clone();
            assert_eq!(
                serde_json::from_str::<CommandAppearance>(&source(&changed)).unwrap(),
                expected
            );
            for duplicate in [member, &escaped] {
                changed[index] = format!("{member},{duplicate}");
                assert!(
                    serde_json::from_str::<CommandAppearance>(&source(&changed)).is_err(),
                    "duplicate {pointer}/{name}"
                );
            }
            changed = original.clone();
            changed.remove(index);
            assert!(
                serde_json::from_str::<CommandAppearance>(&source(&changed)).is_err(),
                "missing {pointer}/{name}"
            );
            changed = original.clone();
            changed[index] = format!("{}:null", serde_json::to_string(name).unwrap());
            assert!(
                serde_json::from_str::<CommandAppearance>(&source(&changed)).is_err(),
                "null {pointer}/{name}"
            );
        }
        let mut unknown = original.clone();
        unknown.push("\"extra\":false".into());
        assert!(
            serde_json::from_str::<CommandAppearance>(&source(&unknown)).is_err(),
            "unknown {pointer}"
        );
        for record in ["null", "true", "1", "\"record\"", "[]"] {
            let source = render_record(&baseline, &segments, record);
            assert!(
                serde_json::from_str::<CommandAppearance>(&source).is_err(),
                "{pointer} {record}"
            );
        }
    }
}

#[test]
fn raw_responses_and_versions_preserve_validation() {
    for source in [
        r#"{"bodyMix":0.125,"bodyMix":0.125,"depthScale":0.5}"#,
        r#"{"bodyMix":0.125,"\u0062odyMix":0.125,"depthScale":0.5}"#,
        r#"{"bodyMix":0.125,"depthScale":0.5,"\u0064epthScale":0.5}"#,
        r#"{"bodyMix":1e400,"depthScale":0.5}"#,
        r#"{"bodyMix":0.125,"depthScale":1e400}"#,
        r#"{"bodyMix":0.125,"depthScale":0.5,"extra":false}"#,
    ] {
        assert!(
            serde_json::from_str::<CommandResponse>(source).is_err(),
            "{source}"
        );
    }
    let response: CommandResponse =
        serde_json::from_str(r#"{"depthScale":0.5,"\u0062odyMix":0.125}"#).unwrap();
    assert_eq!(response, CommandResponse::try_new(0.125, 0.5).unwrap());
    for version in [json!("9.9.9"), json!(null), json!(1)] {
        let mut document = appearance();
        document["schemaVersion"] = version;
        assert!(serde_json::from_value::<CommandAppearance>(document).is_err());
    }
}
