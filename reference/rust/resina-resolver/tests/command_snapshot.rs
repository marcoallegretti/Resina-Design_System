use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{
    ActivationEvent, ActivationState, InteractionState, PhysicalBounds, SurfaceIntent, SurfaceSize,
    TypographyRole,
};
use resina_resolver::{
    CommandAccessibilityError, CommandContentError, CommandLabelInput, CommandLabelIr,
    CommandPaintIr, CommandSnapshotError, CommandSnapshotInput, HitRegionError,
    compile_theme_source, resolve_activation, resolve_command_label, resolve_command_motion_source,
    resolve_command_paint_source, resolve_command_snapshot, resolve_command_states,
};
use serde::Deserialize;
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

fn label(request: &Value, width: f64, padding: f64, direction: LayoutDirection) -> CommandLabelIr {
    let body = &request["surface"]["body"];
    let theme = compile_theme_source(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    let resolved = theme.resolve(&environment(request)).unwrap();
    resolve_command_label(
        CommandLabelInput {
            text: "Reconnect",
            typography: &resolved.typography()[&TypographyRole::Label],
            minimum_size: SurfaceSize {
                width,
                height: 40.0,
            },
            maximum_size: SurfaceSize {
                width,
                height: 40.0,
            },
            padding: SafeArea {
                start: padding,
                end: padding,
                top: 8.0,
                bottom: 8.0,
            },
            direction,
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width: 40.0,
                height: 24.0,
            })
        },
    )
    .unwrap()
}

fn input<'a, 'context>(
    env: &'context EnvironmentSnapshot,
    label: &'a CommandLabelIr,
    paint: &'a CommandPaintIr,
    activation: &'context ActivationState,
    hovered: bool,
) -> CommandSnapshotInput<'a, 'context> {
    CommandSnapshotInput {
        label,
        paint,
        activation,
        hovered,
        description: Some("Reconnect to the saved network"),
        focusable: true,
        environment: env,
        available_bounds: PhysicalBounds {
            x: -100.0,
            y: -100.0,
            width: 400.0,
            height: 400.0,
        },
        component_minimum: SurfaceSize {
            width: 48.0,
            height: 48.0,
        },
        occupied_regions: &[],
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Matrix {
    schema_version: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    activation: ActivationState,
    hovered: bool,
    paint_states: Vec<InteractionState>,
    coherent: bool,
}

#[test]
fn public_cases_reject_stale_signals_even_when_body_phase_matches() {
    let matrix: Matrix = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/command-snapshot-cases.json"
    ))
    .unwrap();
    assert_eq!(matrix.schema_version, "0.1.0");
    assert_eq!(matrix.cases.len(), 16);
    let mut request = request();
    let env = environment(&request);
    let label = label(&request, 64.0, 8.0, env.layout_direction());
    for case in matrix.cases {
        request["surface"]["body"]["surface"]["states"]["states"] =
            serde_json::to_value(case.paint_states).unwrap();
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        let result =
            resolve_command_snapshot(input(&env, &label, &paint, &case.activation, case.hovered));
        if case.coherent {
            let snapshot = result.unwrap();
            assert!(std::ptr::eq(snapshot.label(), &label));
            assert!(std::ptr::eq(snapshot.paint(), &paint));
            assert_eq!(snapshot.accessibility().name(), label.text());
            assert_eq!(
                snapshot.accessibility().state().enabled(),
                case.activation.enabled()
            );
            assert_eq!(
                snapshot.accessibility().state().focused(),
                case.activation.focused()
            );
            assert_eq!(
                snapshot.accessibility().actions()[0].available(),
                case.activation.enabled()
            );
            assert_eq!(
                snapshot.hit_region().minimum_size(),
                SurfaceSize {
                    width: 48.0,
                    height: 48.0
                }
            );
        } else {
            assert!(
                matches!(result, Err(CommandSnapshotError::StatesMismatch { .. })),
                "{}: {result:?}",
                case.name
            );
        }
    }
}

#[test]
fn invoke_and_availability_transitions_require_complete_snapshot_replacement() {
    let mut request = request();
    let env = environment(&request);
    let label = label(&request, 64.0, 8.0, env.layout_direction());
    let initial = ActivationState::try_new(true, true, None).unwrap();
    let pressed = resolve_activation(
        &initial,
        &ActivationEvent::PointerDown {
            id: "primary".into(),
            inside: true,
        },
    )
    .unwrap();
    let surface: SurfaceIntent =
        serde_json::from_value(request["surface"]["body"]["surface"].clone()).unwrap();
    request["surface"]["body"]["surface"] =
        serde_json::to_value(surface.with_states(resolve_command_states(pressed.state(), false)))
            .unwrap();
    let pressed_paint = resolve_command_paint_source(&request.to_string()).unwrap();
    resolve_command_snapshot(input(&env, &label, &pressed_paint, pressed.state(), false)).unwrap();
    let invoked = resolve_activation(pressed.state(), &ActivationEvent::Invoke {}).unwrap();
    assert!(invoked.activate());
    assert!(matches!(
        resolve_command_snapshot(input(&env, &label, &pressed_paint, invoked.state(), false)),
        Err(CommandSnapshotError::StatesMismatch { .. })
    ));
    let disabled = resolve_activation(
        invoked.state(),
        &ActivationEvent::Availability { enabled: false },
    )
    .unwrap();
    let surface: SurfaceIntent =
        serde_json::from_value(request["surface"]["body"]["surface"].clone()).unwrap();
    request["surface"]["body"]["surface"] =
        serde_json::to_value(surface.with_states(resolve_command_states(disabled.state(), false)))
            .unwrap();
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let snapshot =
        resolve_command_snapshot(input(&env, &label, &paint, disabled.state(), false)).unwrap();
    assert!(snapshot.accessibility().state().focused());
    assert!(!snapshot.accessibility().actions()[0].available());
    assert!(snapshot.paint().paint().focus().is_some());
    assert!(
        !resolve_activation(
            disabled.state(),
            &ActivationEvent::PointerUp {
                id: "primary".into(),
                inside: true
            }
        )
        .unwrap()
        .activate()
    );
}

#[test]
fn content_target_and_accessibility_failures_retain_their_causes_without_snapshot() {
    let request = request();
    let env = environment(&request);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let state = ActivationState::try_new(true, false, None).unwrap();
    let label = label(&request, 64.0, 8.0, env.layout_direction());
    let oversized = self::label(&request, 80.0, 8.0, env.layout_direction());
    let error =
        resolve_command_snapshot(input(&env, &oversized, &paint, &state, false)).unwrap_err();
    assert!(matches!(
        error,
        CommandSnapshotError::Content(CommandContentError::SizeMismatch)
    ));
    assert!(std::error::Error::source(&error).is_some());
    let unsafe_label = self::label(&request, 64.0, 0.0, env.layout_direction());
    assert!(matches!(
        resolve_command_snapshot(input(&env, &unsafe_label, &paint, &state, false)),
        Err(CommandSnapshotError::Content(
            CommandContentError::OutsideContent
        ))
    ));
    let mut clipped = input(&env, &label, &paint, &state, false);
    clipped.available_bounds = PhysicalBounds {
        x: 0.0,
        y: 0.0,
        width: 64.0,
        height: 40.0,
    };
    let error = resolve_command_snapshot(clipped).unwrap_err();
    assert!(matches!(
        error,
        CommandSnapshotError::HitRegion(HitRegionError::Clipped)
    ));
    assert!(std::error::Error::source(&error).is_some());
    let neighbors = [PhysicalBounds {
        x: 0.0,
        y: 40.0,
        width: 64.0,
        height: 48.0,
    }];
    let mut occupied = input(&env, &label, &paint, &state, false);
    occupied.occupied_regions = &neighbors;
    assert!(matches!(
        resolve_command_snapshot(occupied),
        Err(CommandSnapshotError::HitRegion(HitRegionError::Occupied(0)))
    ));
    let mut inaccessible = input(&env, &label, &paint, &state, false);
    inaccessible.focusable = false;
    let error = resolve_command_snapshot(inaccessible).unwrap_err();
    assert!(matches!(
        error,
        CommandSnapshotError::Accessibility(CommandAccessibilityError::EnabledNotFocusable)
    ));
    assert!(std::error::Error::source(&error).is_some());
    let mut blank = input(&env, &label, &paint, &state, false);
    blank.description = Some(" \n");
    assert!(matches!(
        resolve_command_snapshot(blank),
        Err(CommandSnapshotError::Accessibility(
            CommandAccessibilityError::BlankDescription
        ))
    ));
}

#[test]
fn label_direction_must_match_the_current_environment() {
    let request = request();
    let env = environment(&request);
    assert_eq!(env.layout_direction(), LayoutDirection::Rtl);
    let label = label(&request, 64.0, 8.0, LayoutDirection::Ltr);
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let state = ActivationState::try_new(true, false, None).unwrap();
    assert!(matches!(
        resolve_command_snapshot(input(&env, &label, &paint, &state, false)),
        Err(CommandSnapshotError::DirectionMismatch)
    ));
}

#[test]
fn publication_owns_semantics_without_borrowing_live_activation_or_description() {
    let request = request();
    let env = environment(&request);
    let label = label(&request, 64.0, 8.0, env.layout_direction());
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    let mut state = ActivationState::try_new(true, false, None).unwrap();
    let snapshot = {
        let description = String::from("Reconnect to the saved network");
        let mut request = input(&env, &label, &paint, &state, false);
        request.description = Some(&description);
        resolve_command_snapshot(request).unwrap()
    };
    state = resolve_activation(&state, &ActivationEvent::Availability { enabled: false })
        .unwrap()
        .state()
        .clone();
    assert_eq!(
        snapshot.accessibility().description(),
        Some("Reconnect to the saved network")
    );
    assert!(snapshot.accessibility().actions()[0].available());
    assert!(
        !resolve_activation(&state, &ActivationEvent::Invoke {})
            .unwrap()
            .activate()
    );
}

#[test]
fn sampled_motion_publishes_checked_members_without_rewriting_the_sample() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    request["surface"]["body"]["size"] = json!({"width":64,"height":40});
    request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
        json!(false);
    let state = ActivationState::try_new(
        true,
        true,
        Some(resina_model::PressHold::Pointer {
            id: "primary".into(),
            inside: true,
        }),
    )
    .unwrap();
    let surface: SurfaceIntent =
        serde_json::from_value(request["surface"]["body"]["surface"].clone()).unwrap();
    request["surface"]["body"]["surface"] =
        serde_json::to_value(surface.with_states(resolve_command_states(&state, false))).unwrap();
    let env = environment(&request);
    let label = label(&request, 64.0, 8.0, env.layout_direction());
    for time in [0.0, 0.016, 0.1, 1.0] {
        request["time"] = json!(time);
        let motion = resolve_command_motion_source(&request.to_string()).unwrap();
        let snapshot =
            resolve_command_snapshot(input(&env, &label, motion.command(), &state, false)).unwrap();
        assert!(std::ptr::eq(snapshot.paint(), motion.command()));
        assert!(snapshot.accessibility().actions()[0].available());
        assert!(snapshot.paint().paint().focus().is_some());
        assert_eq!(snapshot.hit_region().bounds().width, label.size().width);
    }
}
