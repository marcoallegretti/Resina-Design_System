use crate::{
    HeadlessResolution, ThemeCompilationError, TypographyResolutionError,
    compile_theme_source_with_sources,
};
use resina_environment::EnvironmentSnapshot;
use resina_tokens::parse_token_document;
use serde::Deserialize;
use std::{collections::BTreeMap, fmt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ThemeResolutionRequest {
    schema_version: String,
    theme_source: String,
    external_sources: BTreeMap<String, String>,
    environment: EnvironmentSnapshot,
}

#[derive(Debug)]
pub enum ThemeResolutionError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    Compile(ThemeCompilationError),
    Resolve(Vec<TypographyResolutionError>),
}

impl fmt::Display for ThemeResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "theme request parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid theme request: {error}"),
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Compile(error) => write!(formatter, "theme compilation failed: {error}"),
            Self::Resolve(errors) => {
                formatter.write_str("theme resolution failed")?;
                for error in errors {
                    write!(formatter, "\n  {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ThemeResolutionError {}

pub fn resolve_theme_request_source(
    source: &str,
) -> Result<HeadlessResolution, ThemeResolutionError> {
    let document = parse_token_document(source).map_err(ThemeResolutionError::Parse)?;
    if document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(ThemeResolutionError::UnsupportedVersion);
    }
    let request: ThemeResolutionRequest =
        serde_json::from_value(document).map_err(ThemeResolutionError::Request)?;
    debug_assert_eq!(request.schema_version, "0.1.0");
    let theme = compile_theme_source_with_sources(&request.theme_source, &request.external_sources)
        .map_err(ThemeResolutionError::Compile)?;
    theme
        .resolve(&request.environment)
        .map_err(ThemeResolutionError::Resolve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const THEME: &str = include_str!("../../../../conformance/themes/valid-source.json");
    const HEADLESS: &str = include_str!("../../../../conformance/headless/valid-request.json");
    const EXPECTED: &str =
        include_str!("../../../../conformance/headless/expected-resolution.json");

    #[test]
    fn embedded_theme_request_matches_headless_fixture() {
        let headless: Value = serde_json::from_str(HEADLESS).unwrap();
        let request = json!({
            "schemaVersion": "0.1.0",
            "themeSource": THEME,
            "externalSources": {},
            "environment": headless["environment"]
        });
        let actual = resolve_theme_request_source(&request.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(EXPECTED).unwrap()
        );
    }

    #[test]
    fn invalid_theme_request_does_not_resolve() {
        let headless: Value = serde_json::from_str(HEADLESS).unwrap();
        let request = json!({
            "schemaVersion": "0.1.0",
            "themeSource": "{\"schemaVersion\":\"0.1.0\",\"schemaVersion\":\"0.1.0\"}",
            "externalSources": {},
            "environment": headless["environment"]
        });
        assert!(matches!(
            resolve_theme_request_source(&request.to_string()),
            Err(ThemeResolutionError::Compile(_))
        ));
    }
}
