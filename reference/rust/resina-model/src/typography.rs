use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor, value::StringDeserializer},
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypographyRoleSpec {
    family_role: FontFamilyRole,
    font_size: TokenPath,
    font_weight: TokenPath,
    line_height: TokenPath,
    letter_spacing: TokenPath,
    minimum_text_scale: f64,
}

impl<'de> Deserialize<'de> for TypographyRoleSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input =
            crate::deserialize_assignment_object::<D, TypographyRoleSpecInput>(deserializer)?;
        Self::try_from(input).map_err(D::Error::custom)
    }
}

fn deserialize_family_role<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<FontFamilyRole, D::Error> {
    FontFamilyRole::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(
        deserializer,
    )?))
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypographyRoleSpecInput {
    #[serde(deserialize_with = "deserialize_family_role")]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypographyAssignments {
    schema_version: String,
    roles: BTreeMap<TypographyRole, TypographyRoleSpec>,
}

impl<'de> Deserialize<'de> for TypographyAssignments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input =
            crate::deserialize_assignment_object::<D, TypographyAssignmentsInput>(deserializer)?;
        Self::try_from(input).map_err(D::Error::custom)
    }
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
            for result in [
                serde_json::from_str::<TypographyAssignments>(&vector["document"].to_string()),
                serde_json::from_value::<TypographyAssignments>(vector["document"].clone()),
            ] {
                if let Some(expected) = vector.get("expected") {
                    let assignments = result.unwrap();
                    for role in TypographyRole::ALL {
                        let name = serde_json::to_value(role).unwrap();
                        assert_role(assignments.role(role), &expected[name.as_str().unwrap()]);
                    }
                    assert_eq!(
                        serde_json::to_value(assignments).unwrap(),
                        vector["document"]
                    );
                } else {
                    let error = result.unwrap_err();
                    assert!(
                        error
                            .to_string()
                            .contains(vector["error"].as_str().unwrap()),
                        "{}: {error}",
                        vector["name"]
                    );
                }
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

    fn assert_role(spec: &TypographyRoleSpec, expected: &Value) {
        assert_eq!(
            serde_json::to_value(spec.family_role()).unwrap(),
            expected["familyRole"]
        );
        assert_eq!(
            spec.font_size_path(),
            expected["fontSize"].as_str().unwrap()
        );
        assert_eq!(
            spec.font_weight_path(),
            expected["fontWeight"].as_str().unwrap()
        );
        assert_eq!(
            spec.line_height_path(),
            expected["lineHeight"].as_str().unwrap()
        );
        assert_eq!(
            spec.letter_spacing_path(),
            expected["letterSpacing"].as_str().unwrap()
        );
        assert_eq!(
            spec.minimum_text_scale(),
            expected["minimumTextScale"].as_f64().unwrap()
        );
        assert_eq!(serde_json::to_value(spec).unwrap(), *expected);
    }

    #[test]
    fn standalone_roles_enforce_the_same_source_shapes() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/assignment-vectors.json"
        ))
        .unwrap();
        for (role, expected) in vectors[0]["document"]["roles"].as_object().unwrap() {
            for result in [
                serde_json::from_str::<TypographyRoleSpec>(&expected.to_string()),
                serde_json::from_value::<TypographyRoleSpec>(expected.clone()),
            ] {
                assert_role(&result.unwrap(), expected);
            }
            for vector in &vectors {
                let name = vector["name"].as_str().unwrap();
                if name.starts_with(&format!("typography source shape role {role} "))
                    || name == format!("typography source shape family {role} tagged")
                {
                    let document = &vector["document"]["roles"][role];
                    for result in [
                        serde_json::from_str::<TypographyRoleSpec>(&document.to_string()),
                        serde_json::from_value::<TypographyRoleSpec>(document.clone()),
                    ] {
                        let error = result.unwrap_err();
                        assert!(
                            error
                                .to_string()
                                .contains(vector["error"].as_str().unwrap()),
                            "{name}: {error}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn duplicate_and_escaped_assignment_members_preserve_validation() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/assignment-vectors.json"
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
                TypographyRole::ALL
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
        ] {
            for field in fields {
                let member = format!("\"{field}\":{}", object[&field]);
                let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
                let escaped_member = format!("\"{escaped}\":{}", object[&field]);
                for duplicate in [&member, &escaped_member] {
                    let changed = source.replacen(&member, &format!("{member},{duplicate}"), 1);
                    assert!(changed.len() > source.len());
                    let error =
                        serde_json::from_str::<TypographyAssignments>(&changed).unwrap_err();
                    assert!(error.to_string().contains("duplicate"), "{field}: {error}");
                }
                let changed = source.replacen(&member, &escaped_member, 1);
                let assignment: TypographyAssignments = serde_json::from_str(&changed).unwrap();
                assert_eq!(serde_json::to_value(assignment).unwrap(), *original);
            }
        }
    }

    #[test]
    fn duplicate_and_escaped_role_fields_preserve_validation() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/typography/assignment-vectors.json"
        ))
        .unwrap();
        let original = &vectors[0]["document"]["roles"]["body"];
        let source = original.to_string();
        for field in [
            "familyRole",
            "fontSize",
            "fontWeight",
            "lineHeight",
            "letterSpacing",
            "minimumTextScale",
        ] {
            let member = format!("\"{field}\":{}", original[field]);
            let escaped = format!("\\u{:04x}{}", field.as_bytes()[0], &field[1..]);
            let escaped_member = format!("\"{escaped}\":{}", original[field]);
            for duplicate in [&member, &escaped_member] {
                let changed = source.replacen(&member, &format!("{member},{duplicate}"), 1);
                assert!(changed.len() > source.len());
                let error = serde_json::from_str::<TypographyRoleSpec>(&changed).unwrap_err();
                assert!(error.to_string().contains("duplicate"), "{field}: {error}");
            }
            let changed = source.replacen(&member, &escaped_member, 1);
            let spec: TypographyRoleSpec = serde_json::from_str(&changed).unwrap();
            assert_role(&spec, original);
        }
    }
}
