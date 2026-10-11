use guido::{prelude::FontFamily, renderer::DrawCommand};
use resina_environment::SafeArea;
use resina_guido::{CommandInputs, CommandPrepareError};
use resina_model::{
    ActivationEvent, ActivationState, CommandEmphasis, PhysicalBounds, PhysicalVector, SpringState,
    SurfaceSize,
};
use resina_resolver::{compile_theme_source, resolve_activation, resolve_srgb_fallback};
use serde_json::Value;
use std::sync::Arc;

fn inputs() -> CommandInputs {
    guido::load_font(
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("explicit font required")).unwrap(),
    );
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let body = &source["surface"]["body"];
    CommandInputs {
        theme: Arc::new(
            compile_theme_source(body["theme"]["themeSource"].as_str().unwrap()).unwrap(),
        ),
        environment: serde_json::from_value(body["theme"]["environment"].clone()).unwrap(),
        anatomy: serde_json::from_str(include_str!(
            "../../../../definitions/components/command.json"
        ))
        .unwrap(),
        emphasis: CommandEmphasis::Standard,
        appearance: serde_json::from_value(body["appearance"].clone()).unwrap(),
        command_appearance: serde_json::from_value(source["commandAppearance"].clone()).unwrap(),
        font: FontFamily::name("DejaVu Sans"),
        text: "Reconnect".into(),
        description: Some("Reconnect to the configured network.".into()),
        minimum_size: SurfaceSize {
            width: 160.0,
            height: 64.0,
        },
        maximum_size: SurfaceSize {
            width: 420.0,
            height: 200.0,
        },
        padding: SafeArea {
            start: 26.0,
            end: 26.0,
            top: 26.0,
            bottom: 26.0,
        },
        available_bounds: PhysicalBounds {
            x: -40.0,
            y: -40.0,
            width: 600.0,
            height: 320.0,
        },
        component_minimum: SurfaceSize {
            width: 48.0,
            height: 48.0,
        },
        occupied_regions: vec![],
        post_treatment_backdrop: Some(
            resolve_srgb_fallback(&body["postTreatmentBackdrop"]).unwrap(),
        ),
        adjacent_color: resolve_srgb_fallback(&body["adjacentColor"]).unwrap(),
        surrounding_color: Some(
            resolve_srgb_fallback(&source["surface"]["surroundingColor"]).unwrap(),
        ),
        minimum_content_contrast: 4.5,
        minimum_edge_contrast: 3.0,
        enabled: true,
        focusable: true,
        device_scale: 1.0,
        samples_per_axis: 1,
    }
}

#[test]
fn complete_authored_updates_rebuild_all_channels_without_mutating_the_old_publication() {
    fn send<T: Send>() {}
    send::<CommandInputs>();
    let mut requested = inputs();
    let state = ActivationState::try_new(true, true, None).unwrap();
    let origin = PhysicalVector {
        x: 7.125,
        y: -0.375,
    };
    let initial = requested.prepare(&state, false, origin).unwrap();
    requested.text = "Reconnect to Wi-Fi".into();
    requested.emphasis = CommandEmphasis::Primary;
    requested.device_scale = 1.25;
    let changed = requested.prepare(&state, false, origin).unwrap();
    assert_eq!(initial.label().text(), "Reconnect");
    assert_eq!(initial.accessibility().name(), "Reconnect");
    assert_eq!(changed.label().text(), "Reconnect to Wi-Fi");
    assert_eq!(changed.accessibility().name(), changed.label().text());
    assert_eq!(changed.activation(), &state);
    assert!(changed.paint().paint().focus().is_some());
    assert!(
        changed
            .hit_region()
            .contains_bounds(
                changed
                    .paint()
                    .paint()
                    .body()
                    .geometry()
                    .silhouette()
                    .bounds()
                    .unwrap()
            )
            .unwrap()
    );
    let DrawCommand::Text { text, rect, .. } = &changed.commands()[1] else {
        panic!("complete label missing")
    };
    assert_eq!(text.as_str(), changed.label().text());
    assert!(
        (f64::from(rect.x) - (changed.label().label_bounds().x + origin.x)).abs() <= 1.0 / 1024.0
    );
    assert_eq!(changed.placement(), origin);
    assert_ne!(
        changed.paint().paint().body().color_role(),
        initial.paint().paint().body().color_role()
    );
    requested.text.clear();
    assert!(matches!(
        requested.prepare(&state, false, origin),
        Err(CommandPrepareError::Label(_))
    ));
    assert_eq!(changed.accessibility().name(), "Reconnect to Wi-Fi");
    assert_eq!(initial.accessibility().name(), "Reconnect");
}

#[test]
fn reservation_is_stable_through_activation_focus_hover_and_availability() {
    let mut requested = inputs();
    let initial = ActivationState::try_new(true, false, None).unwrap();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    let rest = requested.prepare(&initial, false, origin).unwrap();
    let held = resolve_activation(
        &initial,
        &ActivationEvent::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    let focused =
        resolve_activation(held.state(), &ActivationEvent::Focus { focused: true }).unwrap();
    for hovered in [false, true] {
        let frame = requested.prepare(focused.state(), hovered, origin).unwrap();
        assert_eq!(frame.hit_region(), rest.hit_region());
        assert_eq!(frame.activation(), focused.state());
        assert!(frame.accessibility().state().focused());
        assert!(frame.paint().paint().focus().is_some());
    }
    let disabled = resolve_activation(
        focused.state(),
        &ActivationEvent::Availability { enabled: false },
    )
    .unwrap();
    assert!(matches!(
        requested.prepare(disabled.state(), false, origin),
        Err(CommandPrepareError::Availability { .. })
    ));
    requested.enabled = false;
    let unavailable = requested.prepare(disabled.state(), true, origin).unwrap();
    assert_eq!(unavailable.hit_region(), rest.hit_region());
    assert!(!unavailable.accessibility().actions()[0].available());
    assert!(unavailable.accessibility().state().focused());
}

#[test]
fn rejected_geometry_and_native_preparation_return_no_replacement() {
    let mut requested = inputs();
    let state = ActivationState::try_new(true, false, None).unwrap();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    let valid = requested.prepare(&state, false, origin).unwrap();
    requested.occupied_regions.push(valid.hit_region().bounds());
    assert!(matches!(
        requested.prepare(&state, false, origin),
        Err(CommandPrepareError::HitRegion(_))
    ));
    requested.occupied_regions.clear();
    requested.device_scale = f32::NAN;
    assert!(matches!(
        requested.prepare(&state, false, origin),
        Err(CommandPrepareError::Content(_))
    ));
    assert_eq!(valid.label().text(), "Reconnect");
    assert_eq!(valid.activation(), &state);
}

#[test]
fn changed_environment_remeasures_complete_text_and_maps_logical_padding() {
    let mut requested = inputs();
    let mut environment = serde_json::to_value(&requested.environment).unwrap();
    environment["textScale"] = serde_json::json!(1.0);
    requested.environment = serde_json::from_value(environment).unwrap();
    requested.padding.start = 34.0;
    requested.padding.end = 26.0;
    let state = ActivationState::try_new(true, false, None).unwrap();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    let before = requested.prepare(&state, false, origin).unwrap();
    let mut environment = serde_json::to_value(&requested.environment).unwrap();
    environment["textScale"] = serde_json::json!(1.5);
    environment["layoutDirection"] = serde_json::json!("rtl");
    requested.environment = serde_json::from_value(environment).unwrap();
    requested.text = "إعادة الاتصال".into();
    let after = requested.prepare(&state, false, origin).unwrap();
    assert_eq!(
        after.label().typography().font_size(),
        before.label().typography().font_size() * 1.5
    );
    assert_eq!(
        after.label().layout_direction(),
        resina_environment::LayoutDirection::Rtl
    );
    assert_eq!(after.label().label_bounds().x, requested.padding.end);
    assert_eq!(after.accessibility().name(), "إعادة الاتصال");
    assert_eq!(before.label().text(), "Reconnect");
    let DrawCommand::Text {
        text, font_size, ..
    } = &after.commands()[1]
    else {
        panic!("label missing")
    };
    assert_eq!(text, after.label().text());
    assert_eq!(
        f64::from(*font_size),
        after.label().typography().font_size()
    );
}

#[test]
fn sampled_publication_retains_unprojected_state_for_continuous_retargeting() {
    let mut requested = inputs();
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let mut channels: resina_resolver::CommandMotionChannels =
        serde_json::from_value(source["channels"].clone()).unwrap();
    channels.body_mix.initial = SpringState::try_new(-0.1, 0.2).unwrap();
    channels.depth_scale.initial = SpringState::try_new(1.2, -0.4).unwrap();
    let state = ActivationState::try_new(true, false, None).unwrap();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    let first = requested
        .prepare_motion(&state, false, origin, &channels, 0.0)
        .unwrap();
    let sample = first.motion().unwrap();
    assert_eq!(sample.body_mix().state(), channels.body_mix.initial);
    assert_eq!(sample.depth_scale().state(), channels.depth_scale.initial);
    assert_eq!(
        sample.depth_scale_projection(),
        resina_resolver::CommandProjection::UpperBound
    );
    assert_eq!(first.paint().response().depth_scale(), 1.0);
    let moving = requested
        .prepare_motion(&state, false, origin, &channels, 0.03)
        .unwrap();
    channels.body_mix.initial = moving.motion().unwrap().body_mix().state();
    channels.depth_scale.initial = moving.motion().unwrap().depth_scale().state();
    let pressed = resolve_activation(
        &state,
        &ActivationEvent::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    let retargeted = requested
        .prepare_motion(pressed.state(), true, origin, &channels, 0.0)
        .unwrap();
    assert_eq!(
        retargeted.motion().unwrap().body_mix().state(),
        moving.motion().unwrap().body_mix().state()
    );
    assert_eq!(
        retargeted.motion().unwrap().depth_scale().state(),
        moving.motion().unwrap().depth_scale().state()
    );
    assert_eq!(retargeted.hit_region(), first.hit_region());
    assert_eq!(retargeted.activation(), pressed.state());
    let mut environment = serde_json::to_value(&requested.environment).unwrap();
    environment["accessibilityPreferences"]["reducedMotion"] = serde_json::json!(true);
    requested.environment = serde_json::from_value(environment).unwrap();
    let immediate = requested
        .prepare_motion(pressed.state(), true, origin, &channels, 0.0)
        .unwrap();
    let sample = immediate.motion().unwrap();
    assert_eq!(
        sample.policy(),
        resina_resolver::CommandMotionPolicy::ReducedMotion
    );
    assert!(sample.body_mix().settled() && sample.depth_scale().settled());
    assert_eq!(sample.body_mix().state().velocity(), 0.0);
    assert_eq!(sample.depth_scale().state().velocity(), 0.0);
    assert_eq!(sample.command().response(), sample.target());
}
