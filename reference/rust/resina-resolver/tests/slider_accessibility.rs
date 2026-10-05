use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderValue, SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, SliderAccessibilityInput, SliderAdjustment,
    SliderAdjustmentInput, SliderOrientation, SliderValueIr, compile_theme_source,
    resolve_command_label, resolve_slider_accessibility, resolve_slider_adjustment,
    resolve_slider_value,
};
use resina_resolver::{
    SliderEditAction, SliderEditInput, SliderEditSession, SliderLayoutInput, SliderMinimumPosition,
    SliderPresentation, SliderValuePolicy, resolve_slider_edit, resolve_slider_layout,
};
use serde_json::Value;
use std::convert::Infallible;

fn label(text: &str, direction: LayoutDirection) -> CommandLabelIr {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let theme = compile_theme_source(
        source["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let environment =
        serde_json::from_value(source["surface"]["body"]["theme"]["environment"].clone()).unwrap();
    let typography =
        theme.resolve(&environment).unwrap().typography()[&TypographyRole::Label].clone();
    resolve_command_label(
        CommandLabelInput {
            text,
            typography: &typography,
            minimum_size: SurfaceSize {
                width: 64.0,
                height: 32.0,
            },
            maximum_size: SurfaceSize {
                width: 200.0,
                height: 160.0,
            },
            padding: SafeArea {
                start: 12.0,
                end: 8.0,
                top: 6.0,
                bottom: 10.0,
            },
            direction,
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width: 60.0,
                height: 54.0,
            })
        },
    )
    .unwrap()
}

fn value(current: f64) -> SliderValueIr {
    resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, current).unwrap()).unwrap()
}
#[test]
fn semantic_value_tracks_preview_and_restores_after_cancel_or_conflict() {
    let label = label("Gain", LayoutDirection::Ltr);
    let current = value(0.0);
    let policy = SliderValuePolicy::Continuous;
    let begin = SliderEditSession::begin(&current, "r0", &policy).unwrap();
    let layout = resolve_slider_layout(SliderLayoutInput {
        allocation_size: SurfaceSize {
            width: 160.0,
            height: 40.0,
        },
        thumb_size: SurfaceSize {
            width: 20.0,
            height: 24.0,
        },
        track_thickness: 4.0,
        insets: &SafeArea {
            start: 12.0,
            end: 8.0,
            top: 6.0,
            bottom: 10.0,
        },
        layout_direction: LayoutDirection::Ltr,
        orientation: SliderOrientation::Horizontal,
        minimum_position: SliderMinimumPosition::Start,
        value: &current,
    })
    .unwrap();
    let preview = resolve_slider_edit(SliderEditInput {
        session: &begin,
        current: &current,
        revision: "r0",
        value_policy: &policy,
        enabled: true,
        read_only: false,
        action: SliderEditAction::Preview {
            layout: &layout,
            desired_origin: 72.0,
        },
    })
    .unwrap();
    assert!(preview.commit().is_none());
    let edit = preview.session().unwrap();
    let presentation = SliderPresentation::try_new(&current, "r0", &policy, Some(edit)).unwrap();
    let semantics = |presentation: &SliderPresentation, text: &str| {
        let result = resolve_slider_accessibility(SliderAccessibilityInput {
            label: &label,
            value: presentation.visible(),
            description: None,
            value_text: Some(text),
            enabled: true,
            read_only: false,
            focused: true,
            focusable: true,
            orientation: SliderOrientation::Horizontal,
        })
        .unwrap();
        assert_eq!(result.value(), presentation.visible().value());
        assert_eq!(result.value_text(), Some(text));
        result
    };
    assert_eq!(semantics(&presentation, "10 dB").value().value(), 10.0);
    assert_eq!(presentation.committed(), &current);
    for (replacement, revision, action, expected, text) in [
        (current, "r0", SliderEditAction::Cancel, 0.0, "0 dB"),
        (value(20.0), "r1", SliderEditAction::Commit, 20.0, "20 dB"),
        (current, "r0", SliderEditAction::Commit, 10.0, "10 dB"),
    ] {
        let result = resolve_slider_edit(SliderEditInput {
            session: edit,
            current: &replacement,
            revision,
            value_policy: &policy,
            enabled: true,
            read_only: false,
            action,
        })
        .unwrap();
        assert!(result.session().is_none());
        let committed = result
            .commit()
            .map_or(&replacement, |commit| commit.value());
        let idle = SliderPresentation::try_new(committed, "r2", &policy, None).unwrap();
        assert_eq!(semantics(&idle, text).value().value(), expected);
        assert_eq!(idle.visible(), idle.committed());
    }
}
#[test]
fn public_semantics_cases_preserve_complete_text_value_and_state() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/accessibility/slider-cases.json"
    ))
    .unwrap();
    for case in cases {
        let input = &case["input"];
        let direction = serde_json::from_value(input["labelDirection"].clone()).unwrap();
        let label = label(input["labelText"].as_str().unwrap(), direction);
        let current: SliderValue = serde_json::from_value(input["value"].clone()).unwrap();
        let current = resolve_slider_value(&current).unwrap();
        let orientation: SliderOrientation =
            serde_json::from_value(input["orientation"].clone()).unwrap();
        let result = resolve_slider_accessibility(SliderAccessibilityInput {
            label: &label,
            value: &current,
            description: input["description"].as_str(),
            value_text: input["valueText"].as_str(),
            enabled: input["enabled"].as_bool().unwrap(),
            read_only: input["readOnly"].as_bool().unwrap(),
            focused: input["focused"].as_bool().unwrap(),
            focusable: input["focusable"].as_bool().unwrap(),
            orientation,
        });
        let name = case["name"].as_str().unwrap();
        if let Some(error) = case["error"].as_str() {
            assert_eq!(format!("{:?}", result.unwrap_err()), error, "{name}");
        } else {
            let result = result.unwrap();
            assert_eq!(
                serde_json::to_value(&result).unwrap(),
                case["expected"],
                "{name}"
            );
            assert_eq!(result.role(), "slider");
            assert_eq!(result.name(), label.text());
            assert_eq!(result.value(), current.value());
            assert_eq!(result.description(), input["description"].as_str());
            assert_eq!(result.value_text(), input["valueText"].as_str());
            assert_eq!(
                result.state().enabled(),
                input["enabled"].as_bool().unwrap()
            );
            assert_eq!(
                result.state().read_only(),
                input["readOnly"].as_bool().unwrap()
            );
            assert_eq!(
                result.state().focused(),
                input["focused"].as_bool().unwrap()
            );
            assert_eq!(result.orientation(), orientation);
            assert_eq!(result.focusable(), input["focusable"].as_bool().unwrap());
            for (action, kind) in result
                .actions()
                .iter()
                .zip(["setValue", "increase", "decrease"])
            {
                assert_eq!(action.kind(), kind);
                assert_eq!(
                    action.available(),
                    result.state().enabled() && !result.state().read_only()
                );
            }
        }
    }
}
#[test]
fn semantic_delivery_rechecks_live_permission_and_commits_coherent_value() {
    let label = label("Output level", LayoutDirection::Ltr);
    let snapshot = |current: &SliderValueIr, enabled, read_only, value_text| {
        resolve_slider_accessibility(SliderAccessibilityInput {
            label: &label,
            value: current,
            description: Some("  Adjust playback output.\n"),
            value_text: Some(value_text),
            enabled,
            read_only,
            focused: true,
            focusable: true,
            orientation: SliderOrientation::Horizontal,
        })
        .unwrap()
    };
    let current = value(0.0);
    let before = snapshot(&current, true, false, "0 dB");
    let increased = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &current,
        enabled: true,
        read_only: false,
        adjustment: SliderAdjustment::Increase(10.0),
    })
    .unwrap();
    assert!(increased.accepted() && increased.changed());
    let after = snapshot(increased.value(), true, false, "10 dB");
    assert_eq!(after.name(), before.name());
    assert_eq!(after.description(), before.description());
    assert_eq!(after.state(), before.state());
    assert_eq!(after.value().value(), 10.0);
    assert_eq!(after.value_text(), Some("10 dB"));
    for (enabled, read_only) in [(false, false), (true, true), (false, true)] {
        let unavailable = snapshot(increased.value(), enabled, read_only, "10 dB");
        assert!(before.actions()[0].available());
        assert!(
            unavailable
                .actions()
                .iter()
                .all(|action| !action.available())
        );
        let delivered = resolve_slider_adjustment(SliderAdjustmentInput {
            current: increased.value(),
            enabled,
            read_only,
            adjustment: SliderAdjustment::SetValue(20.0),
        })
        .unwrap();
        assert!(!delivered.accepted() && !delivered.changed());
        assert_eq!(
            unavailable,
            snapshot(delivered.value(), enabled, read_only, "10 dB")
        );
        assert!(unavailable.state().focused());
    }
    let external =
        resolve_slider_value(&SliderValue::try_new(-100.0, 100.0, 20.0).unwrap()).unwrap();
    let external_snapshot = snapshot(&external, true, false, "20 dB");
    assert_eq!(external_snapshot.value().minimum(), -100.0);
    assert_eq!(external_snapshot.value().maximum(), 100.0);
    let delivered = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &external,
        enabled: true,
        read_only: false,
        adjustment: SliderAdjustment::SetValue(80.0),
    })
    .unwrap();
    let updated = snapshot(delivered.value(), true, false, "80 dB");
    assert_eq!(updated.value().value(), 80.0);
    assert_eq!(updated.value().minimum(), -100.0);
    assert_eq!(updated.value().maximum(), 100.0);
    assert_eq!(updated.name(), before.name());
    let endpoint = value(30.0);
    assert!(snapshot(&endpoint, true, false, "30 dB").actions()[1].available());
    let noop = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &endpoint,
        enabled: true,
        read_only: false,
        adjustment: SliderAdjustment::Increase(10.0),
    })
    .unwrap();
    assert!(noop.accepted() && !noop.changed());
}
