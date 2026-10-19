use resina_environment::DensityPreference;
use resina_model::{DENSITY_MODES, DensityModes, SpatialAssignments, SpatialRole};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidDimensionValue,
    NegativeDistance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialResolutionError {
    pub kind: SpatialResolutionErrorKind,
    pub role: SpatialRole,
    pub density: Option<DensityPreference>,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for SpatialResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} for {:?}", self.kind, self.role)?;
        if let Some(density) = self.density {
            write!(formatter, " in {density:?} density")?;
        }
        write!(formatter, " at {}", self.token_path)?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SpatialResolutionError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
enum BoundSpace {
    Uniform(Value),
    PerDensity(DensityModes<Value>),
}

/// Validated spatial values, keeping every density mode of a density-mapped role.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SpatialBinding {
    roles: BTreeMap<SpatialRole, BoundSpace>,
}

impl<'de> Deserialize<'de> for SpatialBinding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Roles;

        impl<'de> Visitor<'de> for Roles {
            type Value = SpatialBinding;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a spatial binding object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut roles = BTreeMap::new();
                while let Some(role) = access.next_key::<SpatialRole>()? {
                    let bound = if role.is_density_mapped() {
                        let values: DensityModes<Value> = access.next_value()?;
                        values
                            .try_map(|_, value| checked_dimension(value))
                            .map_err(A::Error::custom)?;
                        BoundSpace::PerDensity(values)
                    } else {
                        let value: Value = access.next_value()?;
                        checked_dimension(&value).map_err(A::Error::custom)?;
                        BoundSpace::Uniform(value)
                    };
                    if roles.insert(role, bound).is_some() {
                        return Err(A::Error::custom("duplicate spatial role"));
                    }
                }
                if SpatialRole::ALL
                    .iter()
                    .any(|role| !roles.contains_key(role))
                {
                    return Err(A::Error::custom("missing spatial role"));
                }
                Ok(SpatialBinding { roles })
            }
        }

        deserializer.deserialize_map(Roles)
    }
}

fn checked_dimension(value: &Value) -> Result<(), String> {
    validate_resolved_value("dimension", value)
        .map_err(|error| format!("invalid spatial dimension object: {error}"))?;
    if value["value"]
        .as_f64()
        .is_some_and(|distance| distance < 0.0)
    {
        return Err("negative spatial distance".to_owned());
    }
    Ok(())
}

impl SpatialBinding {
    pub fn value_for(&self, role: SpatialRole, density: DensityPreference) -> &Value {
        match &self.roles[&role] {
            BoundSpace::Uniform(value) => value,
            BoundSpace::PerDensity(values) => values.get(density),
        }
    }

    pub fn for_density(&self, density: DensityPreference) -> BTreeMap<SpatialRole, Value> {
        SpatialRole::ALL
            .into_iter()
            .map(|role| (role, self.value_for(role, density).clone()))
            .collect()
    }
}

pub fn resolve_semantic_space(
    assignments: &SpatialAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<SpatialBinding, Vec<SpatialResolutionError>> {
    let mut roles = BTreeMap::new();
    let mut errors = Vec::new();
    for role in SpatialRole::ALL {
        if role.is_density_mapped() {
            let results = DensityModes::from_fn(|density| {
                let path = assignments.token_path_for(role, density);
                resolve_dimension(tokens, path).map_err(|(kind, detail)| SpatialResolutionError {
                    kind,
                    role,
                    density: Some(density),
                    token_path: path.to_owned(),
                    detail,
                })
            });
            match results.try_map(|_, result| result.clone()) {
                Ok(values) => {
                    roles.insert(role, BoundSpace::PerDensity(values));
                }
                Err(_) => errors.extend(
                    DENSITY_MODES
                        .into_iter()
                        .filter_map(|density| results.get(density).clone().err()),
                ),
            }
        } else {
            let path = assignments.token_path_for(role, DensityPreference::Standard);
            match resolve_dimension(tokens, path) {
                Ok(value) => {
                    roles.insert(role, BoundSpace::Uniform(value));
                }
                Err((kind, detail)) => errors.push(SpatialResolutionError {
                    kind,
                    role,
                    density: None,
                    token_path: path.to_owned(),
                    detail,
                }),
            }
        }
    }
    if errors.is_empty() {
        Ok(SpatialBinding { roles })
    } else {
        Err(errors)
    }
}

fn resolve_dimension(
    tokens: &BTreeMap<String, ResolvedToken>,
    path: &str,
) -> Result<Value, (SpatialResolutionErrorKind, Option<String>)> {
    match tokens.get(path) {
        None => Err((SpatialResolutionErrorKind::MissingToken, None)),
        Some(token) if token.token_type != "dimension" => Err((
            SpatialResolutionErrorKind::WrongTokenType,
            Some(token.token_type.clone()),
        )),
        Some(token) => match validate_resolved_value("dimension", &token.value) {
            Err(error) => Err((
                SpatialResolutionErrorKind::InvalidDimensionValue,
                Some(error.to_string()),
            )),
            Ok(())
                if token.value["value"]
                    .as_f64()
                    .is_some_and(|value| value < 0.0) =>
            {
                Err((SpatialResolutionErrorKind::NegativeDistance, None))
            }
            Ok(()) => Ok(token.value.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use resina_tokens::resolve_token_document;
    use serde_json::json;

    #[test]
    fn spatial_roles_bind_resolved_dimension_aliases() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/assignment-vectors.json"
        ))
        .unwrap();
        let assignments: SpatialAssignments =
            serde_json::from_value(vectors[0]["document"].clone()).unwrap();
        let source = json!({
            "foundation": {
                "space": {
                    "$type": "dimension",
                    "0": {"$value": {"value": 0, "unit": "px"}},
                    "1": {"$value": {"value": 4, "unit": "px"}},
                    "2": {"$value": {"value": 8, "unit": "px"}},
                    "3": {"$value": {"value": 1, "unit": "rem"}},
                    "4": {"$value": {"value": 20, "unit": "px"}},
                    "5": {"$value": {"value": 32, "unit": "px"}},
                    "6": {"$value": "{foundation.space.0}"}
                }
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let binding = resolve_semantic_space(&assignments, &tokens).unwrap();
        let values = binding.for_density(DensityPreference::Standard);
        assert_eq!(
            values[&SpatialRole::ContainerOuter],
            json!({"value": 1, "unit": "rem"})
        );
        assert_eq!(
            values[&SpatialRole::Page],
            values[&SpatialRole::ControlInline]
        );
        assert_eq!(
            binding.value_for(SpatialRole::ControlInsetInline, DensityPreference::Compact),
            &json!({"value": 4, "unit": "px"})
        );
        assert_eq!(
            binding.value_for(
                SpatialRole::ControlInsetInline,
                DensityPreference::Immersive
            ),
            &json!({"value": 32, "unit": "px"})
        );
    }

    #[test]
    fn spatial_resolution_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: SpatialAssignments =
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
            let result = resolve_semantic_space(&assignments, &tokens);
            if let Some(expected) = vector.get("expected") {
                let binding = result.unwrap();
                assert_eq!(
                    expected.as_object().unwrap().len(),
                    SpatialRole::ALL.len(),
                    "{}",
                    vector["name"]
                );
                for role in SpatialRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    for density in DENSITY_MODES {
                        let expected = &expected[name.as_str().unwrap()];
                        let expected = if role.is_density_mapped() {
                            &expected[serde_json::to_value(density).unwrap().as_str().unwrap()]
                        } else {
                            expected
                        };
                        assert_eq!(
                            binding.value_for(role, density),
                            expected,
                            "{}: {name}",
                            vector["name"]
                        );
                    }
                }
            } else {
                let errors = result.unwrap_err();
                let actual: Vec<_> = errors
                    .iter()
                    .map(|error| {
                        json!({
                            "kind": format!("{:?}", error.kind),
                            "role": error.role,
                            "density": error.density,
                            "tokenPath": error.token_path,
                            "detail": error.detail
                        })
                    })
                    .collect();
                assert_eq!(
                    serde_json::to_value(actual).unwrap(),
                    vector["errors"],
                    "{}",
                    vector["name"]
                );
            }
        }
    }
}
