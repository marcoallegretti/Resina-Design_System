use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderValue, SurfaceSize};
use resina_resolver::{
    SliderEditAction, SliderEditError, SliderEditInput, SliderEditOutcome, SliderEditSession,
    SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    resolve_slider_edit, resolve_slider_layout, resolve_slider_value,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Begin {
    value: SliderValue,
    revision: String,
}
#[derive(Deserialize)]
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
        .unwrap()
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Action {
    Preview {
        layout: Layout,
        #[serde(rename = "desiredOrigin")]
        desired_origin: f64,
    },
    Commit,
    Cancel,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Step {
    current: SliderValue,
    revision: String,
    enabled: bool,
    read_only: bool,
    action: Action,
    expected: Option<Value>,
    error: Option<String>,
}
fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/interaction/slider-edit-cases.json"
    ))
    .unwrap()
}
#[test]
fn public_traces_verify_complete_preview_commit_and_abort_results() {
    let source = corpus();
    assert_eq!(source["schemaVersion"], "0.1.0");
    let mut verified_steps = 0;
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let begin: Begin = serde_json::from_value(case["begin"].clone()).unwrap();
        let baseline = resolve_slider_value(&begin.value).unwrap();
        let mut session = Some(SliderEditSession::begin(&baseline, &begin.revision).unwrap());
        let mut commits = 0;
        for raw in case["steps"].as_array().unwrap() {
            verified_steps += 1;
            let step: Step = serde_json::from_value(raw.clone()).unwrap();
            let current = resolve_slider_value(&step.current).unwrap();
            let layout = match &step.action {
                Action::Preview { layout, .. } => Some(layout.resolve()),
                _ => None,
            };
            let action = match step.action {
                Action::Preview { desired_origin, .. } => SliderEditAction::Preview {
                    layout: layout.as_ref().unwrap(),
                    desired_origin,
                },
                Action::Commit => SliderEditAction::Commit,
                Action::Cancel => SliderEditAction::Cancel,
            };
            let result = resolve_slider_edit(SliderEditInput {
                session: session.as_ref().expect("trace already closed"),
                current: &current,
                revision: &step.revision,
                enabled: step.enabled,
                read_only: step.read_only,
                action,
            });
            if let Some(error) = step.error {
                let actual = match result.expect_err(name) {
                    SliderEditError::InvalidRevision => "invalidRevision",
                    SliderEditError::InvalidPosition => "invalidPosition",
                    SliderEditError::IncoherentLayout => "incoherentLayout",
                    SliderEditError::Position(_) => "position",
                    SliderEditError::Adjustment(_) => "adjustment",
                };
                assert_eq!(actual, error, "{name}");
                assert!(step.expected.is_none());
            } else {
                let result = result.expect(name);
                let expected = step.expected.unwrap();
                assert_eq!(serde_json::to_value(&result).unwrap(), expected, "{name}");
                assert_eq!(
                    result.session().is_some(),
                    result.outcome() == SliderEditOutcome::Previewed
                );
                assert_eq!(
                    result.commit().is_some(),
                    result.outcome() == SliderEditOutcome::Committed
                );
                assert_eq!(
                    serde_json::to_value(result.preview()).unwrap(),
                    expected["preview"]
                );
                if let Some(next) = result.session() {
                    assert_eq!(next.baseline(), &baseline);
                    assert_eq!(next.revision(), begin.revision);
                    assert_eq!(next.preview(), result.preview());
                }
                if let Some(commit) = result.commit() {
                    commits += 1;
                    assert!(commit.accepted());
                    assert_eq!(commit.value(), result.preview());
                }
                session = result.session().cloned();
            }
        }
        assert!(session.is_none(), "{name}: terminal trace must close");
        assert!(commits <= 1, "{name}: commit belongs only to completion");
    }
    assert_eq!(verified_steps, 121);
}
#[test]
fn revision_changes_detect_change_and_return_without_overwriting_current() {
    let baseline = resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 0.0).unwrap()).unwrap();
    let session = SliderEditSession::begin(&baseline, "first").unwrap();
    for action in [SliderEditAction::Commit, SliderEditAction::Cancel] {
        let result = resolve_slider_edit(SliderEditInput {
            session: &session,
            current: &baseline,
            revision: "third",
            enabled: true,
            read_only: false,
            action,
        })
        .unwrap();
        assert_eq!(
            result.outcome(),
            if matches!(action, SliderEditAction::Cancel) {
                SliderEditOutcome::Cancelled
            } else {
                SliderEditOutcome::Conflict
            }
        );
        assert_eq!(result.preview(), &baseline);
        assert!(result.session().is_none());
        assert!(result.commit().is_none());
    }
}
#[test]
fn nonfinite_preview_positions_fail_before_permission_and_conflict_guards() {
    let source = corpus();
    let begin: Begin = serde_json::from_value(source["cases"][0]["begin"].clone()).unwrap();
    let baseline = resolve_slider_value(&begin.value).unwrap();
    let session = SliderEditSession::begin(&baseline, "first").unwrap();
    let first: Step = serde_json::from_value(source["cases"][0]["steps"][0].clone()).unwrap();
    let Action::Preview { layout, .. } = first.action else {
        panic!("fixture must preview")
    };
    let layout = layout.resolve();
    for desired_origin in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for revision in ["first", "third"] {
            for (enabled, read_only) in [(true, false), (false, false), (true, true), (false, true)]
            {
                let error = resolve_slider_edit(SliderEditInput {
                    session: &session,
                    current: &baseline,
                    revision,
                    enabled,
                    read_only,
                    action: SliderEditAction::Preview {
                        layout: &layout,
                        desired_origin,
                    },
                })
                .unwrap_err();
                assert!(matches!(error, SliderEditError::InvalidPosition));
                assert!(std::error::Error::source(&error).is_none());
                assert!(error.to_string().contains("finite"));
            }
        }
    }
}
#[test]
fn begin_requires_explicit_opaque_revision_and_preserves_complete_baseline() {
    let value = resolve_slider_value(&SliderValue::try_new(0.0, 100.0, 25.0).unwrap()).unwrap();
    assert!(matches!(
        SliderEditSession::begin(&value, ""),
        Err(SliderEditError::InvalidRevision)
    ));
    for revision in ["r0", " r0 ", "révision", "\u{200b}"] {
        let session = SliderEditSession::begin(&value, revision).unwrap();
        assert_eq!(session.revision(), revision);
        assert_eq!(session.baseline(), &value);
        assert_eq!(session.preview(), &value);
        assert_eq!(
            serde_json::to_value(&session).unwrap()["schemaVersion"],
            "0.1.0"
        );
    }
}
