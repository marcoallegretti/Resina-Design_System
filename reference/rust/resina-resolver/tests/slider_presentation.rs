use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SliderValue, SurfaceSize};
use resina_resolver::{
    SliderAdjustment, SliderAdjustmentInput, SliderAdjustmentIr, SliderEditAction, SliderEditInput,
    SliderEditSession, SliderKey, SliderKeyInput, SliderKeyPolicy, SliderKeySteps,
    SliderLayoutInput, SliderMinimumPosition, SliderOrientation, SliderPresentation,
    SliderPresentationCommitInput, SliderPresentationError, SliderStops, SliderStopsError,
    SliderTieBreak, SliderValueIr, SliderValuePolicy, resolve_slider_adjustment,
    resolve_slider_edit, resolve_slider_key, resolve_slider_layout,
    resolve_slider_presentation_commit, resolve_slider_value,
};

fn value(current: f64) -> SliderValueIr {
    resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, current).unwrap()).unwrap()
}

fn policies() -> [SliderValuePolicy; 2] {
    [
        SliderValuePolicy::Continuous,
        SliderValuePolicy::Stops {
            stops: SliderStops::try_new(-10.0, 30.0, &[-10.0, 0.0, 10.0, 20.0, 30.0]).unwrap(),
            tie_break: SliderTieBreak::Higher,
        },
    ]
}

fn preview(policy: &SliderValuePolicy, origin: f64) -> SliderEditSession {
    let current = value(0.0);
    let session = SliderEditSession::begin(&current, "r0", policy).unwrap();
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
    let result = resolve_slider_edit(SliderEditInput {
        session: &session,
        current: &current,
        revision: "r0",
        value_policy: policy,
        enabled: true,
        read_only: false,
        action: SliderEditAction::Preview {
            layout: &layout,
            desired_origin: origin,
        },
    })
    .unwrap();
    assert!(result.commit().is_none());
    result.session().unwrap().clone()
}

fn adopt(presentation: &SliderPresentation, candidate: &SliderAdjustmentIr) -> SliderAdjustmentIr {
    resolve_slider_presentation_commit(SliderPresentationCommitInput {
        presentation,
        candidate,
        candidate_base: presentation.visible(),
        candidate_revision: presentation.revision(),
        enabled: true,
        read_only: false,
        source_available: true,
    })
    .unwrap()
}

#[test]
fn presentation_requires_current_edit_identity_and_domain() {
    for policy in policies() {
        let current = value(0.0);
        let edit = preview(&policy, 72.0);
        let presentation =
            SliderPresentation::try_new(&current, "r0", &policy, Some(&edit)).unwrap();
        assert_eq!(presentation.committed(), &current);
        assert_eq!(presentation.visible(), &value(10.0));
        assert_eq!(presentation.value_policy(), &policy);
        assert!(presentation.editing());
        for (current, revision) in [(value(20.0), "r0"), (current, "r1")] {
            assert!(matches!(
                SliderPresentation::try_new(&current, revision, &policy, Some(&edit)),
                Err(SliderPresentationError::EditConflict)
            ));
        }
        assert!(matches!(
            SliderPresentation::try_new(&current, "", &policy, None),
            Err(SliderPresentationError::InvalidRevision)
        ));
        let idle = SliderPresentation::try_new(&current, "r0", &policy, None).unwrap();
        assert!(!idle.editing());
        assert_eq!(idle.visible(), idle.committed());
    }
    let stopped = policies()[1].clone();
    assert!(matches!(
        SliderPresentation::try_new(&value(1.0), "r0", &stopped, None),
        Err(SliderPresentationError::ValuePolicy(
            SliderStopsError::CurrentNotAllowed
        ))
    ));
    let edit = preview(&stopped, 72.0);
    assert!(matches!(
        SliderPresentation::try_new(
            &value(0.0),
            "r0",
            &SliderValuePolicy::Continuous,
            Some(&edit)
        ),
        Err(SliderPresentationError::EditConflict)
    ));
}

#[test]
fn endpoint_key_noop_against_preview_commits_change_against_baseline() {
    for policy in policies() {
        let edit = preview(&policy, 132.0);
        let presentation =
            SliderPresentation::try_new(&value(0.0), "r0", &policy, Some(&edit)).unwrap();
        let steps = match policy {
            SliderValuePolicy::Continuous => SliderKeySteps::Continuous {
                step: 2.5,
                page: None,
            },
            SliderValuePolicy::Stops { .. } => SliderKeySteps::Stops {
                step: 1,
                page: None,
            },
        };
        let key_policy = SliderKeyPolicy::try_new(steps, true, true).unwrap();
        let key = resolve_slider_key(SliderKeyInput {
            current: presentation.visible(),
            value_policy: &policy,
            key_policy: &key_policy,
            key: SliderKey::ArrowRight,
            enabled: true,
            read_only: false,
            focused: true,
        })
        .unwrap();
        let candidate = key.commit().unwrap();
        assert!(candidate.accepted());
        assert!(!candidate.changed());
        let commit = adopt(&presentation, candidate);
        assert!(commit.accepted());
        assert!(commit.changed());
        assert_eq!(commit.value(), &value(30.0));
        assert_eq!(presentation.committed(), &value(0.0));
    }
}

#[test]
fn returning_to_committed_value_is_accepted_without_product_change() {
    for policy in policies() {
        let edit = preview(&policy, 72.0);
        let presentation =
            SliderPresentation::try_new(&value(0.0), "r0", &policy, Some(&edit)).unwrap();
        let candidate = resolve_slider_adjustment(SliderAdjustmentInput {
            current: presentation.visible(),
            enabled: true,
            read_only: false,
            adjustment: SliderAdjustment::SetValue(0.0),
        })
        .unwrap();
        assert!(candidate.changed());
        let commit = adopt(&presentation, &candidate);
        assert!(commit.accepted());
        assert!(!commit.changed());
        assert_eq!(commit.value(), presentation.committed());
    }
}

#[test]
fn delivery_rechecks_permissions_source_and_original_acceptance() {
    for policy in policies() {
        let edit = preview(&policy, 72.0);
        let presentation =
            SliderPresentation::try_new(&value(0.0), "r0", &policy, Some(&edit)).unwrap();
        for originally_enabled in [true, false] {
            let candidate = resolve_slider_adjustment(SliderAdjustmentInput {
                current: presentation.visible(),
                enabled: originally_enabled,
                read_only: false,
                adjustment: SliderAdjustment::SetValue(20.0),
            })
            .unwrap();
            for (enabled, read_only, source_available) in [
                (true, false, true),
                (false, false, true),
                (true, true, true),
                (true, false, false),
            ] {
                let commit = resolve_slider_presentation_commit(SliderPresentationCommitInput {
                    presentation: &presentation,
                    candidate: &candidate,
                    candidate_base: presentation.visible(),
                    candidate_revision: "r0",
                    enabled,
                    read_only,
                    source_available,
                })
                .unwrap();
                let accepted = originally_enabled && enabled && !read_only && source_available;
                assert_eq!(commit.accepted(), accepted);
                assert_eq!(commit.changed(), accepted);
                assert_eq!(commit.value(), &value(if accepted { 20.0 } else { 0.0 }));
                assert_eq!(presentation.visible(), &value(10.0));
            }
        }
    }
}

#[test]
fn stale_or_out_of_domain_candidates_fail_before_permission_checks() {
    let policy = policies()[1].clone();
    let edit = preview(&policy, 72.0);
    let presentation =
        SliderPresentation::try_new(&value(0.0), "r0", &policy, Some(&edit)).unwrap();
    let candidate = resolve_slider_adjustment(SliderAdjustmentInput {
        current: presentation.visible(),
        enabled: true,
        read_only: false,
        adjustment: SliderAdjustment::SetValue(15.0),
    })
    .unwrap();
    for (base, revision) in [(value(0.0), "r0"), (value(10.0), "r1"), (value(10.0), "")] {
        assert!(matches!(
            resolve_slider_presentation_commit(SliderPresentationCommitInput {
                presentation: &presentation,
                candidate: &candidate,
                candidate_base: &base,
                candidate_revision: revision,
                enabled: false,
                read_only: true,
                source_available: false,
            }),
            Err(SliderPresentationError::StaleIntent)
        ));
    }
    let error = resolve_slider_presentation_commit(SliderPresentationCommitInput {
        presentation: &presentation,
        candidate: &candidate,
        candidate_base: presentation.visible(),
        candidate_revision: "r0",
        enabled: false,
        read_only: true,
        source_available: false,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        SliderPresentationError::ValuePolicy(SliderStopsError::CurrentNotAllowed)
    ));
    assert!(std::error::Error::source(&error).is_some());
    let foreign = resolve_slider_value(&SliderValue::try_new(0.0, 100.0, 20.0).unwrap()).unwrap();
    let candidate = resolve_slider_adjustment(SliderAdjustmentInput {
        current: &foreign,
        enabled: true,
        read_only: false,
        adjustment: SliderAdjustment::SetValue(20.0),
    })
    .unwrap();
    assert!(matches!(
        resolve_slider_presentation_commit(SliderPresentationCommitInput {
            presentation: &presentation,
            candidate: &candidate,
            candidate_base: presentation.visible(),
            candidate_revision: "r0",
            enabled: false,
            read_only: true,
            source_available: false,
        }),
        Err(SliderPresentationError::IntentBounds)
    ));
}
