use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TypographyRole {
    Display,
    TitleLarge,
    Title,
    Heading,
    BodyLarge,
    Body,
    Label,
    Caption,
    Numeric,
    Code,
}

impl TypographyRole {
    pub const ALL: [Self; 10] = [
        Self::Display,
        Self::TitleLarge,
        Self::Title,
        Self::Heading,
        Self::BodyLarge,
        Self::Body,
        Self::Label,
        Self::Caption,
        Self::Numeric,
        Self::Code,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontFamilyRole {
    Sans,
    Display,
    Monospace,
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "TypographyRoleSpecInput")]
pub struct TypographyRoleSpec {
    family_role: FontFamilyRole,
    font_size: TokenPath,
    font_weight: TokenPath,
    line_height: TokenPath,
    letter_spacing: TokenPath,
    minimum_text_scale: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypographyRoleSpecInput {
    family_role: FontFamilyRole,
    font_size: TokenPath,
    font_weight: TokenPath,
    line_height: TokenPath,
    letter_spacing: TokenPath,
    minimum_text_scale: f64,
}

impl TryFrom<TypographyRoleSpecInput> for TypographyRoleSpec {
    type Error = &'static str;

    fn try_from(input: TypographyRoleSpecInput) -> Result<Self, Self::Error> {
        if !input.minimum_text_scale.is_finite() || input.minimum_text_scale < 1.0 {
            return Err("minimumTextScale must be finite and at least 1");
        }
        Ok(Self {
            family_role: input.family_role,
            font_size: input.font_size,
            font_weight: input.font_weight,
            line_height: input.line_height,
            letter_spacing: input.letter_spacing,
            minimum_text_scale: input.minimum_text_scale,
        })
    }
}

impl TypographyRoleSpec {
    pub fn family_role(&self) -> FontFamilyRole {
        self.family_role
    }

    pub fn font_size_path(&self) -> &str {
        &self.font_size.0
    }

    pub fn font_weight_path(&self) -> &str {
        &self.font_weight.0
    }

    pub fn line_height_path(&self) -> &str {
        &self.line_height.0
    }

    pub fn letter_spacing_path(&self) -> &str {
        &self.letter_spacing.0
    }

    pub fn minimum_text_scale(&self) -> f64 {
        self.minimum_text_scale
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "TypographyAssignmentsInput")]
pub struct TypographyAssignments {
    schema_version: String,
    roles: BTreeMap<TypographyRole, TypographyRoleSpec>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypographyAssignmentsInput {
    schema_version: String,
    #[serde(deserialize_with = "deserialize_unique_roles")]
    roles: BTreeMap<TypographyRole, TypographyRoleSpec>,
}

fn deserialize_unique_roles<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<TypographyRole, TypographyRoleSpec>, D::Error> {
    struct UniqueRoles;

    impl<'de> Visitor<'de> for UniqueRoles {
        type Value = BTreeMap<TypographyRole, TypographyRoleSpec>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a typography role mapping without duplicate members")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut roles = BTreeMap::new();
            while let Some((role, spec)) = access.next_entry()? {
                if roles.insert(role, spec).is_some() {
                    return Err(A::Error::custom("duplicate typography role"));
                }
            }
            Ok(roles)
        }
    }

    deserializer.deserialize_map(UniqueRoles)
}

impl TryFrom<TypographyAssignmentsInput> for TypographyAssignments {
    type Error = &'static str;

    fn try_from(input: TypographyAssignmentsInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if TypographyRole::ALL
            .iter()
            .any(|role| !input.roles.contains_key(role))
        {
            return Err("missing typography role");
        }
        Ok(Self {
            schema_version: input.schema_version,
            roles: input.roles,
        })
    }
}

impl TypographyAssignments {
    pub fn role(&self, role: TypographyRole) -> &TypographyRoleSpec {
        &self.roles[&role]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct TokenPath(String);

impl TryFrom<String> for TokenPath {
    type Error = &'static str;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        if path.is_empty() {
            Err("typography token path must not be empty")
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
            "../../../../conformance/typography/assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                serde_json::from_value::<TypographyAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for role in TypographyRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        serde_json::to_value(assignments.role(role)).unwrap(),
                        expected[name.as_str().unwrap()],
                        "{}: {name}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(assignments).unwrap(),
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

    #[test]
    fn duplicate_role_is_rejected_before_value_conversion() {
        let source = include_str!("../../../../conformance/typography/invalid-duplicate-role.json");
        let error = serde_json::from_str::<TypographyAssignments>(source).unwrap_err();
        assert!(
            error.to_string().contains("duplicate typography role"),
            "{error}"
        );
    }
}
