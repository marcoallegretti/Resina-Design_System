use crate::{
    ElevationDepthAssignments, KeyLight, MaterialFamily, OpaquePigmentProfiles,
    ShapeFallbackAssignments,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "SurfaceBandProfileInput")]
pub struct SurfaceBandProfile {
    edge_width: f64,
    highlight_width: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SurfaceBandProfileInput {
    edge_width: f64,
    highlight_width: f64,
}

impl TryFrom<SurfaceBandProfileInput> for SurfaceBandProfile {
    type Error = &'static str;
    fn try_from(input: SurfaceBandProfileInput) -> Result<Self, Self::Error> {
        if !input.edge_width.is_finite() || input.edge_width <= 0.0 {
            return Err("edgeWidth must be finite and positive");
        }
        if !input.highlight_width.is_finite() || input.highlight_width < 0.0 {
            return Err("highlightWidth must be finite and nonnegative");
        }
        Ok(Self {
            edge_width: input.edge_width,
            highlight_width: input.highlight_width,
        })
    }
}

impl SurfaceBandProfile {
    pub fn edge_width(&self) -> f64 {
        self.edge_width
    }
    pub fn highlight_width(&self) -> f64 {
        self.highlight_width
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FamilyBands {
    cast: SurfaceBandProfile,
    frost: SurfaceBandProfile,
    elastomer: SurfaceBandProfile,
    gel: SurfaceBandProfile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpaqueSurfaceAppearance {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    shape_assignments: ShapeFallbackAssignments,
    depth_assignments: ElevationDepthAssignments,
    pigment_profiles: OpaquePigmentProfiles,
    bands: FamilyBands,
    key_light: KeyLight,
}

impl OpaqueSurfaceAppearance {
    pub fn shape_assignments(&self) -> &ShapeFallbackAssignments {
        &self.shape_assignments
    }
    pub fn depth_assignments(&self) -> &ElevationDepthAssignments {
        &self.depth_assignments
    }
    pub fn pigment_profiles(&self) -> &OpaquePigmentProfiles {
        &self.pigment_profiles
    }
    pub fn key_light(&self) -> &KeyLight {
        &self.key_light
    }
    pub fn bands_for(&self, family: MaterialFamily) -> SurfaceBandProfile {
        match family {
            MaterialFamily::Cast => self.bands.cast,
            MaterialFamily::Frost => self.bands.frost,
            MaterialFamily::Elastomer => self.bands.elastomer,
            MaterialFamily::Gel => self.bands.gel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_profiles_reject_nonfinite_or_invalid_extents() {
        for edge_width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                SurfaceBandProfile::try_from(SurfaceBandProfileInput {
                    edge_width,
                    highlight_width: 1.0
                })
                .is_err()
            );
        }
        for highlight_width in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(
                SurfaceBandProfile::try_from(SurfaceBandProfileInput {
                    edge_width: 1.0,
                    highlight_width
                })
                .is_err()
            );
        }
    }

    #[test]
    fn authored_appearance_is_complete_and_has_dimensional_cues() {
        let source = include_str!("../../../../definitions/tier0-surface-appearance.json");
        let appearance: OpaqueSurfaceAppearance = serde_json::from_str(source).unwrap();
        let document: serde_json::Value = serde_json::from_str(source).unwrap();
        for (field, source) in [
            (
                "shapeAssignments",
                include_str!("../../../../definitions/tier0-shapes.json"),
            ),
            (
                "depthAssignments",
                include_str!("../../../../definitions/tier0-depth.json"),
            ),
            (
                "pigmentProfiles",
                include_str!("../../../../definitions/tier0-pigment.json"),
            ),
            (
                "keyLight",
                include_str!("../../../../definitions/key-light.json"),
            ),
        ] {
            assert_eq!(
                document[field],
                serde_json::from_str::<serde_json::Value>(source).unwrap(),
                "{field}"
            );
        }
        for family in [
            MaterialFamily::Cast,
            MaterialFamily::Frost,
            MaterialFamily::Elastomer,
            MaterialFamily::Gel,
        ] {
            let bands = appearance.bands_for(family);
            assert!(bands.edge_width() > 0.0 && bands.highlight_width() > 0.0);
            assert!(
                appearance
                    .pigment_profiles()
                    .profile_for(family)
                    .side_shade()
                    > 0.0
            );
            assert!(
                appearance
                    .pigment_profiles()
                    .profile_for(family)
                    .highlight_lift()
                    > 0.0
            );
        }
    }
}
