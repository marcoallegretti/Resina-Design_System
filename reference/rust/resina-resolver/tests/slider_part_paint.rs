use resina_color::{OpaqueSrgbRange, resolve_srgb_fallback};
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, OpaqueSurfaceAppearance, SliderAppearance, SliderPart, SurfaceIntent, SurfaceSize,
};
use resina_resolver::{
    CompiledTheme, SliderPartPaintInput, compile_theme_source, resolve_slider_part_paint,
};
use serde_json::{Value, json};

struct Inputs {
    theme: CompiledTheme,
    environment: EnvironmentSnapshot,
    surface: SurfaceIntent,
    appearance: OpaqueSurfaceAppearance,
    interaction: SliderAppearance,
    backdrop: resina_color::SrgbFallback,
}
fn inputs(family: &str, states: &Value) -> Inputs {
    let request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    let body = &request["surface"]["body"];
    let mut theme: Value =
        serde_json::from_str(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["materialAssignments"]["control"]["interactive"] = json!(family);
    theme["opaqueColorAssignments"]["roles"]["focus"] = json!("palette.black");
    let mut surface = body["surface"].clone();
    surface["states"] = states.clone();
    Inputs {
        theme: compile_theme_source(&theme.to_string()).unwrap(),
        environment: serde_json::from_value(body["theme"]["environment"].clone()).unwrap(),
        surface: serde_json::from_value(surface).unwrap(),
        appearance: serde_json::from_value(body["appearance"].clone()).unwrap(),
        interaction: serde_json::from_str(include_str!(
            "../../../../conformance/appearance/slider-appearance.json"
        ))
        .unwrap(),
        backdrop: resolve_srgb_fallback(&body["postTreatmentBackdrop"]).unwrap(),
    }
}
fn range(channels: [f64; 3]) -> OpaqueSrgbRange {
    let color = resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":channels})).unwrap();
    OpaqueSrgbRange::try_new(color.clone(), color).unwrap()
}
fn paint_input<'a>(
    inputs: &'a Inputs,
    part: SliderPart,
    read_only: bool,
    adjacent: &'a [OpaqueSrgbRange],
    surrounding: Option<&'a [OpaqueSrgbRange]>,
) -> SliderPartPaintInput<'a> {
    SliderPartPaintInput {
        part,
        read_only,
        surface: &inputs.surface,
        size: SurfaceSize {
            width: 20.0,
            height: 12.0,
        },
        appearance: &inputs.appearance,
        interaction_appearance: &inputs.interaction,
        foreground_role: ColorRole::ContentPrimary,
        post_treatment_backdrop: Some(&inputs.backdrop),
        adjacent_ranges: adjacent,
        surrounding_ranges: surrounding,
        minimum_content_contrast: 1.0,
        minimum_edge_contrast: 3.0,
    }
}

#[test]
fn public_phase_cases_resolve_both_parts_without_losing_interaction_or_focus() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-phase-cases.json"
    ))
    .unwrap();
    let appearance: Value = serde_json::from_str(include_str!(
        "../../../../conformance/appearance/slider-appearance.json"
    ))
    .unwrap();
    let backgrounds = [range([1.0; 3])];
    for family in ["cast", "frost", "elastomer"] {
        for part in [SliderPart::Track, SliderPart::Thumb] {
            for case in &cases {
                let inputs = inputs(family, &case["states"]);
                let result = resolve_slider_part_paint(
                    &inputs.theme,
                    &inputs.environment,
                    paint_input(
                        &inputs,
                        part,
                        case["readOnly"].as_bool().unwrap(),
                        &backgrounds,
                        Some(&backgrounds),
                    ),
                );
                if let Some(phase) = case.get("expected") {
                    let result = result.unwrap();
                    assert_eq!(
                        serde_json::to_value(result.phase()).unwrap(),
                        *phase,
                        "{}",
                        case["name"]
                    );
                    assert_eq!(result.read_only(), case["readOnly"].as_bool().unwrap());
                    assert_eq!(result.part(), part);
                    assert_eq!(result.paint().body().states(), inputs.surface.states());
                    assert_eq!(
                        serde_json::to_value(result.paint().body().material_family()).unwrap(),
                        family
                    );
                    let part_name = match part {
                        SliderPart::Track => "track",
                        SliderPart::Thumb => "thumb",
                    };
                    let expected_response = if phase == "rest" {
                        json!({"bodyMix":0,"depthScale":1})
                    } else {
                        appearance[part_name][family][phase.as_str().unwrap()].clone()
                    };
                    assert_eq!(
                        result.response().body_mix(),
                        expected_response["bodyMix"].as_f64().unwrap()
                    );
                    assert_eq!(
                        result.response().depth_scale(),
                        expected_response["depthScale"].as_f64().unwrap()
                    );
                    let focused = inputs
                        .surface
                        .states()
                        .states()
                        .iter()
                        .any(|state| serde_json::to_value(state).unwrap() == "focused");
                    assert_eq!(
                        result.paint().focus().is_some(),
                        part == SliderPart::Thumb && focused
                    );
                    if let Some(focus) = result.paint().focus() {
                        assert_eq!(
                            focus.indicator().binding().states(),
                            inputs.surface.states()
                        );
                        assert_eq!(
                            focus.geometry().silhouette(),
                            result.paint().body().geometry().silhouette()
                        );
                    }
                    let mix = result.response().body_mix();
                    let base = [0.8, 0.7, 0.6];
                    for (actual, base) in result
                        .paint()
                        .body()
                        .pigment()
                        .body()
                        .components()
                        .into_iter()
                        .zip(base)
                    {
                        let expected = if mix < 0.0 {
                            base * (1.0 + mix)
                        } else {
                            base + (1.0 - base) * mix
                        };
                        assert!((actual - expected).abs() < 1e-12);
                    }
                    assert!(
                        (result.paint().body().lighting().side_offset().y
                            - 2.0 * result.response().depth_scale())
                        .abs()
                            < 1e-12
                    );
                } else {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains(case["error"].as_str().unwrap())
                    );
                }
            }
        }
    }
}

#[test]
fn mixed_backgrounds_fail_atomically_and_only_thumb_owns_navigation() {
    let inputs = inputs(
        "cast",
        &json!({"schemaVersion":"0.1.0","states":["focused","pressed","dragging"]}),
    );
    let white = [range([1.0; 3])];
    let incompatible = [range([0.0; 3]), range([1.0; 3])];
    for part in [SliderPart::Track, SliderPart::Thumb] {
        let error = resolve_slider_part_paint(
            &inputs.theme,
            &inputs.environment,
            paint_input(&inputs, part, false, &incompatible, Some(&white)),
        )
        .unwrap_err();
        assert!(error.to_string().contains("edge contrast is insufficient"));
    }
    assert!(
        resolve_slider_part_paint(
            &inputs.theme,
            &inputs.environment,
            paint_input(&inputs, SliderPart::Track, false, &white, None)
        )
        .unwrap()
        .paint()
        .focus()
        .is_none()
    );
    let error = resolve_slider_part_paint(
        &inputs.theme,
        &inputs.environment,
        paint_input(&inputs, SliderPart::Thumb, false, &white, None),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("focused slider thumb requires surrounding ranges")
    );
    let error = resolve_slider_part_paint(
        &inputs.theme,
        &inputs.environment,
        paint_input(
            &inputs,
            SliderPart::Thumb,
            false,
            &white,
            Some(&incompatible),
        ),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("focus indicator contrast is insufficient")
    );
    for part in [SliderPart::Track, SliderPart::Thumb] {
        let error = resolve_slider_part_paint(
            &inputs.theme,
            &inputs.environment,
            paint_input(&inputs, part, false, &white, Some(&[])),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("surrounding ranges must not be empty")
        );
    }
    let error = resolve_slider_part_paint(
        &inputs.theme,
        &inputs.environment,
        paint_input(&inputs, SliderPart::Track, false, &[], None),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("adjacent ranges must not be empty")
    );
}

#[test]
fn noncontrol_roles_and_unresolved_translucency_are_explicit_failures() {
    let states = json!({"schemaVersion":"0.1.0","states":["rest"]});
    let backgrounds = [range([1.0; 3])];
    let mut opaque = inputs("cast", &states);
    let mut surface = serde_json::to_value(&opaque.surface).unwrap();
    surface["materialRole"] = json!("feedback.selection");
    opaque.surface = serde_json::from_value(surface).unwrap();
    assert!(
        resolve_slider_part_paint(
            &opaque.theme,
            &opaque.environment,
            paint_input(&opaque, SliderPart::Track, false, &backgrounds, None)
        )
        .unwrap_err()
        .to_string()
        .contains("persistent control material role")
    );
    let mut frost = inputs("frost", &states);
    let mut environment = serde_json::to_value(&frost.environment).unwrap();
    environment["accessibilityPreferences"]["reducedTransparency"] = json!(false);
    environment["accessibilityPreferences"]["highContrast"] = json!(false);
    environment["rendererCapabilities"]["translucentSurfaces"] = json!(true);
    frost.environment = serde_json::from_value(environment).unwrap();
    assert!(
        resolve_slider_part_paint(
            &frost.theme,
            &frost.environment,
            paint_input(&frost, SliderPart::Track, false, &backgrounds, None)
        )
        .unwrap_err()
        .to_string()
        .contains("resolved opaque body")
    );
    frost.backdrop =
        resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[0,0,0]})).unwrap();
    let mut input = paint_input(&frost, SliderPart::Track, false, &backgrounds, None);
    input.minimum_content_contrast = 7.0;
    let result = resolve_slider_part_paint(&frost.theme, &frost.environment, input).unwrap();
    assert!(result.paint().body().content_fallback_applied());
    assert_eq!(
        serde_json::to_value(result.paint().body().frost_representation()).unwrap(),
        "opaqueDimensional"
    );
}
