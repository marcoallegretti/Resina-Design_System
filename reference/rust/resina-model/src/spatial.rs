use crate::DensityModes;
use resina_environment::DensityPreference;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SpatialRole {
    #[serde(rename = "space.control.inline")]
    ControlInline,
    #[serde(rename = "space.control.block")]
    ControlBlock,
    #[serde(rename = "space.control.inset.inline")]
    ControlInsetInline,
    #[serde(rename = "space.control.inset.block")]
    ControlInsetBlock,
    #[serde(rename = "space.container.inner")]
    ContainerInner,
    #[serde(rename = "space.container.outer")]
    ContainerOuter,
    #[serde(rename = "space.group")]
    Group,
    #[serde(rename = "space.section")]
    Section,
    #[serde(rename = "space.page")]
    Page,
}

impl SpatialRole {
    pub const ALL: [Self; 9] = [
        Self::ControlInline,
        Self::ControlBlock,
        Self::ControlInsetInline,
        Self::ControlInsetBlock,
        Self::ContainerInner,
        Self::ContainerOuter,
        Self::Group,
        Self::Section,
        Self::Page,
    ];

    pub fn is_density_mapped(self) -> bool {
        matches!(self, Self::ControlInsetInline | Self::ControlInsetBlock)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialAssignments {
    schema_version: String,
    roles: BTreeMap<SpatialRole, SpatialMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
enum SpatialMapping {
    Uniform(TokenPath),
    PerDensity(DensityModes<TokenPath>),
}

impl<'de> Deserialize<'de> for SpatialAssignments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input =
            crate::deserialize_assignment_object::<D, SpatialAssignmentsInput>(deserializer)?;
        Self::try_from(input).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpatialAssignmentsInput {
    schema_version: String,
    #[serde(deserialize_with = "deserialize_unique_roles")]
    roles: BTreeMap<SpatialRole, SpatialMapping>,
}

fn deserialize_unique_roles<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<SpatialRole, SpatialMapping>, D::Error> {
    struct UniqueRoles;

    impl<'de> Visitor<'de> for UniqueRoles {
        type Value = BTreeMap<SpatialRole, SpatialMapping>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a spatial role mapping without duplicate members")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut roles = BTreeMap::new();
            while let Some(role) = access.next_key::<SpatialRole>()? {
                let mapping = if role.is_density_mapped() {
                    SpatialMapping::PerDensity(access.next_value()?)
                } else {
                    SpatialMapping::Uniform(access.next_value()?)
                };
                if roles.insert(role, mapping).is_some() {
                    return Err(A::Error::custom("duplicate spatial role"));
                }
            }
            Ok(roles)
        }
    }

    deserializer.deserialize_map(UniqueRoles)
}

impl TryFrom<SpatialAssignmentsInput> for SpatialAssignments {
    type Error = &'static str;

    fn try_from(input: SpatialAssignmentsInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.2.0" {
            return Err("schemaVersion must be 0.2.0");
        }
        if SpatialRole::ALL
            .iter()
            .any(|role| !input.roles.contains_key(role))
        {
            return Err("missing spatial role");
        }
        Ok(Self {
            schema_version: input.schema_version,
            roles: input.roles,
        })
    }
}

impl SpatialAssignments {
    /// Density is ignored for roles that have one mapping for every density mode.
    pub fn token_path_for(&self, role: SpatialRole, density: DensityPreference) -> &str {
        match &self.roles[&role] {
            SpatialMapping::Uniform(path) => path.0.as_str(),
            SpatialMapping::PerDensity(paths) => paths.get(density).0.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct TokenPath(String);

impl TryFrom<String> for TokenPath {
    type Error = &'static str;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        if path.is_empty() {
            Err("spatial role token path must not be empty")
        } else {
            Ok(Self(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            for result in [
                serde_json::from_str::<SpatialAssignments>(&vector["document"].to_string()),
                serde_json::from_value::<SpatialAssignments>(vector["document"].clone()),
            ] {
                if let Some(expected) = vector.get("expected") {
                    let assignments = result.unwrap();
                    for role in SpatialRole::ALL {
                        let name = serde_json::to_value(role).unwrap();
                        for density in crate::DENSITY_MODES {
                            let expected = &expected[name.as_str().unwrap()];
                            let expected = if role.is_density_mapped() {
                                &expected[serde_json::to_value(density).unwrap().as_str().unwrap()]
                            } else {
                                expected
                            };
                            assert_eq!(
                                assignments.token_path_for(role, density),
                                expected.as_str().unwrap(),
                                "{}: {name}",
                                vector["name"]
                            );
                        }
                    }
                    assert_eq!(
                        serde_json::to_value(&assignments).unwrap(),
                        vector["document"]
                    );
                } else {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains(vector["error"].as_str().unwrap()),
                        "{}",
                        vector["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn duplicate_role_member_is_rejected_before_value_conversion() {
        let source = include_str!("../../../../conformance/spatial/invalid-duplicate-role.json");
        let error = serde_json::from_str::<SpatialAssignments>(source).unwrap_err();
        assert!(
            error.to_string().contains("duplicate spatial role"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_assignment_members_and_escaped_names_preserve_validation() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/spatial/assignment-vectors.json"
        ))
        .unwrap();
        let original = &vectors[0]["document"];
        let source = original.to_string();
        for (object, fields) in [
            (
                original,
                vec!["schemaVersion".to_owned(), "roles".to_owned()],
            ),
            (
                &original["roles"],
                SpatialRole::ALL
                    .into_iter()
                    .map(|role| {
                        serde_json::to_value(role)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned()
                    })
                    .collect(),
            ),
            (
                &original["roles"]["space.control.inset.inline"],
                crate::DENSITY_MODES
                    .into_iter()
                    .map(|density| {
                        serde_json::to_value(density)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_owned()
                    })
                    .collect(),
            ),
        ] {
            for field in fields {
                let member = format!("\"{field}\":{}", object[&field]);
                let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
                let escaped_member = format!("\"{escaped}\":{}", object[&field]);
                for duplicate in [&member, &escaped_member] {
                    let changed = source.replacen(&member, &format!("{member},{duplicate}"), 1);
                    assert!(changed.len() > source.len());
                    let error = serde_json::from_str::<SpatialAssignments>(&changed).unwrap_err();
                    assert!(error.to_string().contains("duplicate"), "{field}: {error}");
                }
                let changed = source.replacen(&member, &escaped_member, 1);
                let assignment: SpatialAssignments = serde_json::from_str(&changed).unwrap();
                assert_eq!(serde_json::to_value(assignment).unwrap(), *original);
            }
        }
    }
}
