mod toggle_part_paint;
pub use toggle_part_paint::{
    TogglePart, TogglePartPaintError, TogglePartPaintInput, TogglePartPaintIr,
    resolve_toggle_part_paint, resolve_toggle_part_paint_source,
};
mod control_paint;
mod toggle_layout;
pub use toggle_layout::{
    ToggleLayoutError, ToggleLayoutInput, ToggleLayoutIr, resolve_toggle_layout,
    resolve_toggle_layout_source,
};
mod toggle_accessibility;
pub use toggle_accessibility::{
    ToggleAccessibilityAction, ToggleAccessibilityError, ToggleAccessibilityInput,
    ToggleAccessibilityIr, ToggleAccessibilityState, resolve_toggle_accessibility,
};
mod toggle_states;
pub use toggle_states::{ToggleStatesError, resolve_toggle_states, resolve_toggle_states_source};

mod toggle_activation;
pub use toggle_activation::{
    ToggleActivationError, ToggleActivationResult, resolve_toggle_activation,
    resolve_toggle_activation_source,
};

mod command_snapshot;
pub use command_snapshot::{
    CommandSnapshot, CommandSnapshotError, CommandSnapshotInput, resolve_command_snapshot,
};

mod command_states;
pub use command_states::{
    CommandStatesError, resolve_command_states, resolve_command_states_source,
};

mod command_accessibility;
pub use command_accessibility::{
    CommandAccessibilityAction, CommandAccessibilityError, CommandAccessibilityInput,
    CommandAccessibilityIr, CommandAccessibilityState, resolve_command_accessibility,
};

mod command_label;
pub use command_label::{
    CommandContentError, CommandLabelError, CommandLabelInput, CommandLabelIr, LabelMeasureInput,
    resolve_command_label,
};

mod command_motion;
pub use command_motion::{
    CommandMotionChannel, CommandMotionChannels, CommandMotionInput, CommandMotionIr,
    CommandMotionPolicy, CommandProjection, resolve_command_motion, resolve_command_motion_source,
};

mod command_paint;
pub use command_paint::{
    CommandPaintError, CommandPaintInput, CommandPaintIr, resolve_command_paint,
    resolve_command_paint_source,
};

use resina_environment::{AccessibilityPreferences, QualityPolicy, RendererCapabilities};
use resina_model::{ColorAssignments, ColorRole, FrostRepresentation};
use resina_tokens::{ResolvedToken, validate_resolved_value};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

mod activation;
pub use activation::{
    ActivationError, ActivationResult, CaptureChange, resolve_activation, resolve_activation_source,
};

mod edge_contrast;
mod elevation;
mod extruded_contour;
mod focus_indicator;
mod focus_ir;
mod frost_legibility;
mod frost_surface_readability;
mod geometry;
mod inset_contour;
mod key_light;
mod spatial;
mod srgb_input;
pub use edge_contrast::{
    EdgeContrastError, EdgeContrastResult, resolve_edge_contrast, resolve_edge_contrast_source,
};
pub use elevation::{
    ElevationDepthError, ElevationDepthResolutionError, ElevationDepthResolutionErrorKind,
    ElevationDepthResult, resolve_elevation_depth, resolve_elevation_depth_source,
};
pub use extruded_contour::{
    ExtrudedContourError, ExtrudedContourResult, PlacedContour, resolve_extruded_contour,
    resolve_extruded_contour_source,
};
pub use focus_indicator::{
    FocusIndicatorError, FocusIndicatorResult, resolve_focus_indicator,
    resolve_focus_indicator_source,
};
pub use focus_ir::{
    FocusIndicatorIr, FocusIrError, FocusIrInput, FocusRingGeometry, resolve_focus_ir,
    resolve_focus_ir_source,
};
pub use frost_legibility::{
    FrostLegibilityError, FrostLegibilityResult, resolve_frost_legibility,
    resolve_frost_legibility_source,
};
pub use frost_surface_readability::{
    FrostSurfaceReadabilityError, FrostSurfaceReadabilityResult, resolve_frost_surface_readability,
    resolve_frost_surface_readability_source,
};
pub use geometry::{CornerGeometryError, normalize_corner_radii};
pub use inset_contour::{
    InsetContourError, InsetContourResult, resolve_inset_contour, resolve_inset_contour_source,
};
pub use key_light::{
    EdgeHighlightWeights, KeyLightError, KeyLightResult, resolve_key_light,
    resolve_key_light_source,
};
pub use resina_color::{
    ColorFallbackError, ContrastError, SrgbFallback, opaque_contrast_ratio, resolve_srgb_fallback,
};
pub use spatial::{SpatialResolutionError, SpatialResolutionErrorKind, resolve_semantic_space};
mod surface;
pub use surface::{BoundSurface, SurfaceBindingError, bind_surface};
mod surface_readability;
pub use surface_readability::{
    SurfaceReadabilityError, SurfaceReadabilityResult, resolve_surface_readability,
    resolve_surface_readability_source,
};
mod opaque_paint;
mod opaque_pigment;
mod opaque_surface;
mod surface_paint;
pub use opaque_paint::SurfacePaintError;
pub use opaque_pigment::{
    OpaquePigmentError, OpaquePigmentResult, resolve_opaque_pigment, resolve_opaque_pigment_source,
};
pub use opaque_surface::{
    OpaqueSurfaceError, OpaqueSurfaceGeometry, OpaqueSurfaceInput, OpaqueSurfaceIr,
    resolve_opaque_surface, resolve_opaque_surface_source,
};
pub use surface_paint::{
    SurfacePaintInput, SurfacePaintIr, SurfacePaintResolutionError, resolve_surface_paint,
    resolve_surface_paint_source,
};
mod color_fallback;
pub use color_fallback::{
    ColorRoleFallbackError, ColorRoleFallbackErrorKind, resolve_semantic_color_fallbacks,
};
mod opaque_color;
pub use opaque_color::{
    OpaqueColorResolutionError, OpaqueColorResolutionErrorKind,
    resolve_semantic_opaque_color_fallbacks,
};
mod headless;
pub use headless::{
    HeadlessBindingError, HeadlessResolution, HeadlessResolutionError, resolve_headless_source,
};
mod scenario;
pub use scenario::{SurfaceScenarioError, resolve_surface_scenario_source};
mod shape_fallback;
pub use shape_fallback::{
    ShapeFallbackError, ShapeFallbackResult, ShapeFallbackSourceError, resolve_shape_fallback,
    resolve_shape_fallback_source,
};
mod focus_traversal;
mod hit_region;
pub use focus_traversal::{
    FocusDirection, FocusTarget, FocusTraversalError, FocusTraversalInput, FocusTraversalResult,
    resolve_focus_traversal, resolve_focus_traversal_source,
};
mod target;
pub use hit_region::{
    HitRegionError, HitRegionInput, HitRegionIr, SurfaceHitRegionInput, resolve_hit_region,
    resolve_hit_region_source, resolve_surface_hit_region,
};
pub use target::{MinimumHitTarget, resolve_minimum_hit_target};
mod theme;
pub use theme::{
    CompiledTheme, ThemeCompilationError, compile_theme_source, compile_theme_source_with_sources,
};
mod theme_request;
pub use theme_request::{ThemeResolutionError, resolve_theme_request_source};
mod typography;
pub use typography::{
    PxDimension, ResolvedTypography, TypographyResolutionError, TypographyResolutionErrorKind,
    resolve_semantic_typography,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorResolutionErrorKind {
    MissingToken,
    WrongTokenType,
    InvalidColorValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorResolutionError {
    pub kind: ColorResolutionErrorKind,
    pub role: ColorRole,
    pub token_path: String,
    pub detail: Option<String>,
}

impl fmt::Display for ColorResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} for {:?} at {}",
            self.kind, self.role, self.token_path
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ColorResolutionError {}

pub fn resolve_semantic_colors(
    assignments: &ColorAssignments,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<BTreeMap<ColorRole, Value>, Vec<ColorResolutionError>> {
    let mut colors = BTreeMap::new();
    let mut errors = Vec::new();
    for role in ColorRole::ALL {
        let path = assignments.token_path_for(role);
        let result = match tokens.get(path) {
            None => Err((ColorResolutionErrorKind::MissingToken, None)),
            Some(token) if token.token_type != "color" => Err((
                ColorResolutionErrorKind::WrongTokenType,
                Some(token.token_type.clone()),
            )),
            Some(token) => validate_resolved_value("color", &token.value)
                .map(|()| token.value.clone())
                .map_err(|error| {
                    (
                        ColorResolutionErrorKind::InvalidColorValue,
                        Some(error.to_string()),
                    )
                }),
        };
        match result {
            Ok(value) => {
                colors.insert(role, value);
            }
            Err((kind, detail)) => errors.push(ColorResolutionError {
                kind,
                role,
                token_path: path.to_owned(),
                detail,
            }),
        }
    }
    if errors.is_empty() {
        Ok(colors)
    } else {
        Err(errors)
    }
}

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
    use resina_tokens::resolve_token_document;

    #[test]
    fn semantic_colors_bind_resolved_document_aliases() {
        let assignments: ColorAssignments = serde_json::from_value(
            serde_json::from_str::<Value>(include_str!(
                "../../../../conformance/color/role-assignment-vectors.json"
            ))
            .unwrap()[0]["document"]
                .clone(),
        )
        .unwrap();
        let source = serde_json::json!({
            "palette": {
                "$type": "color",
                "sample0": {"$value": {"colorSpace": "srgb", "components": [0, 0, 0]}},
                "sample1": {"$value": {"colorSpace": "srgb", "components": [1, 1, 1]}},
                "sample2": {"$value": "{palette.sample0}"}
            }
        });
        let tokens = resolve_token_document(&source).unwrap();
        let colors = resolve_semantic_colors(&assignments, &tokens).unwrap();
        assert_eq!(colors.len(), ColorRole::ALL.len());
        assert_eq!(
            colors[&ColorRole::AccentTertiary],
            colors[&ColorRole::AccentPrimary]
        );
        assert_eq!(
            colors[&ColorRole::AccentSecondary],
            tokens["palette.sample1"].value
        );
    }

    #[test]
    fn semantic_color_resolution_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/color/resolution-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let assignments: ColorAssignments =
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
            let result = resolve_semantic_colors(&assignments, &tokens);
            if let Some(expected) = vector.get("expected") {
                let actual = result.unwrap();
                assert_eq!(actual.len(), ColorRole::ALL.len(), "{}", vector["name"]);
                for role in ColorRole::ALL {
                    let name = serde_json::to_value(role).unwrap();
                    assert_eq!(
                        actual[&role],
                        expected[name.as_str().unwrap()],
                        "{}: {name}",
                        vector["name"]
                    );
                }
            } else {
                let errors = result.unwrap_err();
                let actual: Vec<_> = errors
                    .iter()
                    .map(|error| {
                        serde_json::json!({
                            "kind": format!("{:?}", error.kind),
                            "role": error.role,
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
