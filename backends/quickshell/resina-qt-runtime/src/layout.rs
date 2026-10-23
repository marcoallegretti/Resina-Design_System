use crate::label::compiled_theme;
use cxx_qt_lib::QString;
use resina_environment::EnvironmentSnapshot;
use resina_model::{ComponentLayoutProfile, SurfaceSize};
use resina_resolver::{CommandLayoutError, CommandLayoutInput, resolve_command_layout};
use resina_tokens::{parse_token_document, resolve_token_document};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        fn resolve_command_layout_profile(request: &QString) -> QString;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutRequest {
    theme_source: String,
    external_sources: BTreeMap<String, String>,
    profile: ComponentLayoutProfile,
    tokens: Value,
    environment: EnvironmentSnapshot,
    available_size: SurfaceSize,
}

fn prepare(request: &str) -> Result<Value, String> {
    if request.len() > MAX_REQUEST_BYTES {
        return Err("Qt command layout request exceeds 1 MiB".to_owned());
    }
    let document = parse_token_document(request).map_err(|error| error.to_string())?;
    if !document.is_object() {
        return Err("Qt command layout request must be an object".to_owned());
    }
    let request: LayoutRequest =
        serde_json::from_value(document).map_err(|error| error.to_string())?;
    let theme = compiled_theme(&request.theme_source, &request.external_sources)?;
    let tokens = resolve_token_document(&request.tokens)
        .map_err(|errors| CommandLayoutError::Tokens(errors).to_string())?;
    let layout = resolve_command_layout(CommandLayoutInput {
        profile: &request.profile,
        tokens: &tokens,
        space: theme.space(),
        environment: &request.environment,
        available_size: request.available_size,
    })
    .map_err(|error| error.to_string())?;
    serde_json::to_value(layout).map_err(|error| error.to_string())
}

fn resolve_command_layout_profile(request: &QString) -> QString {
    QString::from(
        prepare(&request.to_string())
            .unwrap_or_else(|error| json!({"error": error}))
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str = include_str!("../../../../conformance/layout/command-layout-cases.json");
    const FOUNDATION: &str = include_str!("../../../../tokens/foundation.json");
    const THEMES: [&str; 2] = [
        include_str!("../../../../tokens/themes/light.json"),
        include_str!("../../../../tokens/themes/dark.json"),
    ];

    fn same(actual: &Value, expected: &Value) -> bool {
        match (actual, expected) {
            (Value::Number(actual), Value::Number(expected)) => {
                actual.as_f64() == expected.as_f64()
            }
            (Value::Object(actual), Value::Object(expected)) => {
                actual.len() == expected.len()
                    && expected
                        .iter()
                        .all(|(key, value)| actual.get(key).is_some_and(|found| same(found, value)))
            }
            _ => actual == expected,
        }
    }

    fn bridge_request(theme: &str, case: &Value) -> String {
        let request = &case["request"];
        json!({
            "themeSource": theme,
            "externalSources": {"foundation.json": FOUNDATION},
            "profile": request["profile"], "tokens": request["tokens"],
            "environment": request["environment"], "availableSize": request["availableSize"],
        })
        .to_string()
    }

    fn authored_binding_cases() -> Vec<Value> {
        let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
        let authored = cases[0]["request"]["spatialBinding"].clone();
        cases
            .into_iter()
            .filter(|case| {
                case["requestSchemaValid"] == true && case["request"]["spatialBinding"] == authored
            })
            .collect()
    }

    #[test]
    fn authored_themes_resolve_the_shared_cases_that_use_their_spatial_binding() {
        let cases = authored_binding_cases();
        assert!(cases.len() >= 40, "{}", cases.len());
        let mut accepted = 0;
        let mut rejected = 0;
        for theme in THEMES {
            for case in &cases {
                let result = prepare(&bridge_request(theme, case));
                match (case.get("expected"), case.get("errorContains")) {
                    (Some(expected), None) => {
                        let actual =
                            result.unwrap_or_else(|error| panic!("{}: {error}", case["name"]));
                        assert!(same(&actual, expected), "{}: {actual}", case["name"]);
                        accepted += 1;
                    }
                    (None, Some(fragment)) => {
                        let error = result.unwrap_err();
                        assert!(
                            error.contains(fragment.as_str().unwrap()),
                            "{}: {error}",
                            case["name"]
                        );
                        rejected += 1;
                    }
                    _ => panic!("{} has no unique outcome", case["name"]),
                }
            }
        }
        assert_eq!(accepted + rejected, THEMES.len() * cases.len());
        assert!(accepted >= 2 * 36 && rejected > 0, "{accepted} {rejected}");
    }

    #[test]
    fn rejected_requests_publish_a_diagnostic_and_no_layout() {
        let case: Value = serde_json::from_str::<Vec<Value>>(CASES).unwrap().remove(0);
        let valid: Value = serde_json::from_str(&bridge_request(THEMES[0], &case)).unwrap();
        let mut unknown = valid.clone();
        unknown["extra"] = json!(1);
        let mut missing_theme_source = valid.clone();
        missing_theme_source
            .as_object_mut()
            .unwrap()
            .remove("themeSource");
        for source in [
            "[]".to_owned(),
            "{}".to_owned(),
            unknown.to_string(),
            missing_theme_source.to_string(),
            " ".repeat(MAX_REQUEST_BYTES + 1),
        ] {
            let result: Value = serde_json::from_str(
                &resolve_command_layout_profile(&QString::from(source.as_str())).to_string(),
            )
            .unwrap();
            assert!(result["error"].is_string() && result.get("minimumSize").is_none());
        }
    }
}
