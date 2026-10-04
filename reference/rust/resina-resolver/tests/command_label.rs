use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, ResolvedTypography, compile_theme_source, resolve_command_label,
};
use serde_json::{Value, json};
use std::convert::Infallible;
fn style() -> ResolvedTypography {
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
    let env =
        serde_json::from_value(source["surface"]["body"]["theme"]["environment"].clone()).unwrap();
    theme.resolve(&env).unwrap().typography()[&TypographyRole::Label].clone()
}
fn size(width: f64, height: f64) -> SurfaceSize {
    SurfaceSize { width, height }
}
fn input(style: &ResolvedTypography) -> CommandLabelInput<'_> {
    CommandLabelInput {
        text: "Reconnect",
        typography: style,
        minimum_size: size(64.0, 32.0),
        maximum_size: size(200.0, 160.0),
        padding: SafeArea {
            start: 12.0,
            end: 8.0,
            top: 6.0,
            bottom: 10.0,
        },
        direction: LayoutDirection::Ltr,
    }
}
#[test]
fn content_sizing_keeps_the_offered_wrap_width_and_complete_height() {
    let style = style();
    for (natural, fitted, expected) in [
        (size(20.0, 18.0), size(20.0, 18.0), size(64.0, 34.0)),
        (size(90.0, 18.0), size(90.0, 18.0), size(110.0, 34.0)),
        (size(360.0, 18.0), size(172.0, 54.0), size(200.0, 70.0)),
    ] {
        let mut calls = 0;
        let ir = resolve_command_label(input(&style), |request| {
            assert_eq!(request.text, "Reconnect");
            assert_eq!(request.typography, &style);
            calls += 1;
            Ok::<_, Infallible>(if calls == 1 {
                assert_eq!(request.maximum_width, None);
                natural
            } else {
                assert_eq!(request.maximum_width, Some(expected.width - 20.0));
                fitted
            })
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(ir.size(), expected);
        assert_eq!(ir.label_bounds().width, expected.width - 20.0);
        assert_eq!(ir.label_bounds().height, fitted.height);
        assert_eq!(ir.label_bounds().x, 12.0);
        assert_eq!(ir.label_bounds().y, 6.0);
        assert_eq!(ir.text(), "Reconnect");
        assert_eq!(ir.typography(), &style);
        if expected.width == 200.0 {
            let expected_ir: Value = serde_json::from_str(include_str!(
                "../../../../conformance/ir/command-label-ir.json"
            ))
            .unwrap();
            assert_eq!(serde_json::to_value(&ir).unwrap(), expected_ir);
        }
    }
}
#[test]
fn rtl_padding_and_vertical_minimum_slack_preserve_label_extent() {
    let style = style();
    let mut request = input(&style);
    request.direction = LayoutDirection::Rtl;
    request.minimum_size.height = 64.0;
    let ir = resolve_command_label(request, |_| Ok::<_, Infallible>(size(20.0, 18.0))).unwrap();
    assert_eq!(ir.label_bounds().x, 8.0);
    assert_eq!(ir.label_bounds().y, 21.0);
    assert_eq!(ir.layout_direction(), LayoutDirection::Rtl);
}
#[test]
fn invalid_policy_fails_before_measurement_and_measurement_errors_survive() {
    let style = style();
    for index in 0..7 {
        let mut request = input(&style);
        match index {
            0 => request.text = " \n\t",
            1 => request.minimum_size.width = f64::NAN,
            2 => request.maximum_size.height = 0.0,
            3 => request.minimum_size.height = 200.0,
            4 => request.padding.start = -1.0,
            5 => request.padding.bottom = f64::INFINITY,
            _ => request.padding.start = 200.0,
        }
        assert!(
            resolve_command_label(request, |_| -> Result<SurfaceSize, Infallible> {
                panic!("invalid policy reached producer")
            })
            .is_err()
        );
    }
    let result =
        resolve_command_label(input(&style), |_| Err::<SurfaceSize, _>("font unavailable"));
    assert_eq!(
        result.unwrap_err().to_string(),
        "label measurement failed: font unavailable"
    );
    let mut calls = 0;
    let error = resolve_command_label(input(&style), |_| {
        calls += 1;
        if calls == 1 {
            Ok(size(20.0, 18.0))
        } else {
            Err(std::io::Error::other("shaping failed"))
        }
    })
    .unwrap_err();
    assert_eq!(calls, 2);
    assert_eq!(
        std::error::Error::source(&error).unwrap().to_string(),
        "shaping failed"
    );
}

#[test]
fn zero_padding_is_valid_but_positive_extents_must_not_disappear() {
    let style = style();
    let mut request = input(&style);
    request.padding = SafeArea {
        start: 0.0,
        end: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    let ir = resolve_command_label(request, |_| Ok::<_, Infallible>(size(20.0, 18.0))).unwrap();
    assert_eq!(ir.size(), size(64.0, 32.0));
    assert_eq!(ir.label_bounds().width, 64.0);
    assert_eq!(ir.label_bounds().y, 7.0);
    let mut request = input(&style);
    request.padding.start = 1.0e100;
    request.maximum_size.width = 1.0e101;
    let result = resolve_command_label(request, |_| -> Result<SurfaceSize, Infallible> {
        panic!("lost padding reached producer")
    });
    assert_eq!(
        result.unwrap_err().to_string(),
        "horizontal padding exceeds representable arithmetic"
    );
}
#[test]
fn empty_nonfinite_overwide_and_oversized_measurements_fail_without_ir() {
    let style = style();
    for bad in [
        size(0.0, 18.0),
        size(20.0, 0.0),
        size(f64::INFINITY, 18.0),
        size(20.0, f64::NAN),
    ] {
        assert!(resolve_command_label(input(&style), |_| Ok::<_, Infallible>(bad)).is_err());
    }
    for fitted in [size(181.0, 18.0), size(170.0, 200.0)] {
        let mut first = true;
        assert!(
            resolve_command_label(input(&style), |_| {
                let result = if first { size(360.0, 18.0) } else { fitted };
                first = false;
                Ok::<_, Infallible>(result)
            })
            .is_err()
        );
    }
    let mut request = input(&style);
    request.maximum_size.width = f64::MAX;
    assert!(resolve_command_label(request, |_| Ok::<_, Infallible>(size(f64::MAX, 18.0))).is_err());
    assert_eq!(
        serde_json::to_value(
            resolve_command_label(input(&style), |_| Ok::<_, Infallible>(size(20.0, 18.0)))
                .unwrap()
        )
        .unwrap()["schemaVersion"],
        json!("0.1.0")
    );
}
