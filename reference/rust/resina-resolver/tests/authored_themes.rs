use resina_color::{composite_srgb_over_opaque, resolve_srgb_fallback};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, ContourSegment, FrostRepresentation, InteractionState, KeyLight, MaterialFamily,
    MaterialRole, OpaqueSurfaceAppearance, PhysicalVector, SurfaceIntent, SurfaceSize,
    TypographyRole,
};
use resina_resolver::{
    HeadlessResolution, OpaquePigmentError, OpaqueSurfaceInput, bind_surface,
    compile_theme_source_with_sources, opaque_contrast_ratio, resolve_edge_contrast,
    resolve_focus_indicator, resolve_frost_legibility, resolve_frost_surface_readability,
    resolve_key_light, resolve_opaque_pigment, resolve_opaque_surface, resolve_surface_readability,
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

fn contour_support(contour: &resina_resolver::ExtrudedContourResult, n: PhysicalVector) -> f64 {
    let dot = |p: PhysicalVector| p.x * n.x + p.y * n.y;
    contour
        .segments()
        .iter()
        .map(|segment| match segment {
            ContourSegment::Line { from, to } => dot(*from).max(dot(*to)),
            ContourSegment::Arc {
                center,
                radii,
                start,
                end,
            } => {
                let radial = PhysicalVector {
                    x: radii.x * n.x,
                    y: radii.y * n.y,
                };
                let length = radial.x.hypot(radial.y);
                let radial = PhysicalVector {
                    x: radial.x / length,
                    y: radial.y / length,
                };
                let evaluate = |u: PhysicalVector| {
                    dot(PhysicalVector {
                        x: center.x + radii.x * u.x,
                        y: center.y + radii.y * u.y,
                    })
                };
                let endpoints = evaluate(*start).max(evaluate(*end));
                if start.x * radial.y - start.y * radial.x >= 0.0
                    && radial.x * end.y - radial.y * end.x >= 0.0
                {
                    endpoints.max(evaluate(radial))
                } else {
                    endpoints
                }
            }
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

fn assert_region_containment(geometry: &resina_resolver::OpaqueSurfaceGeometry) {
    for degrees in 0..360 {
        let angle = f64::from(degrees).to_radians();
        let n = PhysicalVector {
            x: angle.cos(),
            y: angle.sin(),
        };
        let placed = |p: &resina_resolver::PlacedContour| {
            contour_support(p.contour(), n) + p.offset().x * n.x + p.offset().y * n.y
        };
        let edge = placed(geometry.edge_interior());
        let highlight = placed(geometry.highlight_outer());
        let content = placed(geometry.content());
        assert!(edge <= contour_support(geometry.silhouette(), n) + 1e-10);
        assert!(highlight <= edge + 1e-10);
        assert!(content <= highlight + 1e-10);
        assert!(content <= contour_support(geometry.front(), n) + 1e-10);
    }
}

#[test]
fn authored_opaque_ir_preserves_family_shape_elevation_and_readability() {
    let appearance: OpaqueSurfaceAppearance = serde_json::from_str(include_str!(
        "../../../../definitions/tier0-surface-appearance.json"
    ))
    .unwrap();
    let sources = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    let mut template: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    let template = template.as_object_mut().unwrap();
    for source in [LIGHT, DARK] {
        let theme = compile_theme_source_with_sources(source, &sources).unwrap();
        for direction in ["ltr", "rtl"] {
            let mut environment: Value = serde_json::from_str(TIER_ZERO).unwrap();
            environment["layoutDirection"] = direction.into();
            environment["textScale"] = 3.into();
            let environment: EnvironmentSnapshot = serde_json::from_value(environment).unwrap();
            let resolution = theme.resolve(&environment).unwrap();
            let adjacent = &resolution.opaque_color_fallbacks()[&ColorRole::SurfaceBase];
            for (role, color_role, foreground_role, family) in [
                (
                    "surface.base",
                    "surface.high",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Cast,
                ),
                (
                    "surface.chrome",
                    "surface.chrome",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Frost,
                ),
                (
                    "control.primary",
                    "accent.primary",
                    ColorRole::ContentInverse,
                    MaterialFamily::Elastomer,
                ),
                (
                    "feedback.selection",
                    "selection",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Gel,
                ),
            ] {
                for (index, shape) in ["structural", "soft", "rounded", "capsule", "organic"]
                    .into_iter()
                    .enumerate()
                {
                    let mut surface = template["surface"].clone();
                    surface["materialRole"] = role.into();
                    surface["colorRole"] = color_role.into();
                    surface["form"]["shape"] = shape.into();
                    surface["form"]["elevation"] =
                        ["embedded", "base", "raised", "floating", "overlay", "modal"]
                            [(index + usize::from(direction == "rtl")) % 6]
                            .into();
                    let surface: SurfaceIntent = serde_json::from_value(surface).unwrap();
                    let ir = resolve_opaque_surface(
                        &theme,
                        &environment,
                        OpaqueSurfaceInput {
                            surface: &surface,
                            size: SurfaceSize {
                                width: 240.0,
                                height: 80.0,
                            },
                            appearance: &appearance,
                            foreground_role,
                            post_treatment_backdrop: (family == MaterialFamily::Frost)
                                .then_some(adjacent),
                            adjacent_color: adjacent,
                            minimum_content_contrast: 4.5,
                            minimum_edge_contrast: 3.0,
                        },
                    )
                    .unwrap();
                    assert_eq!(ir.material_family(), family);
                    assert_eq!(ir.form(), surface.form());
                    assert_eq!(ir.states(), surface.states());
                    assert_region_containment(ir.geometry());
                    assert_eq!(ir.pigment().body().alpha(), 1.0);
                    assert!(ir.content_contrast_ratio() >= 4.5);
                    assert!(ir.edge().contrast_ratio() >= 3.0);
                    assert!(ir.geometry().content().contour().bounds().unwrap().width > 0.0);
                    assert_eq!(
                        ir.geometry().content().offset().x,
                        ir.edge_width() + ir.highlight_width()
                    );
                    assert_eq!(
                        ir.frost_representation(),
                        (family == MaterialFamily::Frost)
                            .then_some(FrostRepresentation::OpaqueDimensional)
                    );
                    assert!(ir.lighting().direction().x < 0.0 && ir.lighting().direction().y < 0.0);
                    assert!(
                        ir.lighting().side_offset().x >= 0.0
                            && ir.lighting().side_offset().y >= 0.0
                    );
                }
            }
        }
    }
}

fn frost_chrome_intent() -> SurfaceIntent {
    serde_json::from_value(json!({
        "schemaVersion": "0.2.0",
        "materialRole": "surface.chrome",
        "colorRole": "surface.chrome",
        "form": {"schemaVersion": "0.1.0", "shape": "structural", "elevation": "base"},
        "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
        "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}
    }))
    .unwrap()
}

fn focused_control_intent() -> SurfaceIntent {
    serde_json::from_value(json!({
        "schemaVersion": "0.2.0",
        "materialRole": "control.primary",
        "colorRole": "accent.primary",
        "form": {"schemaVersion": "0.1.0", "shape": "soft", "elevation": "raised"},
        "states": {"schemaVersion": "0.1.0", "states": ["selected", "focused"]},
        "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}
    }))
    .unwrap()
}

fn base_state_intent(material_role: &str, color_role: &str) -> SurfaceIntent {
    serde_json::from_value(json!({
        "schemaVersion": "0.2.0",
        "materialRole": material_role,
        "colorRole": color_role,
        "form": {"schemaVersion": "0.1.0", "shape": "soft", "elevation": "base"},
        "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
        "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}
    }))
    .unwrap()
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
    let intent = frost_chrome_intent();
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
fn authored_frost_chrome_keeps_content_and_edge_readable_at_tier_zero() {
    let standard = environment_for_scale(1.0);
    let tier_zero: EnvironmentSnapshot = serde_json::from_str(TIER_ZERO).unwrap();
    let intent = frost_chrome_intent();
    for source in [LIGHT, DARK] {
        for (environment, representation) in [
            (&standard, FrostRepresentation::TranslucentPigmented),
            (&tier_zero, FrostRepresentation::OpaqueDimensional),
        ] {
            let resolution = resolve(source, environment);
            let base = &resolution.opaque_color_fallbacks()[&ColorRole::SurfaceBase];
            let guarded = resolve_frost_surface_readability(
                &intent,
                &resolution,
                ColorRole::ContentPrimary,
                base,
                base,
                4.5,
                3.0,
            )
            .unwrap();
            assert_eq!(guarded.binding().material_family(), MaterialFamily::Frost);
            assert_eq!(
                guarded.foreground(),
                &resolution.color_fallbacks()[&ColorRole::ContentPrimary]
            );
            assert!(guarded.legibility().contrast_ratio() >= 4.5);
            assert!(guarded.edge().contrast_ratio() >= 3.0);
            assert!(!guarded.legibility().fallback_applied());
            assert!(!guarded.edge().fallback_applied());
            assert_eq!(guarded.legibility().representation(), representation);
        }
    }
}

#[test]
fn authored_materials_keep_content_and_edge_readable_in_both_themes() {
    let standard = environment_for_scale(1.0);
    let tier_zero: EnvironmentSnapshot = serde_json::from_str(TIER_ZERO).unwrap();
    let pigment_profiles =
        serde_json::from_str(include_str!("../../../../definitions/tier0-pigment.json")).unwrap();
    let key_light: KeyLight =
        serde_json::from_str(include_str!("../../../../definitions/key-light.json")).unwrap();
    let lighting = resolve_key_light(
        &key_light,
        1.0,
        &[
            PhysicalVector { x: 0.0, y: -1.0 },
            PhysicalVector { x: 1.0, y: 0.0 },
            PhysicalVector { x: 0.0, y: 1.0 },
            PhysicalVector { x: -1.0, y: 0.0 },
        ],
    )
    .unwrap();
    assert!(lighting.side_offset().x > 0.0 && lighting.side_offset().y > 0.0);
    for source in [LIGHT, DARK] {
        for environment in [&standard, &tier_zero] {
            let resolution = resolve(source, environment);
            let base = &resolution.opaque_color_fallbacks()[&ColorRole::SurfaceBase];
            for (material_role, color_role, foreground_role, family) in [
                (
                    "surface.base",
                    "surface.base",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Cast,
                ),
                (
                    "surface.chrome",
                    "surface.chrome",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Frost,
                ),
                (
                    "control.primary",
                    "accent.primary",
                    ColorRole::ContentInverse,
                    MaterialFamily::Elastomer,
                ),
                (
                    "feedback.selection",
                    "selection",
                    ColorRole::ContentPrimary,
                    MaterialFamily::Gel,
                ),
            ] {
                let intent = base_state_intent(material_role, color_role);
                let result = resolve_surface_readability(
                    &intent,
                    &resolution,
                    foreground_role,
                    (family == MaterialFamily::Frost).then_some(base),
                    base,
                    4.5,
                    3.0,
                )
                .unwrap();
                assert_eq!(result.binding().material_family(), family);
                assert!(result.content_contrast_ratio() >= 4.5);
                assert!(result.edge().contrast_ratio() >= 3.0);
                let pigment = resolve_opaque_pigment(family, result.body(), &pigment_profiles);
                if result.body().alpha() == 1.0 {
                    let pigment = pigment.unwrap();
                    assert_eq!(pigment.body(), result.body());
                    assert_eq!(pigment.side().alpha(), 1.0);
                    assert_eq!(pigment.highlight().alpha(), 1.0);
                    for weight in lighting.normal_highlight_weights() {
                        let overlay = resolve_srgb_fallback(&json!({
                            "colorSpace": "srgb", "components": [1, 1, 1],
                            "alpha": pigment.profile().highlight_lift() * weight,
                        }))
                        .unwrap();
                        let lit_edge =
                            composite_srgb_over_opaque(&overlay, pigment.body()).unwrap();
                        assert_eq!(lit_edge.alpha(), 1.0);
                        if *weight == 0.0 {
                            assert_eq!(&lit_edge, pigment.body());
                        }
                        for ((lit, body), full) in lit_edge
                            .components()
                            .into_iter()
                            .zip(pigment.body().components())
                            .zip(pigment.highlight().components())
                        {
                            assert!(lit >= body && lit <= full);
                        }
                    }
                    for ((side, body), highlight) in pigment
                        .side()
                        .components()
                        .into_iter()
                        .zip(pigment.body().components())
                        .zip(pigment.highlight().components())
                    {
                        assert!(side <= body);
                        assert!(highlight >= body);
                    }
                } else {
                    assert!(matches!(pigment, Err(OpaquePigmentError::TranslucentBody)));
                }
                if family == MaterialFamily::Frost {
                    assert!(result.frost_representation().is_some());
                } else {
                    assert_eq!(result.body().alpha(), 1.0);
                    assert_eq!(result.frost_representation(), None);
                }
            }
        }
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

#[test]
fn authored_focus_indicator_survives_tier_zero_and_concurrent_selection() {
    let standard = environment_for_scale(1.0);
    let tier_zero: EnvironmentSnapshot = serde_json::from_str(TIER_ZERO).unwrap();
    let intent = focused_control_intent();
    for source in [LIGHT, DARK] {
        for environment in [&standard, &tier_zero] {
            let resolution = resolve(source, environment);
            for background in [
                ColorRole::SurfaceBase,
                ColorRole::SurfaceLow,
                ColorRole::SurfaceHigh,
            ] {
                let surrounding = &resolution.opaque_color_fallbacks()[&background];
                let indicator = resolve_focus_indicator(&intent, &resolution, surrounding).unwrap();
                assert_eq!(indicator.color_role(), ColorRole::Focus);
                assert!(!indicator.fallback_applied());
                assert!(indicator.contrast_ratio() >= 3.0);
                assert_eq!(indicator.stroke_width(), 2.0);
                assert_eq!(indicator.gap(), 2.0);
                assert!(
                    indicator
                        .binding()
                        .states()
                        .contains(InteractionState::Focused)
                );
                assert!(
                    indicator
                        .binding()
                        .states()
                        .contains(InteractionState::Selected)
                );
            }
        }
    }
}
