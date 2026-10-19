use crate::{DensityModes, SpatialRole};
use resina_environment::DensityPreference;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutComponent {
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProfilePadding {
    pub start: SpatialRole,
    pub end: SpatialRole,
    pub top: SpatialRole,
    pub bottom: SpatialRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SizeTokenPaths {
    pub width: String,
    pub height: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExtent {
    pub minimum_size: SizeTokenPaths,
    pub maximum_size: SizeTokenPaths,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLayoutProfile {
    schema_version: String,
    component: LayoutComponent,
    padding: ProfilePadding,
    extents: DensityModes<ProfileExtent>,
}

impl ComponentLayoutProfile {
    pub fn component(&self) -> LayoutComponent {
        self.component
    }

    pub fn padding(&self) -> ProfilePadding {
        self.padding
    }

    pub fn extent(&self, density: DensityPreference) -> &ProfileExtent {
        self.extents.get(density)
    }

    pub fn extents(&self) -> &DensityModes<ProfileExtent> {
        &self.extents
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfileInput {
    schema_version: String,
    component: LayoutComponent,
    padding: ProfilePadding,
    extents: DensityModes<ProfileExtent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaddingInput {
    start: SpatialRole,
    end: SpatialRole,
    top: SpatialRole,
    bottom: SpatialRole,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExtentInput {
    minimum_size: SizeTokenPaths,
    maximum_size: SizeTokenPaths,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SizeTokenPathsInput {
    width: String,
    height: String,
}

impl<'de> Deserialize<'de> for ProfilePadding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, PaddingInput>(deserializer)?;
        if input.start != SpatialRole::ControlInsetInline
            || input.end != SpatialRole::ControlInsetInline
        {
            return Err(D::Error::custom(
                "padding start and end must use space.control.inset.inline",
            ));
        }
        if input.top != SpatialRole::ControlInsetBlock
            || input.bottom != SpatialRole::ControlInsetBlock
        {
            return Err(D::Error::custom(
                "padding top and bottom must use space.control.inset.block",
            ));
        }
        Ok(Self {
            start: input.start,
            end: input.end,
            top: input.top,
            bottom: input.bottom,
        })
    }
}

impl<'de> Deserialize<'de> for SizeTokenPaths {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, SizeTokenPathsInput>(deserializer)?;
        if input.width.is_empty() || input.height.is_empty() {
            return Err(D::Error::custom("extent token path must not be empty"));
        }
        Ok(Self {
            width: input.width,
            height: input.height,
        })
    }
}

impl<'de> Deserialize<'de> for ProfileExtent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, ExtentInput>(deserializer)?;
        Ok(Self {
            minimum_size: input.minimum_size,
            maximum_size: input.maximum_size,
        })
    }
}

impl<'de> Deserialize<'de> for ComponentLayoutProfile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, ProfileInput>(deserializer)?;
        if input.schema_version != "0.1.0" {
            return Err(D::Error::custom("schemaVersion must be 0.1.0"));
        }
        Ok(Self {
            schema_version: input.schema_version,
            component: input.component,
            padding: input.padding,
            extents: input.extents,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DENSITY_MODES;
    use serde_json::Value;

    const PROFILE: &str = include_str!("../../../../definitions/layout/command.json");
    const CASES: &str = include_str!("../../../../conformance/layout/command-profile-cases.json");

    #[test]
    fn profile_cases_accept_and_reject_for_their_stated_reason() {
        let cases: Vec<Value> = serde_json::from_str(CASES).unwrap();
        for case in cases {
            let source = case["document"].to_string();
            let results = [
                serde_json::from_str::<ComponentLayoutProfile>(&source),
                serde_json::from_value::<ComponentLayoutProfile>(case["document"].clone()),
            ];
            for result in results {
                match case.get("error") {
                    None => {
                        let profile = result.unwrap();
                        let published = serde_json::to_value(&profile).unwrap();
                        assert_eq!(
                            serde_json::from_value::<ComponentLayoutProfile>(published).unwrap(),
                            profile
                        );
                    }
                    Some(error) => {
                        let message = result.unwrap_err().to_string();
                        assert!(
                            message.contains(error.as_str().unwrap()),
                            "{}: {message}",
                            case["name"]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn authored_command_profile_names_one_token_per_density_and_extent() {
        let profile: ComponentLayoutProfile = serde_json::from_str(PROFILE).unwrap();
        assert_eq!(profile.component(), LayoutComponent::Command);
        let mut paths = std::collections::BTreeSet::new();
        for density in DENSITY_MODES {
            let extent = profile.extent(density);
            for size in [&extent.minimum_size, &extent.maximum_size] {
                assert!(paths.insert(size.width.clone()), "{}", size.width);
                assert!(paths.insert(size.height.clone()), "{}", size.height);
            }
        }
        assert_eq!(paths.len(), 16);
    }

    #[test]
    fn duplicate_profile_members_are_rejected() {
        let source = PROFILE.replace("\r\n", "\n");
        for (member, duplicate) in [
            (
                "\"component\": \"command\"",
                "\"component\": \"command\", \"component\": \"command\"",
            ),
            (
                "\"start\": \"space.control.inset.inline\"",
                "\"start\": \"space.control.inset.inline\", \"\\u0073tart\": \"space.control.inset.inline\"",
            ),
            (
                "\"compact\": {",
                "\"compact\": {\"minimumSize\": {\"width\": \"a\", \"height\": \"b\"}, \"maximumSize\": {\"width\": \"c\", \"height\": \"d\"}}, \"compact\": {",
            ),
            (
                "\"width\": \"command.extent.compact.minimum.width\",",
                "\"width\": \"command.extent.compact.minimum.width\", \"width\": \"command.extent.compact.minimum.width\",",
            ),
        ] {
            assert_eq!(source.matches(member).count(), 1, "{member}");
            let changed = source.replacen(member, duplicate, 1);
            let error = serde_json::from_str::<ComponentLayoutProfile>(&changed).unwrap_err();
            assert!(error.to_string().contains("duplicate"), "{member}: {error}");
        }
    }
}
