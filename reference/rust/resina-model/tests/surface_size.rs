use resina_model::SurfaceSize;
use serde_json::Value;

#[test]
fn surface_sizes_require_named_objects() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/geometry/surface-size-vectors.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 25);
    for case in cases {
        let document = &case["document"];
        for result in [
            serde_json::from_value::<SurfaceSize>(document.clone()),
            serde_json::from_str::<SurfaceSize>(&document.to_string()),
        ] {
            if case.get("expected").is_some() {
                assert_eq!(serde_json::to_value(result.unwrap()).unwrap(), *document);
            } else {
                let error = result.unwrap_err();
                assert!(
                    error.to_string().contains(case["error"].as_str().unwrap()),
                    "{}: {error}",
                    case["name"]
                );
            }
        }
    }
}

#[test]
fn size_members_keep_duplicate_and_escape_validation() {
    let source = r#"{"width":200,"height":80}"#;
    for (field, escaped, value) in [
        ("width", r"\u0077idth", 200),
        ("height", r"\u0068eight", 80),
    ] {
        let changed = source.replace(&format!("\"{field}\""), &format!("\"{escaped}\""));
        let size: SurfaceSize = serde_json::from_str(&changed).unwrap();
        assert_eq!(
            size,
            SurfaceSize {
                width: 200.0,
                height: 80.0
            }
        );
        let member = format!("\"{field}\":{value}");
        for duplicate in [field, escaped] {
            let changed = source.replace(&member, &format!("{member},\"{duplicate}\":{value}"));
            assert!(
                serde_json::from_str::<SurfaceSize>(&changed)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicate field")
            );
        }
    }
}

#[test]
fn size_decoding_preserves_numerical_validation_ownership() {
    let size: SurfaceSize = serde_json::from_str(r#"{"width":-1,"height":-2}"#).unwrap();
    assert_eq!(
        size,
        SurfaceSize {
            width: -1.0,
            height: -2.0
        }
    );
    for source in [
        r#"{"width":1e999,"height":80}"#,
        r#"{"width":200,"height":1e999}"#,
    ] {
        assert!(serde_json::from_str::<SurfaceSize>(source).is_err());
    }
}
