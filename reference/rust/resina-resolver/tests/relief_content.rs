use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandContentError, CommandLabelInput, CommandLabelIr, ReliefPaint, compile_theme_source,
    resolve_command_label, resolve_relief_paint_source,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;

fn paint(kind: &str, curve: &str, radius: f64, shoulder: f64) -> ReliefPaint {
    let cases: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let mut request = cases[0]["request"].clone();
    request["outline"]["curvature"] = json!(curve);
    for name in ["topStart", "topEnd", "bottomEnd", "bottomStart"] {
        request["outline"]["radii"][name] = json!({"x":radius,"y":radius});
    }
    request["profile"] = json!({"kind":kind,"bodyThickness":4,
        "amplitude":if kind == "flat" {0.0} else {shoulder * 0.25},
        "shoulderWidth":if kind == "flat" {0.0} else {shoulder}});
    request["points"] = json!([]);
    resolve_relief_paint_source(&request.to_string())
        .unwrap()
        .paint
}

fn label(
    size: SurfaceSize,
    start: f64,
    end: f64,
    height: f64,
    direction: LayoutDirection,
) -> CommandLabelIr {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let theme = &source["surface"]["body"]["theme"];
    let resolved = compile_theme_source(theme["themeSource"].as_str().unwrap())
        .unwrap()
        .resolve(&serde_json::from_value(theme["environment"].clone()).unwrap())
        .unwrap();
    resolve_command_label(
        CommandLabelInput {
            text: "Reconnect",
            typography: &resolved.typography()[&TypographyRole::Label],
            minimum_size: size,
            maximum_size: size,
            padding: SafeArea {
                start,
                end,
                top: 0.0,
                bottom: 0.0,
            },
            direction,
        },
        |_| Ok::<_, Infallible>(SurfaceSize { width: 1.0, height }),
    )
    .unwrap()
}

#[test]
fn uniform_relief_content_requires_the_same_face_size_and_complete_label_box() {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Matrix {
        schema_version: String,
        size: SurfaceSize,
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Case {
        name: String,
        kind: String,
        curvature: String,
        radius: f64,
        shoulder_width: f64,
        start: f64,
        end: f64,
        height: f64,
        contained: bool,
    }
    let matrix: Matrix = serde_json::from_str(include_str!(
        "../../../../conformance/content/command-relief-containment.json"
    ))
    .unwrap();
    assert_eq!(matrix.schema_version, "0.1.0");
    assert_eq!(matrix.cases.len(), 16);
    for case in matrix.cases {
        let paint = paint(
            &case.kind,
            &case.curvature,
            case.radius,
            case.shoulder_width,
        );
        let size = paint.surface().outline().size();
        assert_eq!(size, matrix.size);
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let contained = label(size, case.start, case.end, case.height, direction);
            let result = contained.validate_relief_content(&paint);
            assert_eq!(
                result.is_ok(),
                case.contained,
                "{} {direction:?}: {result:?}",
                case.name
            );
            if !case.contained {
                assert_eq!(result, Err(CommandContentError::OutsideContent));
            }
            assert_eq!(
                contained.label_bounds().x,
                if direction == LayoutDirection::Ltr {
                    case.start
                } else {
                    case.end
                }
            );
            let wrong_size = label(
                SurfaceSize {
                    width: size.width + 1.0,
                    height: size.height,
                },
                14.0,
                18.0,
                20.0,
                direction,
            );
            assert_eq!(
                wrong_size.validate_relief_content(&paint),
                Err(CommandContentError::SizeMismatch)
            );
        }
    }
}

#[test]
fn square_plateau_boundary_is_closed_and_the_adjacent_shoulder_is_rejected() {
    for kind in ["convex", "recessed"] {
        let paint = paint(kind, "quartic", 0.0, 6.0);
        let size = paint.surface().outline().size();
        assert_eq!(
            paint.content_bounds(),
            resina_model::PhysicalBounds {
                x: 6.0,
                y: 6.0,
                width: 108.0,
                height: 48.0
            }
        );
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            let boundary = label(size, 6.0, 6.0, 48.0, direction);
            assert_eq!(boundary.validate_relief_content(&paint), Ok(()));
            let shoulder = label(size, 6.0, 6.0, 48.0_f64.next_up(), direction);
            assert!(shoulder.label_bounds().y < 6.0);
            assert_eq!(
                shoulder.validate_relief_content(&paint),
                Err(CommandContentError::OutsideContent)
            );
        }
    }
}
