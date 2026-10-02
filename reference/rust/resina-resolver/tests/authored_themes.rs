use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, FrostRepresentation, MaterialFamily, MaterialRole, SurfaceIntent, TypographyRole,
};
use resina_resolver::{
    HeadlessResolution, bind_surface, compile_theme_source_with_sources, opaque_contrast_ratio,
    resolve_edge_contrast, resolve_frost_legibility,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const FOUNDATION: &str = include_str!("../../../../tokens/foundation.json");
const LIGHT: &str = include_str!("../../../../tokens/themes/light.json");
const DARK: &str = include_str!("../../../../tokens/themes/dark.json");
const ENVIRONMENT: &str = include_str!("../../../../conformance/headless/valid-request.json");
const TIER_ZERO: &str =
    include_str!("../../../../conformance/environment/valid-minimal-capabilities.json");

fn resolve(source: &str, environment: &EnvironmentSnapshot) -> HeadlessResolution {
    let sources = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    compile_theme_source_with_sources(source, &sources)
        .unwrap()
        .resolve(environment)
        .unwrap()
}

fn environment_for_scale(scale: f64) -> EnvironmentSnapshot {
    let request: Value = serde_json::from_str(ENVIRONMENT).unwrap();
    let mut environment = request["environment"].clone();
    environment["textScale"] = scale.into();
    serde_json::from_value(environment).unwrap()
}

fn minimum_contrast(result: &HeadlessResolution, front: ColorRole, back: ColorRole, minimum: f64) {
    let colors = result.opaque_color_fallbacks();
    let ratio = opaque_contrast_ratio(&colors[&front], &colors[&back]).unwrap();
    assert!(
        ratio >= minimum,
        "{front:?} over {back:?}: {ratio} < {minimum}"
    );
}

fn verify_srgb_hex(node: &Value) -> usize {
    let Some(object) = node.as_object() else {
        return 0;
    };
    if object.get("$type").and_then(Value::as_str) == Some("color") {
        let value = &object["$value"];
        assert_eq!(value["colorSpace"], "srgb");
        let hex = value["hex"].as_str().unwrap();
        let components = value["components"].as_array().unwrap();
        assert_eq!(components.len(), 3);
        for (index, component) in components.iter().enumerate() {
            let start = 1 + index * 2;
            let byte = u8::from_str_radix(&hex[start..start + 2], 16).unwrap();
            assert!(
                (component.as_f64().unwrap() - f64::from(byte) / 255.0).abs() < 1e-12,
                "{hex}: component {index} differs from numeric sRGB"
            );
        }
        return 1;
    }
    object.values().map(verify_srgb_hex).sum()
}

#[test]
fn authored_srgb_components_match_hex_fallbacks() {
    for source in [LIGHT, DARK] {
        let theme: Value = serde_json::from_str(source).unwrap();
        let pigment = &theme["tokenResolver"]["sets"]["theme"]["sources"][0]["pigment"];
        assert_eq!(verify_srgb_hex(pigment), 20);
    }
}

#[test]
fn authored_themes_compile_with_foundation_and_preserve_hierarchy() {
    let standard = environment_for_scale(1.0);
    let scaled = environment_for_scale(2.0);
    let tier_zero: EnvironmentSnapshot = serde_json::from_str(TIER_ZERO).unwrap();
    for source in [LIGHT, DARK] {
        let result = resolve(source, &standard);
        assert_eq!(
            result.materials()[&MaterialRole::SurfaceChrome],
            MaterialFamily::Frost
        );
        assert_eq!(
            result.materials()[&MaterialRole::ControlPrimary],
            MaterialFamily::Elastomer
        );
        assert_eq!(
            result.materials()[&MaterialRole::FeedbackDrag],
            MaterialFamily::Gel
        );
        assert_eq!(result.typography()[&TypographyRole::Body].font_size(), 16.0);
        assert_eq!(
            result.typography()[&TypographyRole::Display].font_size(),
            48.0
        );
        assert_eq!(
            result.space()[&resina_model::SpatialRole::Page]["value"],
            32
        );
        assert!(result.color_fallbacks()[&ColorRole::SurfaceChrome].alpha() < 1.0);
        assert_eq!(
            result.opaque_color_fallbacks()[&ColorRole::SurfaceChrome].alpha(),
            1.0
        );
        assert_eq!(
            resolve(source, &scaled).typography()[&TypographyRole::Body].font_size(),
            32.0
        );
        let opaque = resolve(source, &tier_zero);
        assert_eq!(
            opaque.frost_representation(),
            FrostRepresentation::OpaqueDimensional
        );
        assert_eq!(opaque.typography()[&TypographyRole::Body].font_size(), 32.0);
    }
}

#[test]
fn authored_opaque_role_pairs_clear_contrast_preflight() {
    let environment = environment_for_scale(1.0);
    for source in [LIGHT, DARK] {
        let result = resolve(source, &environment);
        for background in [
            ColorRole::SurfaceBase,
            ColorRole::SurfaceLow,
            ColorRole::SurfaceHigh,
            ColorRole::SurfaceChrome,
        ] {
            for foreground in [
                ColorRole::ContentPrimary,
                ColorRole::ContentSecondary,
                ColorRole::ContentMuted,
                ColorRole::StatusError,
                ColorRole::StatusWarning,
                ColorRole::StatusSuccess,
                ColorRole::StatusInfo,
            ] {
                minimum_contrast(&result, foreground, background, 4.5);
            }
            minimum_contrast(&result, ColorRole::Outline, background, 3.0);
            minimum_contrast(&result, ColorRole::Focus, background, 3.0);
        }
        minimum_contrast(
            &result,
            ColorRole::ContentPrimary,
            ColorRole::Selection,
            4.5,
        );
        for background in [
            ColorRole::AccentPrimary,
            ColorRole::AccentSecondary,
            ColorRole::AccentTertiary,
        ] {
            minimum_contrast(&result, ColorRole::ContentInverse, background, 4.5);
        }
    }
}

#[test]
fn authored_frost_chrome_is_legible_on_known_base_surface() {
    let environment = environment_for_scale(1.0);
    let intent: SurfaceIntent = serde_json::from_value(json!({
        "schemaVersion": "0.2.0",
        "materialRole": "surface.chrome",
        "colorRole": "surface.chrome",
        "form": {"schemaVersion": "0.1.0", "shape": "structural", "elevation": "base"},
        "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
        "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}
    }))
    .unwrap();
    for source in [LIGHT, DARK] {
        let resolution = resolve(source, &environment);
        let bound = bind_surface(&intent, &resolution).unwrap();
        let colors = resolution.opaque_color_fallbacks();
        let guarded = resolve_frost_legibility(
            bound.frost_representation().unwrap(),
            bound.frost_portable_body().unwrap(),
            bound.opaque_color_fallback(),
            &colors[&ColorRole::ContentPrimary],
            &colors[&ColorRole::SurfaceBase],
            4.5,
        )
        .unwrap();
        assert!(guarded.contrast_ratio() >= 4.5);
        assert_eq!(
            guarded.representation(),
            bound.frost_representation().unwrap()
        );
        assert!(!guarded.fallback_applied());
    }
}

#[test]
fn authored_outline_separates_from_known_base_surface() {
    let environment = environment_for_scale(1.0);
    for source in [LIGHT, DARK] {
        let resolution = resolve(source, &environment);
        let colors = resolution.opaque_color_fallbacks();
        let edge = resolve_edge_contrast(
            &colors[&ColorRole::Outline],
            &colors[&ColorRole::OutlineStrong],
            &colors[&ColorRole::SurfaceBase],
            3.0,
        )
        .unwrap();
        assert_eq!(edge.color_role(), ColorRole::Outline);
        assert!(edge.contrast_ratio() >= 3.0);
        assert!(!edge.fallback_applied());
    }
}
