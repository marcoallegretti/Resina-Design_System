#![cfg(feature = "testing")]

use guido::prelude::*;
use resina_guido::{FocusBinding, FocusTransferError, request_focus};
use resina_resolver::{FocusDirection, FocusTarget, FocusTraversalInput, resolve_focus_traversal};
use std::{cell::Cell, rc::Rc};

#[test]
fn native_focus_is_deferred_validated_and_observable() {
    let mut app = guido::testing::Headless::new().expect("GUIdo GPU rendering is required");
    let slot = Rc::new(Cell::new(None));
    let captured = slot.clone();
    let surface = app.surface(SurfaceConfig::new().height(64), move || {
        let first = create_widget_ref();
        let second = create_widget_ref();
        let unbound = create_widget_ref();
        captured.set(Some((first, second, unbound)));
        container()
            .width(128.)
            .height(64.)
            .child(container().width(32.).height(32.).widget_ref(first))
            .child(container().width(32.).height(32.).widget_ref(second))
    });
    app.configure(surface, 128, 64, 1.);
    app.step();
    let (first, second, unbound) = slot.get().unwrap();
    assert!(first.widget().is_some());
    assert!(second.widget().is_some());
    let bindings = [
        FocusBinding {
            id: "é",
            widget: first,
        },
        FocusBinding {
            id: "second",
            widget: second,
        },
    ];
    let targets = [
        FocusTarget {
            id: "é",
            eligible: true,
        },
        FocusTarget {
            id: "second",
            eligible: true,
        },
    ];
    let resolve = |current, wrap| {
        resolve_focus_traversal(FocusTraversalInput {
            targets: &targets,
            current,
            direction: FocusDirection::Forward,
            wrap,
        })
        .unwrap()
    };
    let first_result = resolve(None, false);
    let second_result = resolve(Some("é"), false);
    let no_target = resolve(Some("second"), false);
    let first_request = request_focus(&first_result, &bindings).unwrap().unwrap();
    assert_eq!(first_request.target_id(), "é");
    assert!(!first_request.is_focused().unwrap());
    app.step();
    assert!(first_request.is_focused().unwrap());
    let second_request = request_focus(&second_result, &bindings).unwrap().unwrap();
    assert!(first_request.is_focused().unwrap());
    assert!(!second_request.is_focused().unwrap());
    assert!(request_focus(&no_target, &[]).unwrap().is_none());
    app.step();
    assert!(!first_request.is_focused().unwrap());
    assert!(second_request.is_focused().unwrap());

    let partial_bindings = [
        FocusBinding {
            id: "é",
            widget: first,
        },
        FocusBinding {
            id: "unmounted",
            widget: unbound,
        },
    ];
    request_focus(&first_result, &partial_bindings).unwrap();
    app.step();
    assert!(first_request.is_focused().unwrap());
    request_focus(&first_result, &bindings).unwrap();
    request_focus(&second_result, &bindings).unwrap();
    app.step();
    assert!(second_request.is_focused().unwrap());

    let invalid = [
        (Vec::new(), FocusTransferError::MissingBinding("é".into())),
        (
            vec![FocusBinding {
                id: "é",
                widget: unbound,
            }],
            FocusTransferError::UnboundTarget("é".into()),
        ),
        (
            vec![
                FocusBinding {
                    id: "é",
                    widget: first,
                },
                FocusBinding {
                    id: "",
                    widget: second,
                },
            ],
            FocusTransferError::EmptyId(1),
        ),
        (
            vec![
                FocusBinding {
                    id: "é",
                    widget: first,
                },
                FocusBinding {
                    id: "é",
                    widget: second,
                },
            ],
            FocusTransferError::DuplicateId("é".into()),
        ),
        (
            vec![
                FocusBinding {
                    id: "é",
                    widget: first,
                },
                FocusBinding {
                    id: "alias",
                    widget: first,
                },
            ],
            FocusTransferError::AliasedWidget("alias".into()),
        ),
    ];
    for (invalid_bindings, expected) in invalid {
        request_focus(&first_result, &bindings).unwrap();
        app.step();
        assert!(first_request.is_focused().unwrap());
        request_focus(&second_result, &bindings).unwrap();
        assert!(
            request_focus(&no_target, &invalid_bindings)
                .unwrap()
                .is_none()
        );
        assert!(
            matches!(request_focus(&first_result, &invalid_bindings), Err(error) if error == expected)
        );
        app.step();
        assert!(second_request.is_focused().unwrap());
        assert!(!first_request.is_focused().unwrap());
    }
    request_focus(&first_result, &bindings).unwrap();
    assert!(request_focus(&no_target, &[]).unwrap().is_none());
    app.step();
    assert!(first_request.is_focused().unwrap());
    drop(app);
    assert_eq!(
        first_request.is_focused(),
        Err(FocusTransferError::StaleTarget("é".into()))
    );
}
