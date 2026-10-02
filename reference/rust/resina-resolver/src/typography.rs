use resina_environment::EnvironmentSnapshot;
use resina_model::{FontFamilyRole, TypographyAssignments, TypographyRole};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypographyResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidValue,
    UnsupportedUnit,
    NonpositiveValue,
    NonfiniteScaledValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypographyResolutionError {
    pub kind: TypographyResolutionErrorKind,
    pub role: TypographyRole,
    pub field: &'static str,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for TypographyResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} for {:?}.{} at {}",
            self.kind, self.role, self.field, self.token_path
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for TypographyResolutionError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PxDimension {
    value: f64,
    unit: &'static str,
}

impl PxDimension {
    fn new(value: f64) -> Self {
        Self { value, unit: "px" }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTypography {
    family_role: FontFamilyRole,
    font_size: PxDimension,
    font_weight: f64,
    line_height: f64,
    letter_spacing: PxDimension,
}

impl ResolvedTypography {
    pub fn family_role(&self) -> FontFamilyRole {
        self.family_role
    }

    pub fn font_size(&self) -> f64 {
        self.font_size.value()
    }

    pub fn font_weight(&self) -> f64 {
        self.font_weight
    }

    pub fn line_height(&self) -> f64 {
        self.line_height
    }

    pub fn letter_spacing(&self) -> f64 {
        self.letter_spacing.value()
    }
}

pub fn resolve_semantic_typography(
    assignments: &TypographyAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
    environment: &EnvironmentSnapshot,
) -> Result<BTreeMap<TypographyRole, ResolvedTypography>, Vec<TypographyResolutionError>> {
    resolve_semantic_typography_for_scale(assignments, tokens, environment.text_scale())
}

pub(crate) fn resolve_semantic_typography_for_scale(
    assignments: &TypographyAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
    text_scale: f64,
) -> Result<BTreeMap<TypographyRole, ResolvedTypography>, Vec<TypographyResolutionError>> {
    let mut typography = BTreeMap::new();
    let mut errors = Vec::new();
    for role in TypographyRole::ALL {
        let spec = assignments.role(role);
        let scale = text_scale.max(spec.minimum_text_scale());
        let size = dimension(tokens, role, "fontSize", spec.font_size_path(), scale, true);
        let weight = font_weight(tokens, role, spec.font_weight_path());
        let line = positive_number(tokens, role, "lineHeight", spec.line_height_path());
        let tracking = dimension(
            tokens,
            role,
            "letterSpacing",
            spec.letter_spacing_path(),
            scale,
            false,
        );
        match (size, weight, line, tracking) {
            (Ok(font_size), Ok(font_weight), Ok(line_height), Ok(letter_spacing)) => {
                typography.insert(
                    role,
                    ResolvedTypography {
                        family_role: spec.family_role(),
                        font_size: PxDimension::new(font_size),
                        font_weight,
                        line_height,
                        letter_spacing: PxDimension::new(letter_spacing),
                    },
                );
            }
            (size, weight, line, tracking) => {
                errors.extend(
                    [size.err(), weight.err(), line.err(), tracking.err()]
                        .into_iter()
                        .flatten(),
                );
            }
        }
    }
    if errors.is_empty() {
        Ok(typography)
    } else {
        Err(errors)
    }
}

fn token_value<'a>(
    tokens: &'a BTreeMap<String, ResolvedToken>,
    role: TypographyRole,
    field: &'static str,
    path: &str,
    expected: &str,
) -> Result<&'a Value, TypographyResolutionError> {
    let token = tokens.get(path).ok_or_else(|| {
        error(
            TypographyResolutionErrorKind::MissingToken,
            role,
            field,
            path,
            None,
        )
    })?;
    if token.token_type != expected {
        return Err(error(
            TypographyResolutionErrorKind::WrongTokenType,
            role,
            field,
            path,
            Some(token.token_type.clone()),
        ));
    }
    validate_resolved_value(expected, &token.value).map_err(|failure| {
        error(
            TypographyResolutionErrorKind::InvalidValue,
            role,
            field,
            path,
            Some(failure.to_string()),
        )
    })?;
    Ok(&token.value)
}

fn dimension(
    tokens: &BTreeMap<String, ResolvedToken>,
    role: TypographyRole,
    field: &'static str,
    path: &str,
    scale: f64,
    positive: bool,
) -> Result<f64, TypographyResolutionError> {
    let value = token_value(tokens, role, field, path, "dimension")?;
    if value["unit"] != "px" {
        return Err(error(
            TypographyResolutionErrorKind::UnsupportedUnit,
            role,
            field,
            path,
            Some(value["unit"].to_string()),
        ));
    }
    let number = value["value"].as_f64().ok_or_else(|| {
        error(
            TypographyResolutionErrorKind::InvalidValue,
            role,
            field,
            path,
            None,
        )
    })?;
    if positive && number <= 0.0 {
        return Err(error(
            TypographyResolutionErrorKind::NonpositiveValue,
            role,
            field,
            path,
            None,
        ));
    }
    let scaled = number * scale;
    if !scaled.is_finite() {
        return Err(error(
            TypographyResolutionErrorKind::NonfiniteScaledValue,
            role,
            field,
            path,
            None,
        ));
    }
    Ok(scaled)
}

fn positive_number(
    tokens: &BTreeMap<String, ResolvedToken>,
    role: TypographyRole,
    field: &'static str,
    path: &str,
) -> Result<f64, TypographyResolutionError> {
    let value = token_value(tokens, role, field, path, "number")?;
    let number = value.as_f64().ok_or_else(|| {
        error(
            TypographyResolutionErrorKind::InvalidValue,
            role,
            field,
            path,
            None,
        )
    })?;
    if number <= 0.0 {
        Err(error(
            TypographyResolutionErrorKind::NonpositiveValue,
            role,
            field,
            path,
            None,
        ))
    } else {
        Ok(number)
    }
}

fn font_weight(
    tokens: &BTreeMap<String, ResolvedToken>,
    role: TypographyRole,
    path: &str,
) -> Result<f64, TypographyResolutionError> {
    let value = token_value(tokens, role, "fontWeight", path, "fontWeight")?;
    if let Some(number) = value.as_f64() {
        return Ok(number);
    }
    let number = match value.as_str() {
        Some("thin" | "hairline") => 100.0,
        Some("extra-light" | "ultra-light") => 200.0,
        Some("light") => 300.0,
        Some("normal" | "regular" | "book") => 400.0,
        Some("medium") => 500.0,
        Some("semi-bold" | "demi-bold") => 600.0,
        Some("bold") => 700.0,
        Some("extra-bold" | "ultra-bold") => 800.0,
        Some("black" | "heavy") => 900.0,
        Some("extra-black" | "ultra-black") => 950.0,
        _ => {
            return Err(error(
                TypographyResolutionErrorKind::InvalidValue,
                role,
                "fontWeight",
                path,
                None,
            ));
        }
    };
    Ok(number)
}

fn error(
    kind: TypographyResolutionErrorKind,
    role: TypographyRole,
    field: &'static str,
    path: &str,
    detail: Option<String>,
) -> TypographyResolutionError {
    TypographyResolutionError {
        kind,
        role,
        field,
        token_path: path.to_owned(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_tokens::resolve_token_document;
    use serde_json::{Value, json};

    #[test]
    fn typography_roles_bind_resolved_token_aliases() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/assignment-vectors.json"
        ))
        .unwrap();
        let assignments: TypographyAssignments =
            serde_json::from_value(vectors[0]["document"].clone()).unwrap();
        let source = json!({
            "foundation": {
                "size": {"$type": "dimension", "body": {"$value": {"value": 24, "unit": "px"}}},
                "tracking": {"$type": "dimension", "body": {"$value": {"value": -0.5, "unit": "px"}}}
            },
            "type": {
                "size": {"base": {"$value": "{foundation.size.body}"}},
                "weight": {"base": {"$type": "fontWeight", "$value": "semi-bold"}},
                "line": {"base": {"$type": "number", "$value": 1.25}},
                "tracking": {"base": {"$value": "{foundation.tracking.body}"}}
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let mut environment: Value = serde_json::from_str(include_str!(
            "../../../../conformance/environment/valid-mixed-input.json"
        ))
        .unwrap();
        environment["textScale"] = json!(2);
        let environment: EnvironmentSnapshot = serde_json::from_value(environment).unwrap();
        let styles = resolve_semantic_typography(&assignments, &tokens, &environment).unwrap();
        assert_eq!(styles[&TypographyRole::Body].font_size(), 48.0);
        assert_eq!(styles[&TypographyRole::Body].font_weight(), 600.0);
        assert_eq!(styles[&TypographyRole::Body].letter_spacing(), -1.0);
        assert_eq!(
            styles[&TypographyRole::Code].family_role(),
            FontFamilyRole::Monospace
        );
    }

    #[test]
    fn dtcg_font_weight_aliases_resolve_to_numeric_weights() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/font-weight-alias-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let mut tokens = BTreeMap::new();
            tokens.insert(
                "weight".to_owned(),
                ResolvedToken {
                    token_type: "fontWeight".to_owned(),
                    value: vector["alias"].clone(),
                },
            );
            assert_eq!(
                font_weight(&tokens, TypographyRole::Body, "weight").unwrap(),
                vector["weight"].as_f64().unwrap(),
                "{}",
                vector["alias"]
            );
        }

        let mut tokens = BTreeMap::new();
        tokens.insert(
            "weight".to_owned(),
            ResolvedToken {
                token_type: "fontWeight".to_owned(),
                value: json!(350),
            },
        );
        assert_eq!(
            font_weight(&tokens, TypographyRole::Body, "weight").unwrap(),
            350.0
        );
    }

    #[test]
    fn typography_resolution_conformance_vectors() {
        let base: Value = serde_json::from_str(include_str!(
            "../../../../conformance/environment/valid-mixed-input.json"
        ))
        .unwrap();
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: TypographyAssignments =
                serde_json::from_value(vector["assignments"].clone()).unwrap();
            let tokens = vector["tokens"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(path, token)| {
                    (
                        path.clone(),
                        ResolvedToken {
                            token_type: token["token_type"].as_str().unwrap().to_owned(),
                            value: token["value"].clone(),
                        },
                    )
                })
                .collect();
            let mut environment = base.clone();
            environment["textScale"] = vector["textScale"].clone();
            let environment: EnvironmentSnapshot = serde_json::from_value(environment).unwrap();
            let result = resolve_semantic_typography(&assignments, &tokens, &environment);
            if let Some(expected) = vector.get("expected") {
                let styles = result.unwrap();
                assert_eq!(
                    styles.len(),
                    TypographyRole::ALL.len(),
                    "{}",
                    vector["name"]
                );
                for role in TypographyRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        serde_json::to_value(&styles[&role]).unwrap(),
                        expected[name.as_str().unwrap()],
                        "{}: {name}",
                        vector["name"]
                    );
                }
            } else {
                let errors: Vec<_> = result.unwrap_err().iter().map(|error| json!({
                    "kind":format!("{:?}",error.kind), "role":error.role, "field":error.field,
                    "tokenPath":error.token_path, "detail":error.detail
                })).collect();
                assert_eq!(
                    serde_json::to_value(errors).unwrap(),
                    vector["errors"],
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
