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

fn token_path<E: serde::de::Error>(path: String) -> Result<String, E> {
    if path.is_empty() {
        return Err(E::custom("token path must not be empty"));
    }
    Ok(path)
}

impl<'de> Deserialize<'de> for SizeTokenPaths {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, SizeTokenPathsInput>(deserializer)?;
        Ok(Self {
            width: token_path(input.width)?,
            height: token_path(input.height)?,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ToggleComponent {
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleParts {
    pub track_size: SizeTokenPaths,
    pub thumb_size: SizeTokenPaths,
    pub thumb_inset: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleLayoutProfile {
    schema_version: String,
    component: ToggleComponent,
    parts: DensityModes<ToggleParts>,
}

impl ToggleLayoutProfile {
    pub fn parts(&self) -> &DensityModes<ToggleParts> {
        &self.parts
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TogglePartsInput {
    track_size: SizeTokenPaths,
    thumb_size: SizeTokenPaths,
    thumb_inset: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToggleProfileInput {
    schema_version: String,
    component: ToggleComponent,
    parts: DensityModes<ToggleParts>,
}

impl<'de> Deserialize<'de> for ToggleParts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, TogglePartsInput>(deserializer)?;
        Ok(Self {
            track_size: input.track_size,
            thumb_size: input.thumb_size,
            thumb_inset: token_path(input.thumb_inset)?,
        })
    }
}

impl<'de> Deserialize<'de> for ToggleLayoutProfile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = crate::deserialize_assignment_object::<D, ToggleProfileInput>(deserializer)?;
        if input.schema_version != "0.1.0" {
            return Err(D::Error::custom("schemaVersion must be 0.1.0"));
        }
        Ok(Self {
            schema_version: input.schema_version,
            component: input.component,
            parts: input.parts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DENSITY_MODES;
    use serde::de::DeserializeOwned;
    use serde_json::Value;
    use std::{collections::BTreeSet, fmt::Debug};

    const PROFILE: &str = include_str!("../../../../definitions/layout/command.json");
    const CASES: &str = include_str!("../../../../conformance/layout/command-profile-cases.json");
    const TOGGLE_PROFILE: &str = include_str!("../../../../definitions/layout/toggle.json");
    const TOGGLE_CASES: &str =
        include_str!("../../../../conformance/layout/toggle-profile-cases.json");

    fn assert_cases_accept_and_reject_for_their_stated_reason<T>(cases: &str)
    where
        T: DeserializeOwned + Serialize + PartialEq + Debug,
    {
        let cases: Vec<Value> = serde_json::from_str(cases).unwrap();
        for case in cases {
            let source = case["document"].to_string();
            let results = [
                serde_json::from_str::<T>(&source),
                serde_json::from_value::<T>(case["document"].clone()),
            ];
            for result in results {
                match case.get("error") {
                    None => {
                        let profile = result.unwrap();
                        let published = serde_json::to_value(&profile).unwrap();
                        assert_eq!(serde_json::from_value::<T>(published).unwrap(), profile);
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
    fn profile_cases_accept_and_reject_for_their_stated_reason() {
        assert_cases_accept_and_reject_for_their_stated_reason::<ComponentLayoutProfile>(CASES);
    }

    #[test]
    fn toggle_profile_cases_accept_and_reject_for_their_stated_reason() {
        assert_cases_accept_and_reject_for_their_stated_reason::<ToggleLayoutProfile>(TOGGLE_CASES);
    }

    #[test]
    fn authored_command_profile_names_one_token_per_density_and_extent() {
        let profile: ComponentLayoutProfile = serde_json::from_str(PROFILE).unwrap();
        assert_eq!(profile.component(), LayoutComponent::Command);
        let mut paths = BTreeSet::new();
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
    fn authored_toggle_profile_names_one_token_per_density_and_part_dimension() {
        let profile: ToggleLayoutProfile = serde_json::from_str(TOGGLE_PROFILE).unwrap();
        let mut paths = BTreeSet::new();
        for density in DENSITY_MODES {
            let parts = profile.parts().get(density);
            for path in [
                &parts.track_size.width,
                &parts.track_size.height,
                &parts.thumb_size.width,
                &parts.thumb_size.height,
                &parts.thumb_inset,
            ] {
                assert!(paths.insert(path.clone()), "{path}");
            }
        }
        assert_eq!(paths.len(), 20);
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

    #[test]
    fn duplicate_toggle_profile_members_are_rejected() {
        let source = TOGGLE_PROFILE.replace("\r\n", "\n");
        for (member, duplicate) in [
            (
                "\"component\": \"toggle\"",
                "\"component\": \"toggle\", \"component\": \"toggle\"",
            ),
            (
                "\"compact\": {",
                "\"compact\": {\"trackSize\": {\"width\": \"a\", \"height\": \"b\"}, \"thumbSize\": {\"width\": \"c\", \"height\": \"d\"}, \"thumbInset\": \"e\"}, \"compact\": {",
            ),
            (
                "\"thumbInset\": \"toggle.thumb.standard.inset\"",
                "\"thumbInset\": \"toggle.thumb.standard.inset\", \"thumb\\u0049nset\": \"toggle.thumb.standard.inset\"",
            ),
            (
                "\"height\": \"toggle.track.compact.height\"",
                "\"height\": \"toggle.track.compact.height\", \"height\": \"toggle.track.compact.height\"",
            ),
        ] {
            assert_eq!(source.matches(member).count(), 1, "{member}");
            let changed = source.replacen(member, duplicate, 1);
            let error = serde_json::from_str::<ToggleLayoutProfile>(&changed).unwrap_err();
            assert!(error.to_string().contains("duplicate"), "{member}: {error}");
        }
    }
}
