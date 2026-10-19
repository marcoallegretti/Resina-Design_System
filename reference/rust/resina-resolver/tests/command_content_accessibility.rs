use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{
    ActivationState, CommandContent, CommandContentForm, SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandAccessibilityInput, CommandAccessibilityIr, CommandContentAccessibilityError,
    CommandContentAccessibilityInput, CommandLabelError, CommandLabelInput, CommandLabelIr,
    compile_theme_source, resolve_command_accessibility, resolve_command_content_accessibility,
    resolve_command_label,
};
use serde_json::{Value, json};
use std::convert::Infallible;

fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/accessibility/command-content-cases.json"
    ))
    .unwrap()
}

fn case(name: &str) -> Value {
    cases()
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap()
}

fn resolve(
    content: &CommandContent,
    input: &Value,
) -> Result<CommandAccessibilityIr, CommandContentAccessibilityError> {
    let activation: ActivationState = serde_json::from_value(input["activation"].clone()).unwrap();
    let label = input["laidOutLabel"].as_str().map(laid_out_label);
    resolve_command_content_accessibility(CommandContentAccessibilityInput {
        content,
        label: label.as_ref(),
        activation: &activation,
        description: input["description"].as_str(),
        focusable: input["focusable"].as_bool().unwrap(),
    })
}

fn laid_out_label(text: &str) -> CommandLabelIr {
    lay_out_label(text, 60.0).unwrap()
}

fn lay_out_label(text: &str, width: f64) -> Result<CommandLabelIr, CommandLabelError<Infallible>> {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let theme = compile_theme_source(
        source["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let environment =
        serde_json::from_value(source["surface"]["body"]["theme"]["environment"].clone()).unwrap();
    let typography =
        theme.resolve(&environment).unwrap().typography()[&TypographyRole::Label].clone();
    resolve_command_label(
        CommandLabelInput {
            text,
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 64.0,
                height: 32.0,
            },
            maximum_size: SurfaceSize {
                width: 200.0,
                height: 160.0,
            },
            padding: SafeArea {
                start: 12.0,
                end: 8.0,
                top: 6.0,
                bottom: 10.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width,
                height: 54.0,
            })
        },
    )
}

#[test]
fn public_cases_match_from_text_and_value() {
    let cases = cases();
    let mut forms = Vec::new();
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let input = &case["input"];
        let parsed = [
            serde_json::from_str::<CommandContent>(&input["content"].to_string()).unwrap(),
            serde_json::from_value::<CommandContent>(input["content"].clone()).unwrap(),
        ];
        assert_eq!(parsed[0], parsed[1], "{name}");
        forms.push(parsed[0].form());
        for content in &parsed {
            let result = resolve(content, input);
            match case.get("error") {
                Some(error) => assert_eq!(
                    format!("{:?}", result.unwrap_err()),
                    error.as_str().unwrap(),
                    "{name}"
                ),
                None => {
                    let ir = result.unwrap();
                    assert_eq!(
                        serde_json::to_value(&ir).unwrap(),
                        case["expected"],
                        "{name}"
                    );
                    assert_eq!(ir.name(), content.text(), "{name}");
                }
            }
        }
    }
    for form in [
        CommandContentForm::Label,
        CommandContentForm::IconAndLabel,
        CommandContentForm::IconOnly,
    ] {
        assert!(forms.contains(&form), "{form:?}");
    }
}

#[test]
fn icon_and_label_resolves_the_ordinary_command_snapshot() {
    let ordinary: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/accessibility/command-cases.json"
    ))
    .unwrap();
    let icon = case("icon and label")["input"]["content"]["icon"].clone();
    for ordinary_case in ordinary {
        let input = &ordinary_case["input"];
        let text = input["labelText"].as_str().unwrap();
        let label = laid_out_label(text);
        let activation: ActivationState =
            serde_json::from_value(input["activation"].clone()).unwrap();
        let expected = resolve_command_accessibility(CommandAccessibilityInput {
            label: &label,
            activation: &activation,
            description: input["description"].as_str(),
            focusable: input["focusable"].as_bool().unwrap(),
        })
        .map_err(CommandContentAccessibilityError::from);
        let mut content_input = input.clone();
        content_input["laidOutLabel"] = json!(text);
        content_input["content"] =
            json!({"schemaVersion": "0.1.0", "form": "iconAndLabel", "icon": icon, "label": text});
        let content: CommandContent =
            serde_json::from_value(content_input["content"].clone()).unwrap();
        assert_eq!(
            resolve(&content, &content_input),
            expected,
            "{}",
            ordinary_case["name"]
        );
        content_input["content"] =
            json!({"schemaVersion": "0.1.0", "form": "label", "label": text});
        let content: CommandContent =
            serde_json::from_value(content_input["content"].clone()).unwrap();
        assert_eq!(
            resolve(&content, &content_input),
            Err(CommandContentAccessibilityError::LabelForm),
            "{}",
            ordinary_case["name"]
        );
    }
}

#[test]
fn failed_label_layout_publishes_no_snapshot() {
    let case = case("icon and label");
    let input = &case["input"];
    let text = input["laidOutLabel"].as_str().unwrap();
    assert!(lay_out_label(text, 60.0).is_ok());
    assert!(matches!(
        lay_out_label(text, 400.0),
        Err(CommandLabelError::Invalid(
            "fitted label exceeds offered width"
        ))
    ));
    let content: CommandContent = serde_json::from_value(input["content"].clone()).unwrap();
    let mut failed = input.clone();
    failed["laidOutLabel"] = Value::Null;
    assert_eq!(
        resolve(&content, &failed),
        Err(CommandContentAccessibilityError::MissingLabelLayout)
    );
}

#[test]
fn icon_never_contributes_to_the_snapshot() {
    let replacements = [
        json!({"schemaVersion": "0.1.0", "reference": "resina.fixture:other", "kind": "symbolic",
               "inkBounds": {"x": 0, "y": 0, "width": 1, "height": 1},
               "opticalBounds": {"x": 0, "y": 0, "width": 1, "height": 1},
               "direction": {"behavior": "mirror"},
               "layers": [{"role": "content"}, {"role": "accent.primary"}]}),
        json!({"schemaVersion": "0.1.0", "reference": "product.media:stop", "kind": "color",
               "inkBounds": {"x": 0.25, "y": 0.25, "width": 0.5, "height": 0.5},
               "opticalBounds": {"x": 0.2, "y": 0.2, "width": 0.6, "height": 0.6},
               "direction": {"behavior": "directionalVariants",
                             "ltr": "product.media:stop-ltr", "rtl": "product.media:stop-rtl"},
               "bakedLight": true}),
    ];
    for case in cases() {
        let input = &case["input"];
        if input["content"].get("icon").is_none() {
            continue;
        }
        let original = resolve(
            &serde_json::from_value(input["content"].clone()).unwrap(),
            input,
        );
        for replacement in &replacements {
            let mut content = input["content"].clone();
            content["icon"] = replacement.clone();
            let content: CommandContent = serde_json::from_value(content).unwrap();
            assert_eq!(resolve(&content, input), original, "{}", case["name"]);
        }
    }
}
