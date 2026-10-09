use resina_model::{MaterialFamily, OpaqueSurfaceAppearance, SurfaceBandProfile};
use serde_json::{Value, json};

fn appearance() -> Value {
    serde_json::from_str(include_str!(
        "../../../../definitions/tier0-surface-appearance.json"
    ))
    .unwrap()
}

#[test]
fn band_profile_requires_named_members() {
    let valid = json!({"edgeWidth": 0.75, "highlightWidth": 0.25});
    let profile: SurfaceBandProfile = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(profile.edge_width(), 0.75);
    assert_eq!(profile.highlight_width(), 0.25);
    assert_eq!(serde_json::to_value(profile).unwrap(), valid);
    for source in ["[0.75,0.25]", "[]", "null", "true", "1", "\"band\""] {
        assert!(
            serde_json::from_str::<SurfaceBandProfile>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn appearance_requires_a_named_object() {
    let valid = appearance();
    let result: OpaqueSurfaceAppearance = serde_json::from_value(valid.clone()).unwrap();
    let serialized = serde_json::to_value(&result).unwrap();
    assert!(serialized.is_object());
    assert_eq!(
        serde_json::from_value::<OpaqueSurfaceAppearance>(serialized).unwrap(),
        result
    );
    let positional: Vec<_> = [
        "schemaVersion",
        "shapeAssignments",
        "depthAssignments",
        "pigmentProfiles",
        "bands",
        "keyLight",
    ]
    .iter()
    .map(|field| valid[*field].clone())
    .collect();
    assert!(serde_json::from_value::<OpaqueSurfaceAppearance>(json!(positional)).is_err());
}

#[test]
fn every_family_band_requires_a_named_object() {
    let valid = appearance();
    let families = ["cast", "frost", "elastomer", "gel"];
    for family in families {
        let mut request = valid.clone();
        let band = &request["bands"][family];
        request["bands"][family] = json!([band["edgeWidth"], band["highlightWidth"]]);
        assert!(
            serde_json::from_value::<OpaqueSurfaceAppearance>(request).is_err(),
            "{family}"
        );
    }
    let mut request = valid.clone();
    request["bands"] = json!(families.map(|family| valid["bands"][family].clone()));
    assert!(serde_json::from_value::<OpaqueSurfaceAppearance>(request).is_err());
}

#[test]
fn band_width_validation_remains_in_the_constructor() {
    for (edge, highlight) in [(0.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
        assert!(
            serde_json::from_value::<SurfaceBandProfile>(json!({
                "edgeWidth":edge,"highlightWidth":highlight
            }))
            .is_err()
        );
    }
    let valid = json!({"edgeWidth":0.75,"highlightWidth":0.0});
    let band: SurfaceBandProfile = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(band.highlight_width(), 0.0);
    assert_eq!(serde_json::to_value(band).unwrap(), valid);
    let model: OpaqueSurfaceAppearance = serde_json::from_value(appearance()).unwrap();
    assert_eq!(model.bands_for(MaterialFamily::Cast).edge_width(), 1.0);
}

#[test]
fn band_members_preserve_strict_decoding() {
    for source in [
        r#"{"highlightWidth":0.25,"edgeWidth":0.75}"#,
        r#"{"\u0065dgeWidth":0.75,"highlightWidth":0.25}"#,
    ] {
        let band: SurfaceBandProfile = serde_json::from_str(source).unwrap();
        assert_eq!(band.edge_width(), 0.75);
        assert_eq!(band.highlight_width(), 0.25);
    }
    for source in [
        r#"{"edgeWidth":0.75,"edgeWidth":0.75,"highlightWidth":0.25}"#,
        r#"{"edgeWidth":0.75,"\u0065dgeWidth":0.75,"highlightWidth":0.25}"#,
        r#"{"edgeWidth":0.75,"highlightWidth":0.25,"\u0068ighlightWidth":0.25}"#,
        r#"{"edgeWidth":0.75}"#,
        r#"{"highlightWidth":0.25}"#,
        r#"{"edgeWidth":null,"highlightWidth":0.25}"#,
        r#"{"edgeWidth":0.75,"highlightWidth":null}"#,
        r#"{"edgeWidth":0.75,"highlightWidth":0.25,"extra":false}"#,
        r#"{"edgeWidth":1e400,"highlightWidth":0.25}"#,
    ] {
        assert!(
            serde_json::from_str::<SurfaceBandProfile>(source).is_err(),
            "{source}"
        );
    }
}

#[test]
fn appearance_members_preserve_strict_decoding() {
    let baseline = appearance();
    for pointer in ["", "/bands"] {
        let object = if pointer.is_empty() {
            &baseline
        } else {
            &baseline["bands"]
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
                        if name == "bands" {
                            format!("\"bands\":{value}")
                        } else {
                            format!("\"{name}\":{v}")
                        }
                    })
                    .collect();
                format!("{{{}}}", outer.join(","))
            }
        };
        let original: Vec<_> = members.iter().map(|(_, member)| member.clone()).collect();
        let expected: OpaqueSurfaceAppearance = serde_json::from_value(baseline.clone()).unwrap();
        let mut reordered = original.clone();
        reordered.reverse();
        assert_eq!(
            serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&reordered)).unwrap(),
            expected
        );
        for (index, (name, member)) in members.iter().enumerate() {
            let value = &object[name];
            let escaped = format!("\"\\u{:04x}{}\":{value}", name.as_bytes()[0], &name[1..]);
            let mut changed = original.clone();
            changed[index] = escaped.clone();
            assert_eq!(
                serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&changed)).unwrap(),
                expected
            );
            for duplicate in [member, &escaped] {
                changed[index] = format!("{member},{duplicate}");
                assert!(
                    serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&changed)).is_err(),
                    "{pointer}/{name}"
                );
            }
            changed = original.clone();
            changed.remove(index);
            assert!(
                serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&changed)).is_err(),
                "missing {pointer}/{name}"
            );
            changed = original.clone();
            changed[index] = format!("\"{name}\":null");
            assert!(
                serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&changed)).is_err(),
                "null {pointer}/{name}"
            );
        }
        let mut unknown = original.clone();
        unknown.push("\"extra\":false".into());
        assert!(serde_json::from_str::<OpaqueSurfaceAppearance>(&source(&unknown)).is_err());
    }
    let mut unsupported = baseline.clone();
    unsupported["schemaVersion"] = json!("9.9.9");
    assert!(serde_json::from_value::<OpaqueSurfaceAppearance>(unsupported).is_err());
}
