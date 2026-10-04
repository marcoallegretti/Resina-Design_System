use resina_model::TypographyRole;
use resina_resolver::{ResolvedTypography, compile_theme_source};
use serde_json::{Value, json};

pub fn style(scale: f64, tracking: f64, weight: f64, line: f64) -> ResolvedTypography {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let source = &request["surface"]["body"]["theme"];
    let mut theme: Value = serde_json::from_str(source["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["type"]["tracking"]["$value"]["value"] = json!(tracking);
    theme["tokens"]["type"]["weight"]["$value"] = json!(weight);
    theme["tokens"]["type"]["line"]["$value"] = json!(line);
    let mut environment = source["environment"].clone();
    environment["textScale"] = json!(scale);
    compile_theme_source(&theme.to_string())
        .unwrap()
        .resolve(&serde_json::from_value(environment).unwrap())
        .unwrap()
        .typography()[&TypographyRole::Label]
        .clone()
}
