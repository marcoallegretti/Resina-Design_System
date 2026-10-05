use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalVector, SliderValue, SurfaceSize};
use resina_resolver::{
    HitRegionIr, SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    SliderPointerError, SliderPointerEvent, SliderPointerInput, SliderPointerOutcome,
    SliderPointerRouting, SliderPointerState, SliderPointerTarget, resolve_hit_region_source,
    resolve_slider_layout, resolve_slider_pointer, resolve_slider_value,
};
use resina_resolver::{
    SliderPresentation, SliderStatesError, SliderStatesInput, SliderValuePolicy,
    resolve_slider_states,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
#[path = "common/slider_value_policy.rs"]
mod value_policy;
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Layout {
    allocation_size: SurfaceSize,
    thumb_size: SurfaceSize,
    track_thickness: f64,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    orientation: SliderOrientation,
    minimum_position: SliderMinimumPosition,
    value: SliderValue,
}
impl Layout {
    fn resolve(&self) -> SliderLayoutIr {
        let value = resolve_slider_value(&self.value).unwrap();
        resolve_slider_layout(SliderLayoutInput {
            allocation_size: self.allocation_size,
            thumb_size: self.thumb_size,
            track_thickness: self.track_thickness,
            insets: &self.insets,
            layout_direction: self.layout_direction,
            orientation: self.orientation,
            minimum_position: self.minimum_position,
            value: &value,
        })
        .unwrap_or_else(|error| panic!("{error:?}: {self:?}"))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Event {
    Down {
        id: String,
        point: PhysicalVector,
        target: SliderPointerTarget,
        region: String,
    },
    Move {
        id: String,
        point: PhysicalVector,
    },
    Up {
        id: String,
        point: PhysicalVector,
    },
    RoutingAcquired {
        id: String,
    },
    RoutingLost {
        id: String,
    },
    Cancel {
        id: String,
    },
    Abort,
    Refresh,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Step {
    current: SliderValue,
    revision: String,
    value_policy: value_policy::Policy,
    visual_value: SliderValue,
    layout: String,
    control: String,
    enabled: bool,
    read_only: bool,
    routing: SliderPointerRouting,
    event: Event,
    expected: Option<Value>,
    error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    environment: Value,
    layouts: BTreeMap<String, Layout>,
    regions: BTreeMap<String, Value>,
    steps: Vec<Step>,
}
impl Case {
    fn region(&self, name: &str) -> HitRegionIr {
        let mut request = self.regions.get(name).expect("unknown region").clone();
        request["environment"] = self.environment.clone();
        resolve_hit_region_source(&request.to_string())
            .unwrap_or_else(|e| panic!("{} {name}: {e}", self.name))
    }
    fn layout(&self, step: &Step) -> SliderLayoutIr {
        let mut layout = self
            .layouts
            .get(&step.layout)
            .expect("unknown layout")
            .clone();
        layout.value = step.visual_value;
        layout.resolve()
    }
}
fn corpus() -> Vec<Case> {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-pointer-cases.json"
    ))
    .unwrap();
    assert_eq!(source["schemaVersion"], "0.1.0");
    serde_json::from_value(source["cases"].clone()).unwrap()
}
fn error_name(error: &SliderPointerError) -> &'static str {
    match error {
        SliderPointerError::InvalidRevision => "invalidRevision",
        SliderPointerError::InvalidIdentity => "invalidIdentity",
        SliderPointerError::InvalidPoint => "invalidPoint",
        SliderPointerError::InvalidControlRegion => "invalidControlRegion",
        SliderPointerError::InvalidTargetRegion => "invalidTargetRegion",
        SliderPointerError::IncoherentLayout => "incoherentLayout",
        SliderPointerError::Hit(_) => "hit",
        SliderPointerError::Anchor(_) => "anchor",
        SliderPointerError::Edit(_) => "edit",
        SliderPointerError::ValuePolicy(_) => "valuePolicy",
    }
}

#[test]
fn keyboard_commit_closes_pending_and_acquired_pointer_edits_before_stale_layout() {
    use resina_resolver::{
        SliderKey, SliderKeyInput, SliderKeyPolicy, SliderKeySteps, SliderStops, SliderTieBreak,
        resolve_slider_key,
    };
    let cases = corpus();
    let case = &cases[0];
    let first = &case.steps[0];
    let control = case.region("control");
    let Event::Down { id, point, .. } = &first.event else {
        panic!("fixture must start a press")
    };
    for domain in [
        SliderValuePolicy::Continuous,
        SliderValuePolicy::Stops {
            stops: SliderStops::try_new(-10.0, 30.0, &[-10.0, 0.0, 3.0, 21.0, 30.0]).unwrap(),
            tie_break: SliderTieBreak::Lower,
        },
    ] {
        for (acquired, no_op) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut visual = case.layouts[&first.layout].clone();
            if no_op {
                visual.value = SliderValue::try_new(-10.0, 30.0, 30.0).unwrap();
            }
            let layout = visual.resolve();
            let current = *layout.value();
            let bounds = layout.thumb_bounds();
            let point = if no_op {
                PhysicalVector {
                    x: bounds.x + 3.0,
                    y: bounds.y + 3.0,
                }
            } else {
                *point
            };
            let mut target_request = case.regions["thumb"].clone();
            target_request["visualBounds"] = serde_json::to_value(bounds).unwrap();
            target_request["environment"] = case.environment.clone();
            let target = resolve_hit_region_source(&target_request.to_string()).unwrap();
            let armed = resolve_slider_pointer(SliderPointerInput {
                state: &SliderPointerState::idle(),
                current: &current,
                revision: "r0",
                value_policy: &domain,
                layout: &layout,
                control_region: &control,
                enabled: true,
                read_only: false,
                routing: SliderPointerRouting::Continuous,
                event: SliderPointerEvent::Down {
                    id,
                    point,
                    target: SliderPointerTarget::Thumb,
                    region: &target,
                },
            })
            .unwrap();
            let mut state = armed.state().clone();
            if acquired {
                state = resolve_slider_pointer(SliderPointerInput {
                    state: &state,
                    current: &current,
                    revision: "r0",
                    value_policy: &domain,
                    layout: &layout,
                    control_region: &control,
                    enabled: true,
                    read_only: false,
                    routing: SliderPointerRouting::Continuous,
                    event: SliderPointerEvent::RoutingAcquired { id },
                })
                .unwrap()
                .state()
                .clone();
            }
            let steps = match domain {
                SliderValuePolicy::Continuous => SliderKeySteps::Continuous {
                    step: 2.5,
                    page: None,
                },
                SliderValuePolicy::Stops { .. } => SliderKeySteps::Stops {
                    step: 1,
                    page: None,
                },
            };
            let policy = SliderKeyPolicy::try_new(steps, true, true).unwrap();
            let key = resolve_slider_key(SliderKeyInput {
                current: &current,
                value_policy: &domain,
                key_policy: &policy,
                key: SliderKey::ArrowRight,
                enabled: true,
                read_only: false,
                focused: true,
            })
            .unwrap();
            assert_eq!(key.commit().unwrap().changed(), !no_op);
            let closed = resolve_slider_pointer(SliderPointerInput {
                state: &state,
                current: key.value(),
                revision: if no_op { "r0" } else { "r1" },
                value_policy: &domain,
                layout: &layout,
                control_region: &control,
                enabled: true,
                read_only: false,
                routing: SliderPointerRouting::Continuous,
                event: if no_op {
                    SliderPointerEvent::Abort
                } else {
                    SliderPointerEvent::Refresh
                },
            })
            .unwrap();
            assert_eq!(
                closed.outcome(),
                if no_op {
                    SliderPointerOutcome::Cancelled
                } else {
                    SliderPointerOutcome::Conflict
                }
            );
            assert!(closed.state().held_id().is_none());
            assert!(closed.commit().is_none());
            assert_eq!(closed.preview(), key.value());
            assert_eq!(
                serde_json::to_value(closed.routing()).unwrap(),
                serde_json::json!({"kind":"release","id":id})
            );
        }
    }
}
#[test]
fn public_traces_verify_complete_pointer_state_and_effects() {
    let cases = corpus();
    let state_cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-states-cases.json"
    ))
    .unwrap();
    let mut verified_states = 0;
    assert_eq!(cases.len(), 121);
    let mut steps = 0;
    for case in cases {
        let mut state = SliderPointerState::idle();
        let mut commits = 0;
        for (step_index, step) in case.steps.iter().enumerate() {
            steps += 1;
            let layout = case.layout(step);
            let current = resolve_slider_value(&step.current).unwrap();
            let policy = step.value_policy.resolve();
            let control = case.region(&step.control);
            let target = match &step.event {
                Event::Down { region, .. } => Some(case.region(region)),
                _ => None,
            };
            let event = match &step.event {
                Event::Down {
                    id,
                    point,
                    target: kind,
                    ..
                } => SliderPointerEvent::Down {
                    id,
                    point: *point,
                    target: *kind,
                    region: target.as_ref().unwrap(),
                },
                Event::Move { id, point } => SliderPointerEvent::Move { id, point: *point },
                Event::Up { id, point } => SliderPointerEvent::Up { id, point: *point },
                Event::RoutingAcquired { id } => SliderPointerEvent::RoutingAcquired { id },
                Event::RoutingLost { id } => SliderPointerEvent::RoutingLost { id },
                Event::Cancel { id } => SliderPointerEvent::Cancel { id },
                Event::Abort => SliderPointerEvent::Abort,
                Event::Refresh => SliderPointerEvent::Refresh,
            };
            let result = resolve_slider_pointer(SliderPointerInput {
                value_policy: &policy,
                state: &state,
                current: &current,
                revision: &step.revision,
                layout: &layout,
                control_region: &control,
                enabled: step.enabled,
                read_only: step.read_only,
                routing: step.routing,
                event,
            });
            if let Some(error) = &step.error {
                assert!(step.expected.is_none());
                assert_eq!(error_name(&result.unwrap_err()), error, "{}", case.name);
                continue;
            }
            let result = result.unwrap_or_else(|e| panic!("{} step{steps}: {e}", case.name));
            let expected = step.expected.as_ref().unwrap();
            assert_eq!(
                &serde_json::to_value(&result).unwrap(),
                expected,
                "{} step{steps}",
                case.name
            );
            assert_eq!(
                result.state().held_id(),
                expected["state"]["hold"]["id"].as_str()
            );
            assert_eq!(
                result.state().phase().is_some(),
                result.state().held_id().is_some()
            );
            assert_eq!(
                result.state().target().is_some(),
                result.state().held_id().is_some()
            );
            assert_eq!(
                result.state().edit().is_some(),
                result.state().held_id().is_some()
            );
            if let Some(edit) = result.state().edit() {
                assert_eq!(
                    serde_json::to_value(edit).unwrap(),
                    expected["state"]["hold"]["edit"]
                );
                let presentation =
                    SliderPresentation::try_new(&current, &step.revision, &policy, Some(edit))
                        .unwrap_or_else(|error| panic!("{} step{steps}: {error}", case.name));
                assert_eq!(presentation.committed(), &current);
                assert_eq!(presentation.visible(), result.preview());
                assert!(presentation.editing());
                let projected = resolve_slider_states(SliderStatesInput {
                    presentation: &presentation,
                    pointer: result.state(),
                    enabled: step.enabled,
                    read_only: step.read_only,
                    focused: false,
                    hovered: false,
                    key_pressed: false,
                })
                .unwrap();
                let signals = if expected["state"]["hold"]["phase"] == "acquired" {
                    vec!["pressed", "dragging"]
                } else {
                    vec!["pressed"]
                };
                assert_eq!(
                    serde_json::to_value(projected).unwrap(),
                    serde_json::json!({"schemaVersion":"0.1.0", "states":signals})
                );
                let idle_presentation =
                    SliderPresentation::try_new(&current, &step.revision, &policy, None).unwrap();
                assert!(matches!(
                    resolve_slider_states(SliderStatesInput {
                        presentation: &idle_presentation,
                        pointer: result.state(),
                        enabled: true,
                        read_only: false,
                        focused: false,
                        hovered: false,
                        key_pressed: false,
                    }),
                    Err(SliderStatesError::IncoherentPresentation)
                ));
                let stale_presentation =
                    SliderPresentation::try_new(&current, "different revision", &policy, None)
                        .unwrap();
                assert!(matches!(
                    resolve_slider_states(SliderStatesInput {
                        presentation: &stale_presentation,
                        pointer: result.state(),
                        enabled: false,
                        read_only: true,
                        focused: false,
                        hovered: false,
                        key_pressed: false,
                    }),
                    Err(SliderStatesError::Presentation(_))
                ));
                for vector in state_cases["cases"].as_array().unwrap() {
                    if vector["pointerTrace"] != case.name || vector["stepIndex"] != step_index {
                        continue;
                    }
                    let result = resolve_slider_states(SliderStatesInput {
                        presentation: &presentation,
                        pointer: result.state(),
                        enabled: vector["enabled"].as_bool().unwrap(),
                        read_only: vector["readOnly"].as_bool().unwrap(),
                        focused: vector["focused"].as_bool().unwrap(),
                        hovered: vector["hovered"].as_bool().unwrap(),
                        key_pressed: vector["keyPressed"].as_bool().unwrap(),
                    });
                    if let Some(error) = vector["error"].as_str() {
                        let actual = match result.unwrap_err() {
                            SliderStatesError::UnavailableHold => "unavailableHold",
                            error => panic!("unexpected state error: {error}"),
                        };
                        assert_eq!(actual, error);
                    } else {
                        assert_eq!(
                            serde_json::to_value(result.unwrap()).unwrap(),
                            vector["expected"]
                        );
                    }
                    verified_states += 1;
                }
            }
            assert_eq!(
                result.commit().is_some(),
                result.outcome() == SliderPointerOutcome::Committed
            );
            assert_eq!(result.routing().is_some(), !expected["routing"].is_null());
            assert_eq!(
                serde_json::to_value(result.preview()).unwrap(),
                expected["preview"]
            );
            if let Some(commit) = result.commit() {
                commits += 1;
                assert!(commit.accepted());
                assert_eq!(commit.value(), result.preview());
                assert!(result.state().held_id().is_none());
            }
            state = result.state().clone();
        }
        assert!(
            state.held_id().is_none(),
            "{}: trace did not close",
            case.name
        );
        assert!(commits <= 1, "{}: duplicate commit", case.name);
    }
    assert_eq!(steps, 438);
    assert_eq!(
        verified_states,
        state_cases["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| !case["pointerTrace"].is_null())
            .count()
    );
}
#[test]
fn malformed_points_fail_before_permission_identity_and_conflict_guards() {
    let case = corpus().remove(0);
    let layout = case.layouts["normal"].resolve();
    let current = *layout.value();
    let control = case.region("control");
    let target = case.region("thumb");
    let state = resolve_slider_pointer(SliderPointerInput {
        value_policy: &SliderValuePolicy::Continuous,
        state: &SliderPointerState::idle(),
        current: &current,
        revision: "r0",
        layout: &layout,
        control_region: &control,
        enabled: true,
        read_only: false,
        routing: SliderPointerRouting::Continuous,
        event: SliderPointerEvent::Down {
            id: "held",
            point: PhysicalVector { x: 45.0, y: 18.0 },
            target: SliderPointerTarget::Thumb,
            region: &target,
        },
    })
    .unwrap()
    .state()
    .clone();
    for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for point in [
            PhysicalVector { x: n, y: 18.0 },
            PhysicalVector { x: 45.0, y: n },
        ] {
            for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)]
            {
                for event in [
                    SliderPointerEvent::Move {
                        id: "foreign",
                        point,
                    },
                    SliderPointerEvent::Up { id: "held", point },
                    SliderPointerEvent::Down {
                        id: "other",
                        point,
                        target: SliderPointerTarget::Track,
                        region: &control,
                    },
                ] {
                    let error = resolve_slider_pointer(SliderPointerInput {
                        value_policy: &SliderValuePolicy::Continuous,
                        state: &state,
                        current: &current,
                        revision: "changed",
                        layout: &layout,
                        control_region: &control,
                        enabled,
                        read_only,
                        routing: SliderPointerRouting::Unavailable,
                        event,
                    })
                    .unwrap_err();
                    assert!(matches!(error, SliderPointerError::InvalidPoint));
                    assert!(error.to_string().contains("finite"));
                    assert!(std::error::Error::source(&error).is_none());
                }
            }
        }
    }
    assert_eq!(state.held_id(), Some("held"));
}
#[test]
fn numeric_preview_errors_keep_ownership_until_explicit_abort() {
    let case = corpus().remove(0);
    let mut visual = case.layouts["normal"].clone();
    visual.value = SliderValue::try_new(1.0, 2.0, 1.5).unwrap();
    let layout = visual.resolve();
    let current = *layout.value();
    let control = case.region("control");
    let mut state = SliderPointerState::idle();
    for event in [
        SliderPointerEvent::Down {
            id: "press",
            point: PhysicalVector { x: 75.0, y: 18.0 },
            target: SliderPointerTarget::Thumb,
            region: &control,
        },
        SliderPointerEvent::RoutingAcquired { id: "press" },
    ] {
        state = resolve_slider_pointer(SliderPointerInput {
            value_policy: &SliderValuePolicy::Continuous,
            state: &state,
            current: &current,
            revision: "r0",
            layout: &layout,
            control_region: &control,
            enabled: true,
            read_only: false,
            routing: SliderPointerRouting::Continuous,
            event,
        })
        .unwrap()
        .state()
        .clone();
    }
    let before = serde_json::to_value(&state).unwrap();
    let error = resolve_slider_pointer(SliderPointerInput {
        value_policy: &SliderValuePolicy::Continuous,
        state: &state,
        current: &current,
        revision: "r0",
        layout: &layout,
        control_region: &control,
        enabled: true,
        read_only: false,
        routing: SliderPointerRouting::Continuous,
        event: SliderPointerEvent::Move {
            id: "press",
            point: PhysicalVector {
                x: 75.0_f64.next_up(),
                y: 18.0,
            },
        },
    })
    .unwrap_err();
    assert!(matches!(error, SliderPointerError::Edit(_)));
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    let result = resolve_slider_pointer(SliderPointerInput {
        value_policy: &SliderValuePolicy::Continuous,
        state: &state,
        current: &current,
        revision: "r0",
        layout: &layout,
        control_region: &control,
        enabled: true,
        read_only: false,
        routing: SliderPointerRouting::Continuous,
        event: SliderPointerEvent::Abort,
    })
    .unwrap();
    assert!(result.state().held_id().is_none());
    assert!(result.commit().is_none());
    assert_eq!(result.preview(), &current);
    assert_eq!(
        serde_json::to_value(result.routing()).unwrap(),
        serde_json::json!({"kind":"release","id":"press"})
    );
}

#[test]
fn unrepresentable_track_center_fails_even_when_starting_unavailable() {
    let case = corpus().remove(0);
    let mut visual = case.layouts["normal"].clone();
    visual.allocation_size.width = 2.0_f64.powi(53) + 4.0;
    visual.thumb_size.width = 2.0;
    visual.insets.start = 2.0;
    visual.insets.end = 0.0;
    visual.value = SliderValue::try_new(0.0, 1.0, 1.0 - 2.0_f64.powi(-52)).unwrap();
    let layout = visual.resolve();
    let mut request = case.regions["control"].clone();
    request["environment"] = case.environment.clone();
    request["visualBounds"]["width"] = serde_json::json!(visual.allocation_size.width);
    request["availableBounds"]["width"] = serde_json::json!(visual.allocation_size.width * 2.0);
    let region = resolve_hit_region_source(&request.to_string()).unwrap();
    for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)] {
        let error = resolve_slider_pointer(SliderPointerInput {
            value_policy: &SliderValuePolicy::Continuous,
            state: &SliderPointerState::idle(),
            current: layout.value(),
            revision: "r0",
            layout: &layout,
            control_region: &region,
            enabled,
            read_only,
            routing: SliderPointerRouting::Unavailable,
            event: SliderPointerEvent::Down {
                id: "press",
                point: PhysicalVector {
                    x: 2.0_f64.powi(53),
                    y: 18.0,
                },
                target: SliderPointerTarget::Track,
                region: &region,
            },
        })
        .unwrap_err();
        assert!(matches!(error, SliderPointerError::Anchor(_)));
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn overflowing_grab_displacement_requires_abort_without_partial_preview() {
    let case = corpus().remove(0);
    let layout = case.layouts["normal"].resolve();
    let mut request = case.regions["control"].clone();
    request["environment"] = case.environment.clone();
    request["componentMinimum"]["width"] = serde_json::json!(1e308);
    request["availableBounds"]["x"] = serde_json::json!(-7e307);
    request["availableBounds"]["width"] = serde_json::json!(1.7e308);
    let region = resolve_hit_region_source(&request.to_string()).unwrap();
    let mut state = SliderPointerState::idle();
    for event in [
        SliderPointerEvent::Down {
            id: "press",
            point: PhysicalVector { x: -4e307, y: 18.0 },
            target: SliderPointerTarget::Thumb,
            region: &region,
        },
        SliderPointerEvent::RoutingAcquired { id: "press" },
    ] {
        state = resolve_slider_pointer(SliderPointerInput {
            value_policy: &SliderValuePolicy::Continuous,
            state: &state,
            current: layout.value(),
            revision: "r0",
            layout: &layout,
            control_region: &region,
            enabled: true,
            read_only: false,
            routing: SliderPointerRouting::Continuous,
            event,
        })
        .unwrap()
        .state()
        .clone();
    }
    let before = serde_json::to_value(&state).unwrap();
    let error = resolve_slider_pointer(SliderPointerInput {
        value_policy: &SliderValuePolicy::Continuous,
        state: &state,
        current: layout.value(),
        revision: "r0",
        layout: &layout,
        control_region: &region,
        enabled: true,
        read_only: false,
        routing: SliderPointerRouting::Continuous,
        event: SliderPointerEvent::Move {
            id: "press",
            point: PhysicalVector {
                x: f64::MAX,
                y: 18.0,
            },
        },
    })
    .unwrap_err();
    assert!(matches!(error, SliderPointerError::Anchor(_)));
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    let result = resolve_slider_pointer(SliderPointerInput {
        value_policy: &SliderValuePolicy::Continuous,
        state: &state,
        current: layout.value(),
        revision: "r0",
        layout: &layout,
        control_region: &region,
        enabled: true,
        read_only: false,
        routing: SliderPointerRouting::Continuous,
        event: SliderPointerEvent::Abort,
    })
    .unwrap();
    assert!(result.state().held_id().is_none());
    assert!(result.commit().is_none());
    assert_eq!(result.preview(), layout.value());
    assert!(result.routing().is_some());
}
