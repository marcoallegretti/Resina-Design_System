use resina_environment::{EnvironmentSnapshot, SafeArea};
use resina_model::{
    ActivationEvent, ActivationState, CommandPhase, PhysicalBounds, PhysicalVector, SurfaceIntent,
    SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandAccessibilityInput, CommandLabelInput, HitRegionError, HitRegionIr, OpaqueSurfaceIr,
    SurfaceHitRegionInput, compile_theme_source, resolve_activation, resolve_command_accessibility,
    resolve_command_label, resolve_command_motion_source, resolve_command_paint_source,
    resolve_command_states, resolve_surface_hit_region,
};
use serde_json::{Value, json};
use std::convert::Infallible;

fn request() -> Value {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    request["surface"]["body"]["size"] = json!({"width":64,"height":40});
    request
}

fn environment(request: &Value) -> EnvironmentSnapshot {
    serde_json::from_value(request["surface"]["body"]["theme"]["environment"].clone()).unwrap()
}

fn available() -> PhysicalBounds {
    PhysicalBounds {
        x: -100.0,
        y: -100.0,
        width: 400.0,
        height: 400.0,
    }
}

fn input<'a>(env: &'a EnvironmentSnapshot, body: &'a OpaqueSurfaceIr) -> SurfaceHitRegionInput<'a> {
    SurfaceHitRegionInput {
        environment: env,
        body,
        available_bounds: available(),
        component_minimum: SurfaceSize {
            width: 24.0,
            height: 24.0,
        },
        occupied_regions: &[],
    }
}

fn assert_bounds(hit: &HitRegionIr, x: f64, y: f64, width: f64, height: f64) {
    let bounds = hit.bounds();
    for (actual, expected) in [
        (bounds.x, x),
        (bounds.y, y),
        (bounds.width, width),
        (bounds.height, height),
    ] {
        assert!((actual - expected).abs() < 1.0e-12, "{bounds:?}");
    }
}

#[test]
fn static_targets_include_material_depth_and_exclude_independent_focus() {
    for (family, pressed_depth) in [("cast", 1.8), ("frost", 1.7), ("elastomer", 0.5)] {
        for (phase, depth) in [
            ("rest", 2.0),
            ("hover", 2.0),
            ("pressed", pressed_depth),
            ("disabled", 1.4),
        ] {
            let mut request = request();
            let mut theme: Value = serde_json::from_str(
                request["surface"]["body"]["theme"]["themeSource"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            theme["materialAssignments"]["control"]["interactive"] = json!(family);
            request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
            let env = environment(&request);
            let mut targets = Vec::new();
            for focused in [false, true] {
                request["surface"]["body"]["surface"]["states"]["states"] = if focused {
                    json!([phase, "focused"])
                } else {
                    json!([phase])
                };
                let paint = resolve_command_paint_source(&request.to_string()).unwrap();
                assert_eq!(paint.paint().focus().is_some(), focused);
                let hit = resolve_surface_hit_region(input(&env, paint.paint().body())).unwrap();
                assert_bounds(&hit, 0.0, 0.0, 64.0, 40.0 + depth);
                assert!(hit.contains(PhysicalVector { x: 32.0, y: 40.0 }).unwrap());
                assert!(!hit.contains(PhysicalVector { x: -0.5, y: 20.0 }).unwrap());
                targets.push(hit);
            }
            assert_eq!(targets[0], targets[1]);
        }
    }
}

#[test]
fn rounded_paint_does_not_clip_the_rectangular_target() {
    let mut request = request();
    request["surface"]["body"]["surface"]["form"]["shape"] = json!("capsule");
    let env = environment(&request);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let body = paint.paint().body();
    let hit = resolve_surface_hit_region(input(&env, body)).unwrap();
    let corner = PhysicalVector { x: 0.0, y: 0.0 };
    assert_eq!(body.sample_paint(corner).unwrap(), None);
    assert!(hit.contains(corner).unwrap());
}

#[test]
fn negative_sweep_coordinates_do_not_mirror_under_rtl() {
    for direction in ["ltr", "rtl"] {
        let mut request = request();
        request["surface"]["body"]["theme"]["environment"]["layoutDirection"] = json!(direction);
        request["surface"]["body"]["appearance"]["keyLight"]["direction"] = json!({"x":1,"y":0});
        let env = environment(&request);
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        let hit = resolve_surface_hit_region(input(&env, paint.paint().body())).unwrap();
        assert_bounds(&hit, -2.0, 0.0, 66.0, 40.0);
        assert!(hit.contains(PhysicalVector { x: -2.0, y: 0.0 }).unwrap());
        assert!(!hit.contains(PhysicalVector { x: 64.0, y: 0.0 }).unwrap());
    }
}

#[test]
fn coarse_target_expansion_uses_the_swept_center_and_component_minimum() {
    let mut request = request();
    request["surface"]["body"]["size"] = json!({"width":20,"height":12});
    request["surface"]["body"]["theme"]["environment"]["inputCapabilities"] =
        json!(["directTouch"]);
    let env = environment(&request);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let hit = resolve_surface_hit_region(input(&env, paint.paint().body())).unwrap();
    assert_bounds(&hit, -14.0, -17.0, 48.0, 48.0);
    assert_eq!(
        hit.minimum_size(),
        SurfaceSize {
            width: 48.0,
            height: 48.0
        }
    );
    let mut larger = input(&env, paint.paint().body());
    larger.component_minimum.width = 60.0;
    let hit = resolve_surface_hit_region(larger).unwrap();
    assert_bounds(&hit, -20.0, -17.0, 60.0, 48.0);
}

#[test]
fn body_clipping_and_neighbor_conflicts_fail_even_beyond_the_front() {
    let request = request();
    let env = environment(&request);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let body = paint.paint().body();
    let mut clipped = input(&env, body);
    clipped.available_bounds = PhysicalBounds {
        x: 0.0,
        y: 0.0,
        width: 64.0,
        height: 40.0,
    };
    assert!(matches!(
        resolve_surface_hit_region(clipped),
        Err(HitRegionError::Clipped)
    ));
    for (y, overlaps) in [(40.0, true), (42.0, false)] {
        let neighbors = [PhysicalBounds {
            x: 0.0,
            y,
            width: 64.0,
            height: 24.0,
        }];
        let mut occupied = input(&env, body);
        occupied.occupied_regions = &neighbors;
        let result = resolve_surface_hit_region(occupied);
        if overlaps {
            assert!(matches!(result, Err(HitRegionError::Occupied(0))));
        } else {
            result.unwrap();
        }
    }
    let mut invalid = input(&env, body);
    invalid.component_minimum.width = f64::NAN;
    assert!(matches!(
        resolve_surface_hit_region(invalid),
        Err(HitRegionError::InvalidMinimum)
    ));
}

#[test]
fn live_target_membership_routes_through_current_activation_and_accessibility() {
    let mut request = request();
    request["surface"]["body"]["theme"]["environment"]["inputCapabilities"] =
        json!(["coarsePointer"]);
    let env = environment(&request);
    let theme = compile_theme_source(
        request["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let snapshot = theme.resolve(&env).unwrap();
    let label = resolve_command_label(
        CommandLabelInput {
            text: "Reconnect",
            typography: &snapshot.typography()[&TypographyRole::Label],
            minimum_size: SurfaceSize {
                width: 64.0,
                height: 40.0,
            },
            maximum_size: SurfaceSize {
                width: 64.0,
                height: 40.0,
            },
            padding: SafeArea {
                start: 8.0,
                end: 8.0,
                top: 8.0,
                bottom: 8.0,
            },
            direction: env.layout_direction(),
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width: 40.0,
                height: 24.0,
            })
        },
    )
    .unwrap();
    let point = PhysicalVector { x: 0.0, y: -2.0 };
    let initial = ActivationState::try_new(true, true, None).unwrap();
    let mut state = initial;
    for phase in [
        CommandPhase::Rest,
        CommandPhase::Pressed,
        CommandPhase::Disabled,
    ] {
        let surface: SurfaceIntent =
            serde_json::from_value(request["surface"]["body"]["surface"].clone()).unwrap();
        request["surface"]["body"]["surface"] =
            serde_json::to_value(surface.with_states(resolve_command_states(&state, false)))
                .unwrap();
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        assert_eq!(paint.phase(), phase);
        label.validate_content(paint.paint().body()).unwrap();
        let hit = resolve_surface_hit_region(input(&env, paint.paint().body())).unwrap();
        let inside = hit.contains(point).unwrap();
        assert!(inside);
        let accessibility = resolve_command_accessibility(CommandAccessibilityInput {
            label: &label,
            activation: &state,
            description: None,
            focusable: true,
        })
        .unwrap();
        assert_eq!(accessibility.name(), label.text());
        assert!(accessibility.state().focused());
        assert_eq!(accessibility.actions()[0].available(), state.enabled());
        if phase == CommandPhase::Rest {
            state = resolve_activation(
                &state,
                &ActivationEvent::PointerDown {
                    id: "primary".into(),
                    inside,
                },
            )
            .unwrap()
            .state()
            .clone();
        } else if phase == CommandPhase::Pressed {
            state = resolve_activation(&state, &ActivationEvent::Availability { enabled: false })
                .unwrap()
                .state()
                .clone();
        } else {
            assert!(
                !resolve_activation(&state, &ActivationEvent::Invoke {})
                    .unwrap()
                    .activate()
            );
            let reservation = [hit.bounds()];
            let mut occupied = input(&env, paint.paint().body());
            occupied.occupied_regions = &reservation;
            assert!(matches!(
                resolve_surface_hit_region(occupied),
                Err(HitRegionError::Occupied(0))
            ));
        }
    }
}

#[test]
fn sampled_motion_targets_follow_actual_depth_with_immediate_reduced_motion() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    request["surface"]["body"]["size"] = json!({"width":64,"height":40});
    request["surface"]["body"]["surface"]["states"]["states"] = json!(["pressed"]);
    request["surface"]["body"]["theme"]["environment"]["inputCapabilities"] =
        json!(["directTouch"]);
    for reduced in [false, true] {
        request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
            json!(reduced);
        let env = environment(&request);
        for time in [0.0_f64, 0.016, 0.1, 1.0] {
            request["time"] = json!(time);
            let motion = resolve_command_motion_source(&request.to_string()).unwrap();
            let hit =
                resolve_surface_hit_region(input(&env, motion.command().paint().body())).unwrap();
            let depth_scale = if reduced {
                0.25
            } else {
                0.25 + 0.75 * (1.0 + 2.0 * time) * (-2.0 * time).exp()
            };
            assert_bounds(&hit, 0.0, -4.0 + depth_scale, 64.0, 48.0);
            assert!(hit.contains(PhysicalVector { x: 32.0, y: 40.0 }).unwrap());
        }
    }
}
