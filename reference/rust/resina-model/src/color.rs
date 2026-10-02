use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColorRole {
    #[serde(rename = "accent.primary")]
    AccentPrimary,
    #[serde(rename = "accent.secondary")]
    AccentSecondary,
    #[serde(rename = "accent.tertiary")]
    AccentTertiary,
    #[serde(rename = "surface.base")]
    SurfaceBase,
    #[serde(rename = "surface.low")]
    SurfaceLow,
    #[serde(rename = "surface.high")]
    SurfaceHigh,
    #[serde(rename = "surface.chrome")]
    SurfaceChrome,
    #[serde(rename = "content.primary")]
    ContentPrimary,
    #[serde(rename = "content.secondary")]
    ContentSecondary,
    #[serde(rename = "content.muted")]
    ContentMuted,
    #[serde(rename = "content.inverse")]
    ContentInverse,
    #[serde(rename = "outline")]
    Outline,
    #[serde(rename = "outline.strong")]
    OutlineStrong,
    #[serde(rename = "status.error")]
    StatusError,
    #[serde(rename = "status.warning")]
    StatusWarning,
    #[serde(rename = "status.success")]
    StatusSuccess,
    #[serde(rename = "status.info")]
    StatusInfo,
    #[serde(rename = "focus")]
    Focus,
    #[serde(rename = "selection")]
    Selection,
}

impl ColorRole {
    pub const ALL: [Self; 19] = [
        Self::AccentPrimary,
        Self::AccentSecondary,
        Self::AccentTertiary,
        Self::SurfaceBase,
        Self::SurfaceLow,
        Self::SurfaceHigh,
        Self::SurfaceChrome,
        Self::ContentPrimary,
        Self::ContentSecondary,
        Self::ContentMuted,
        Self::ContentInverse,
        Self::Outline,
        Self::OutlineStrong,
        Self::StatusError,
        Self::StatusWarning,
        Self::StatusSuccess,
        Self::StatusInfo,
        Self::Focus,
        Self::Selection,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorAssignments {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    roles: ColorRoleTokens,
}

impl ColorAssignments {
    pub fn token_path_for(&self, role: ColorRole) -> &str {
        match role {
            ColorRole::AccentPrimary => self.roles.accent_primary.as_str(),
            ColorRole::AccentSecondary => self.roles.accent_secondary.as_str(),
            ColorRole::AccentTertiary => self.roles.accent_tertiary.as_str(),
            ColorRole::SurfaceBase => self.roles.surface_base.as_str(),
            ColorRole::SurfaceLow => self.roles.surface_low.as_str(),
            ColorRole::SurfaceHigh => self.roles.surface_high.as_str(),
            ColorRole::SurfaceChrome => self.roles.surface_chrome.as_str(),
            ColorRole::ContentPrimary => self.roles.content_primary.as_str(),
            ColorRole::ContentSecondary => self.roles.content_secondary.as_str(),
            ColorRole::ContentMuted => self.roles.content_muted.as_str(),
            ColorRole::ContentInverse => self.roles.content_inverse.as_str(),
            ColorRole::Outline => self.roles.outline.as_str(),
            ColorRole::OutlineStrong => self.roles.outline_strong.as_str(),
            ColorRole::StatusError => self.roles.status_error.as_str(),
            ColorRole::StatusWarning => self.roles.status_warning.as_str(),
            ColorRole::StatusSuccess => self.roles.status_success.as_str(),
            ColorRole::StatusInfo => self.roles.status_info.as_str(),
            ColorRole::Focus => self.roles.focus.as_str(),
            ColorRole::Selection => self.roles.selection.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpaqueColorAssignments {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    #[serde(deserialize_with = "deserialize_unique_opaque_roles")]
    roles: BTreeMap<ColorRole, TokenPath>,
}

impl OpaqueColorAssignments {
    pub fn token_path_for(&self, role: ColorRole) -> Option<&str> {
        self.roles.get(&role).map(TokenPath::as_str)
    }
}

fn deserialize_unique_opaque_roles<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<ColorRole, TokenPath>, D::Error> {
    struct UniqueOpaqueRoles;

    impl<'de> Visitor<'de> for UniqueOpaqueRoles {
        type Value = BTreeMap<ColorRole, TokenPath>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an opaque color role mapping without duplicate members")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut roles = BTreeMap::new();
            while let Some((role, path)) = access.next_entry()? {
                if roles.insert(role, path).is_some() {
                    return Err(A::Error::custom("duplicate opaque color role"));
                }
            }
            Ok(roles)
        }
    }

    deserializer.deserialize_map(UniqueOpaqueRoles)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorRoleTokens {
    #[serde(rename = "accent.primary")]
    accent_primary: TokenPath,
    #[serde(rename = "accent.secondary")]
    accent_secondary: TokenPath,
    #[serde(rename = "accent.tertiary")]
    accent_tertiary: TokenPath,
    #[serde(rename = "surface.base")]
    surface_base: TokenPath,
    #[serde(rename = "surface.low")]
    surface_low: TokenPath,
    #[serde(rename = "surface.high")]
    surface_high: TokenPath,
    #[serde(rename = "surface.chrome")]
    surface_chrome: TokenPath,
    #[serde(rename = "content.primary")]
    content_primary: TokenPath,
    #[serde(rename = "content.secondary")]
    content_secondary: TokenPath,
    #[serde(rename = "content.muted")]
    content_muted: TokenPath,
    #[serde(rename = "content.inverse")]
    content_inverse: TokenPath,
    #[serde(rename = "outline")]
    outline: TokenPath,
    #[serde(rename = "outline.strong")]
    outline_strong: TokenPath,
    #[serde(rename = "status.error")]
    status_error: TokenPath,
    #[serde(rename = "status.warning")]
    status_warning: TokenPath,
    #[serde(rename = "status.success")]
    status_success: TokenPath,
    #[serde(rename = "status.info")]
    status_info: TokenPath,
    #[serde(rename = "focus")]
    focus: TokenPath,
    #[serde(rename = "selection")]
    selection: TokenPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
struct TokenPath(String);

impl TokenPath {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TokenPath {
    type Error = &'static str;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        if path.is_empty() {
            Err("color role token path must not be empty")
        } else {
            Ok(Self(path))
        }
    }
}

fn deserialize_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version == "0.1.0" {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 0.1.0"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn role_assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/role-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<ColorAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for role in ColorRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        assignments.token_path_for(role),
                        expected[name.as_str().unwrap()].as_str().unwrap(),
                        "{}: {name}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(&assignments).unwrap(),
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

    #[test]
    fn opaque_assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/opaque-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                serde_json::from_value::<OpaqueColorAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for role in ColorRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        assignments.token_path_for(role),
                        expected[name.as_str().unwrap()].as_str(),
                        "{}: {name}",
                        vector["name"]
                    );
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

    #[test]
    fn opaque_assignment_rejects_duplicate_role_before_map_conversion() {
        let source = r#"{"schemaVersion":"0.1.0","roles":{"focus":"palette.first","focus":"palette.second"}}"#;
        let error = serde_json::from_str::<OpaqueColorAssignments>(source).unwrap_err();
        assert!(error.to_string().contains("duplicate opaque color role"));
    }
}
