use resina_model::{MaterialFamily, OpaquePigmentProfile, OpaquePigmentProfiles};
use serde_json::{Value, json};

fn profiles() -> Value {
    serde_json::from_str(include_str!("../../../../definitions/tier0-pigment.json")).unwrap()
}

#[test]
fn pigment_profile_requires_named_members() {
    let valid = json!({"sideShade":0.125,"highlightLift":0.0625});
    let profile: OpaquePigmentProfile = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(profile.side_shade(), 0.125);
    assert_eq!(profile.highlight_lift(), 0.0625);
    assert_eq!(serde_json::to_value(profile).unwrap(), valid);
    for source in ["[0.125,0.0625]", "[]", "null", "true", "1", "\"pigment\""] {
        assert!(
            serde_json::from_str::<OpaquePigmentProfile>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn pigment_document_requires_a_named_object() {
    let valid = profiles();
    let result: OpaquePigmentProfiles = serde_json::from_value(valid.clone()).unwrap();
    let serialized = serde_json::to_value(&result).unwrap();
    assert!(serialized.is_object());
    assert_eq!(
        serde_json::from_value::<OpaquePigmentProfiles>(serialized).unwrap(),
        result
    );
    assert!(
        serde_json::from_value::<OpaquePigmentProfiles>(json!([
            valid["schemaVersion"],
            valid["profiles"]
        ]))
        .is_err()
    );
}

#[test]
fn every_family_pigment_requires_a_named_object() {
    let valid = profiles();
    let families = ["cast", "frost", "elastomer", "gel"];
    for family in families {
        let mut document = valid.clone();
        let profile = &document["profiles"][family];
        document["profiles"][family] = json!([profile["sideShade"], profile["highlightLift"]]);
        assert!(
            serde_json::from_value::<OpaquePigmentProfiles>(document).is_err(),
            "{family}"
        );
    }
    let mut document = valid.clone();
    document["profiles"] = json!(families.map(|family| valid["profiles"][family].clone()));
    assert!(serde_json::from_value::<OpaquePigmentProfiles>(document).is_err());
}

#[test]
fn coefficient_validation_remains_in_the_constructor() {
    for (shade, lift) in [(-0.1, 0.1), (1.0, 0.1), (0.1, -0.1), (0.1, 1.0)] {
        assert!(
            serde_json::from_value::<OpaquePigmentProfile>(
                json!({"sideShade":shade,"highlightLift":lift})
            )
            .is_err()
        );
    }
    let valid = json!({"sideShade":0.0,"highlightLift":0.0});
    let profile: OpaquePigmentProfile = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(profile.side_shade(), 0.0);
    assert_eq!(profile.highlight_lift(), 0.0);
    assert_eq!(serde_json::to_value(profile).unwrap(), valid);
    let model: OpaquePigmentProfiles = serde_json::from_value(profiles()).unwrap();
    assert_eq!(model.profile_for(MaterialFamily::Cast).side_shade(), 0.1);
}

#[test]
fn pigment_members_preserve_strict_decoding() {
    for source in [
        r#"{"highlightLift":0.25,"sideShade":0.75}"#,
        r#"{"\u0073ideShade":0.75,"highlightLift":0.25}"#,
    ] {
        let profile: OpaquePigmentProfile = serde_json::from_str(source).unwrap();
        assert_eq!(profile.side_shade(), 0.75);
        assert_eq!(profile.highlight_lift(), 0.25);
    }
    for source in [
        r#"{"sideShade":0.75,"sideShade":0.75,"highlightLift":0.25}"#,
        r#"{"sideShade":0.75,"\u0073ideShade":0.75,"highlightLift":0.25}"#,
        r#"{"sideShade":0.75,"highlightLift":0.25,"\u0068ighlightLift":0.25}"#,
        r#"{"sideShade":0.75}"#,
        r#"{"highlightLift":0.25}"#,
        r#"{"sideShade":null,"highlightLift":0.25}"#,
        r#"{"sideShade":0.75,"highlightLift":null}"#,
        r#"{"sideShade":0.75,"highlightLift":0.25,"extra":false}"#,
        r#"{"sideShade":1e400,"highlightLift":0.25}"#,
    ] {
        assert!(
            serde_json::from_str::<OpaquePigmentProfile>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn profile_document_members_preserve_strict_decoding() {
    let baseline = profiles();
    for pointer in ["", "/profiles"] {
        let object = if pointer.is_empty() {
            &baseline
        } else {
            &baseline["profiles"]
        };
        let members: Vec<_> = object
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.clone(), format!("\"{name}\":{value}")))
            .collect();
        let source = |members: &[String]| {
            let value = format!("{{{}}}", members.join(","));
            if pointer.is_empty() {
                value
            } else {
                let outer: Vec<_> = baseline
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(name, v)| {
                        if name == "profiles" {
                            format!("\"profiles\":{value}")
                        } else {
                            format!("\"{name}\":{v}")
                        }
                    })
                    .collect();
                format!("{{{}}}", outer.join(","))
            }
        };
        let original: Vec<_> = members.iter().map(|(_, member)| member.clone()).collect();
        let expected: OpaquePigmentProfiles = serde_json::from_value(baseline.clone()).unwrap();
        let mut reordered = original.clone();
        reordered.reverse();
        assert_eq!(
            serde_json::from_str::<OpaquePigmentProfiles>(&source(&reordered)).unwrap(),
            expected
        );
        for (index, (name, member)) in members.iter().enumerate() {
            let value = &object[name];
            let escaped = format!("\"\\u{:04x}{}\":{value}", name.as_bytes()[0], &name[1..]);
            let mut changed = original.clone();
            changed[index] = escaped.clone();
            assert_eq!(
                serde_json::from_str::<OpaquePigmentProfiles>(&source(&changed)).unwrap(),
                expected
            );
            for duplicate in [member, &escaped] {
                changed[index] = format!("{member},{duplicate}");
                assert!(
                    serde_json::from_str::<OpaquePigmentProfiles>(&source(&changed)).is_err(),
                    "{pointer}/{name}"
                );
            }
            changed = original.clone();
            changed.remove(index);
            assert!(
                serde_json::from_str::<OpaquePigmentProfiles>(&source(&changed)).is_err(),
                "missing {pointer}/{name}"
            );
            changed = original.clone();
            changed[index] = format!("\"{name}\":null");
            assert!(
                serde_json::from_str::<OpaquePigmentProfiles>(&source(&changed)).is_err(),
                "null {pointer}/{name}"
            );
        }
        let mut unknown = original.clone();
        unknown.push("\"extra\":false".into());
        assert!(serde_json::from_str::<OpaquePigmentProfiles>(&source(&unknown)).is_err());
    }
    let mut unsupported = baseline.clone();
    unsupported["schemaVersion"] = json!("9.9.9");
    assert!(serde_json::from_value::<OpaquePigmentProfiles>(unsupported).is_err());
}
