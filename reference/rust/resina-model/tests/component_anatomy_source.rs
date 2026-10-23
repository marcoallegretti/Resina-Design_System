use resina_model::{
    CommandAnatomy, CommandBody, CommandVariant, SelectablePart, SliderAnatomy, SliderPartBinding,
    ToggleAnatomy,
};
use serde_json::{Value, json};

#[test]
fn standalone_parts_require_objects() {
    let command: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/command.json"
    ))
    .unwrap();
    let body = &command["variants"]["standard"]["body"];
    let variant = &command["variants"]["standard"];
    assert!(serde_json::from_value::<CommandBody>(body.clone()).is_ok());
    assert!(serde_json::from_value::<CommandVariant>(variant.clone()).is_ok());
    let body_array = json!([
        body["materialRole"],
        body["colorRole"],
        body["shape"],
        body["elevation"],
        body["contentRole"]
    ]);
    let variant_array = json!([variant["body"], variant["label"]]);
    assert!(serde_json::from_str::<CommandBody>(&body_array.to_string()).is_err());
    assert!(serde_json::from_value::<CommandBody>(body_array).is_err());
    assert!(serde_json::from_str::<CommandVariant>(&variant_array.to_string()).is_err());
    assert!(serde_json::from_value::<CommandVariant>(variant_array).is_err());
    let toggle: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/toggle.json"
    ))
    .unwrap();
    let part = &toggle["track"];
    assert!(serde_json::from_value::<SelectablePart>(part.clone()).is_ok());
    let part_array = json!([
        part["materialRole"],
        part["colorRole"],
        part["checkedColorRole"],
        part["shape"],
        part["elevation"],
        part["contentRole"]
    ]);
    assert!(serde_json::from_str::<SelectablePart>(&part_array.to_string()).is_err());
    assert!(serde_json::from_value::<SelectablePart>(part_array).is_err());
    let slider: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/slider.json"
    ))
    .unwrap();
    let part = &slider["thumb"];
    assert!(serde_json::from_value::<SliderPartBinding>(part.clone()).is_ok());
    let part_array = json!([
        part["materialRole"],
        part["colorRole"],
        part["shape"],
        part["elevation"],
        part["contentRole"]
    ]);
    assert!(serde_json::from_str::<SliderPartBinding>(&part_array.to_string()).is_err());
    assert!(serde_json::from_value::<SliderPartBinding>(part_array).is_err());
}

fn string_members(document: &Value, prefix: &str, members: &mut Vec<(String, String)>) {
    for (name, value) in document.as_object().unwrap() {
        let path = format!("{prefix}/{name}");
        if value.is_object() {
            string_members(value, &path, members);
        } else if name != "schemaVersion" {
            members.push((path, value.as_str().unwrap().to_owned()));
        }
    }
}

#[test]
fn anatomy_roles_and_discriminators_require_strings() {
    for (source, component) in [
        (
            include_str!("../../../../definitions/components/command.json"),
            "command",
        ),
        (
            include_str!("../../../../definitions/components/toggle.json"),
            "toggle",
        ),
        (
            include_str!("../../../../definitions/components/slider.json"),
            "slider",
        ),
    ] {
        let document: Value = serde_json::from_str(source).unwrap();
        let mut members = Vec::new();
        string_members(&document, "", &mut members);
        for (path, name) in members {
            for value in [
                json!({name.clone(): null}),
                json!([name]),
                Value::Null,
                json!(true),
                json!(1),
            ] {
                let mut invalid = document.clone();
                *invalid.pointer_mut(&path).unwrap() = value;
                let rejected = match component {
                    "command" => {
                        serde_json::from_str::<CommandAnatomy>(&invalid.to_string()).is_err()
                            && serde_json::from_value::<CommandAnatomy>(invalid).is_err()
                    }
                    "toggle" => {
                        serde_json::from_str::<ToggleAnatomy>(&invalid.to_string()).is_err()
                            && serde_json::from_value::<ToggleAnatomy>(invalid).is_err()
                    }
                    _ => {
                        serde_json::from_str::<SliderAnatomy>(&invalid.to_string()).is_err()
                            && serde_json::from_value::<SliderAnatomy>(invalid).is_err()
                    }
                };
                assert!(rejected, "{path} must be a string");
            }
        }
    }
}

#[test]
fn command_anatomy_requires_object_representations() {
    let command: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/command.json"
    ))
    .unwrap();
    let positional = json!(["0.1.0", "command", command["variants"]]);
    for invalid in [
        positional,
        Value::Null,
        json!(true),
        json!(1),
        json!("command"),
        json!([]),
    ] {
        for result in [
            serde_json::from_str::<CommandAnatomy>(&invalid.to_string()),
            serde_json::from_value::<CommandAnatomy>(invalid.clone()),
        ] {
            assert!(result.unwrap_err().to_string().contains("anatomy object"));
        }
    }
    for (path, value) in [
        (
            "/variants",
            json!([
                command["variants"]["standard"],
                command["variants"]["primary"]
            ]),
        ),
        (
            "/variants/standard",
            json!([
                command["variants"]["standard"]["body"],
                command["variants"]["standard"]["label"]
            ]),
        ),
        ("/variants/standard/label", json!(["label"])),
        (
            "/variants/standard/body",
            json!([
                "control.interactive",
                "surface.low",
                "rounded",
                "raised",
                "content.primary"
            ]),
        ),
    ] {
        let mut invalid = command.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        for result in [
            serde_json::from_str::<CommandAnatomy>(&invalid.to_string()),
            serde_json::from_value::<CommandAnatomy>(invalid.clone()),
        ] {
            assert!(
                result.unwrap_err().to_string().contains("anatomy object"),
                "{path}"
            );
        }
    }
}

#[test]
fn toggle_anatomy_requires_object_representations() {
    let toggle: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/toggle.json"
    ))
    .unwrap();
    let positional = json!([
        "0.1.0",
        "toggle",
        toggle["track"],
        toggle["thumb"],
        toggle["label"]
    ]);
    for invalid in [
        positional,
        Value::Null,
        json!(true),
        json!(1),
        json!("toggle"),
        json!([]),
    ] {
        for result in [
            serde_json::from_str::<ToggleAnatomy>(&invalid.to_string()),
            serde_json::from_value::<ToggleAnatomy>(invalid.clone()),
        ] {
            assert!(result.unwrap_err().to_string().contains("anatomy object"));
        }
    }
    for (path, value) in [
        ("/label", json!(["label", "content.primary"])),
        (
            "/track",
            json!([
                "control.passive",
                "surface.low",
                "selection",
                "capsule",
                "embedded",
                "content.primary"
            ]),
        ),
    ] {
        let mut invalid = toggle.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        for result in [
            serde_json::from_str::<ToggleAnatomy>(&invalid.to_string()),
            serde_json::from_value::<ToggleAnatomy>(invalid.clone()),
        ] {
            assert!(
                result.unwrap_err().to_string().contains("anatomy object"),
                "{path}"
            );
        }
    }
}

#[test]
fn slider_anatomy_requires_object_representations() {
    let slider: Value = serde_json::from_str(include_str!(
        "../../../../definitions/components/slider.json"
    ))
    .unwrap();
    let positional = json!([
        "0.1.0",
        "slider",
        slider["track"],
        slider["thumb"],
        slider["label"]
    ]);
    for invalid in [
        positional,
        Value::Null,
        json!(true),
        json!(1),
        json!("slider"),
        json!([]),
    ] {
        for result in [
            serde_json::from_str::<SliderAnatomy>(&invalid.to_string()),
            serde_json::from_value::<SliderAnatomy>(invalid.clone()),
        ] {
            assert!(result.unwrap_err().to_string().contains("anatomy object"));
        }
    }
    for (path, value) in [
        ("/label", json!(["label", "content.primary"])),
        (
            "/track",
            json!([
                "control.passive",
                "surface.low",
                "capsule",
                "embedded",
                "content.primary"
            ]),
        ),
    ] {
        let mut invalid = slider.clone();
        *invalid.pointer_mut(path).unwrap() = value;
        for result in [
            serde_json::from_str::<SliderAnatomy>(&invalid.to_string()),
            serde_json::from_value::<SliderAnatomy>(invalid.clone()),
        ] {
            assert!(
                result.unwrap_err().to_string().contains("anatomy object"),
                "{path}"
            );
        }
    }
}

#[test]
fn authored_documents_still_accept_escaped_names_and_reject_duplicates() {
    for (source, component) in [
        (
            include_str!("../../../../definitions/components/command.json"),
            "command",
        ),
        (
            include_str!("../../../../definitions/components/toggle.json"),
            "toggle",
        ),
        (
            include_str!("../../../../definitions/components/slider.json"),
            "slider",
        ),
    ] {
        let escaped = source.replacen("\"schemaVersion\"", "\"\\u0073chemaVersion\"", 1);
        let valid = match component {
            "command" => serde_json::from_str::<CommandAnatomy>(&escaped).is_ok(),
            "toggle" => serde_json::from_str::<ToggleAnatomy>(&escaped).is_ok(),
            _ => serde_json::from_str::<SliderAnatomy>(&escaped).is_ok(),
        };
        assert!(valid);
        let duplicate = source.replacen(
            "\"schemaVersion\": \"0.1.0\"",
            "\"schemaVersion\": \"0.1.0\", \"\\u0073chemaVersion\": \"0.1.0\"",
            1,
        );
        assert_ne!(source, duplicate);
        let error = match component {
            "command" => serde_json::from_str::<CommandAnatomy>(&duplicate)
                .unwrap_err()
                .to_string(),
            "toggle" => serde_json::from_str::<ToggleAnatomy>(&duplicate)
                .unwrap_err()
                .to_string(),
            _ => serde_json::from_str::<SliderAnatomy>(&duplicate)
                .unwrap_err()
                .to_string(),
        };
        assert!(error.contains("duplicate"), "{error}");
    }
}
