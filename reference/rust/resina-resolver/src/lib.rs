use resina_environment::{AccessibilityPreferences, QualityPolicy, RendererCapabilities};
use resina_model::FrostRepresentation;

pub fn resolve_frost_representation(
    capabilities: &RendererCapabilities,
    preferences: &AccessibilityPreferences,
    quality: QualityPolicy,
) -> FrostRepresentation {
    if preferences.reduced_transparency
        || preferences.high_contrast
        || !capabilities.translucent_surfaces
    {
        return FrostRepresentation::OpaqueDimensional;
    }
    if quality == QualityPolicy::Economy {
        return FrostRepresentation::TranslucentPigmented;
    }
    if capabilities.backdrop_effect && capabilities.backdrop_blur {
        if capabilities.shaped_backdrop && quality == QualityPolicy::Full {
            FrostRepresentation::ShapedBackdrop
        } else {
            FrostRepresentation::RegularBackdrop
        }
    } else {
        FrostRepresentation::TranslucentPigmented
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn frost_fallback_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/frost-fallback-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let capabilities: RendererCapabilities =
                serde_json::from_value(vector["capabilities"].clone()).unwrap();
            let preferences: AccessibilityPreferences =
                serde_json::from_value(vector["preferences"].clone()).unwrap();
            let quality: QualityPolicy = serde_json::from_value(vector["quality"].clone()).unwrap();
            let actual = resolve_frost_representation(&capabilities, &preferences, quality);
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                vector["expected"],
                "{}",
                vector["name"]
            );
        }
    }
}
