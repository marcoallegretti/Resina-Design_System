use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandContentError, CommandLabelInput, CommandLabelIr, compile_theme_source,
    resolve_command_label, resolve_command_motion_source, resolve_command_paint_source,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;

fn source() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap()
}
fn label(source: &Value, padding: SafeArea, height: f64) -> CommandLabelIr {
    let body = &source["surface"]["body"];
    let theme = compile_theme_source(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
    let env = serde_json::from_value(body["theme"]["environment"].clone()).unwrap();
    let resolved = theme.resolve(&env).unwrap();
    let size: SurfaceSize = serde_json::from_value(body["size"].clone()).unwrap();
    let direction =
        serde_json::from_value(body["theme"]["environment"]["layoutDirection"].clone()).unwrap();
    resolve_command_label(
        CommandLabelInput {
            text: "Reconnect",
            typography: &resolved.typography()[&TypographyRole::Label],
            minimum_size: size,
            maximum_size: size,
            padding,
            direction,
        },
        |_| {
            Ok::<_, Infallible>(SurfaceSize {
                width: size.width * 0.01,
                height,
            })
        },
    )
    .unwrap()
}
fn padding(start: f64, end: f64) -> SafeArea {
    SafeArea {
        start,
        end,
        top: 0.0,
        bottom: 0.0,
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Matrix {
    schema_version: String,
    size: SurfaceSize,
    rounded_radius: f64,
    edge_width: f64,
    highlight_width: f64,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    shape: String,
    padding: f64,
    height: f64,
    contained: bool,
}
#[test]
fn public_containment_cases_use_actual_curved_content_not_its_bounds() {
    let matrix: Matrix = serde_json::from_str(include_str!(
        "../../../../conformance/content/command-label-containment.json"
    ))
    .unwrap();
    assert_eq!(matrix.schema_version, "0.1.0");
    assert_eq!(matrix.cases.len(), 7);
    for case in matrix.cases {
        for direction in ["ltr", "rtl"] {
            let mut request = source();
            let body = &mut request["surface"]["body"];
            body["size"] = serde_json::to_value(matrix.size).unwrap();
            body["theme"]["environment"]["layoutDirection"] = json!(direction);
            body["surface"]["form"]["shape"] = json!(case.shape);
            let mut theme: Value =
                serde_json::from_str(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
            theme["tokens"]["radius"]["3"]["$value"]["value"] = json!(matrix.rounded_radius);
            body["theme"]["themeSource"] = json!(theme.to_string());
            body["appearance"]["shapeAssignments"]["profiles"]["rounded"]["radius"] =
                json!("radius.3");
            body["appearance"]["bands"]["elastomer"] = json!({
                "edgeWidth": matrix.edge_width, "highlightWidth": matrix.highlight_width
            });
            let label = label(&request, padding(case.padding, case.padding), case.height);
            let paint = resolve_command_paint_source(&request.to_string()).unwrap();
            let result = label.validate_content(paint.paint().body());
            assert_eq!(
                result.is_ok(),
                case.contained,
                "{} {direction}: {result:?}",
                case.name
            );
            if !case.contained {
                assert_eq!(result, Err(CommandContentError::OutsideContent));
            }
        }
    }
}
#[test]
fn physical_padding_and_front_size_are_checked_independently_of_extrusion() {
    for direction in ["ltr", "rtl"] {
        let mut request = source();
        request["surface"]["body"]["size"] = json!({"width":64,"height":40});
        request["surface"]["body"]["theme"]["environment"]["layoutDirection"] = json!(direction);
        let label = label(&request, padding(2.0, 5.0), 36.0);
        assert_eq!(
            label.layout_direction(),
            if direction == "ltr" {
                LayoutDirection::Ltr
            } else {
                LayoutDirection::Rtl
            }
        );
        assert_eq!(
            label.label_bounds().x,
            if direction == "ltr" { 2.0 } else { 5.0 }
        );
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        assert_ne!(
            paint.paint().body().geometry().front().bounds(),
            paint.paint().body().geometry().silhouette().bounds()
        );
        label.validate_content(paint.paint().body()).unwrap();
        for axis in ["width", "height"] {
            let mut mismatch = request.clone();
            mismatch["surface"]["body"]["size"][axis] = json!(80.0);
            let paint = resolve_command_paint_source(&mismatch.to_string()).unwrap();
            assert_eq!(
                label.validate_content(paint.paint().body()),
                Err(CommandContentError::SizeMismatch)
            );
        }
    }
}
#[test]
fn all_command_phases_and_supported_opaque_materials_keep_containment() {
    for family in ["cast", "frost", "elastomer"] {
        for state in ["rest", "hover", "pressed", "disabled"] {
            let mut request = source();
            request["surface"]["body"]["size"] = json!({"width":64,"height":40});
            let mut theme: Value = serde_json::from_str(
                request["surface"]["body"]["theme"]["themeSource"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            theme["materialAssignments"]["control"]["interactive"] = json!(family);
            request["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
            request["surface"]["body"]["surface"]["states"]["states"] = json!([state]);
            let label = label(&request, padding(8.0, 8.0), 24.0);
            let paint = resolve_command_paint_source(&request.to_string()).unwrap();
            label.validate_content(paint.paint().body()).unwrap();
        }
    }
}
#[test]
fn sampled_motion_body_uses_the_same_containment_contract() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    request["surface"]["body"]["size"] = json!({"width":64,"height":40});
    request["surface"]["body"]["theme"]["environment"]["accessibilityPreferences"]["reducedMotion"] =
        json!(false);
    let label = label(&request, padding(8.0, 8.0), 24.0);
    for time in [0.0, 0.016, 0.1, 1.0] {
        request["time"] = json!(time);
        let motion = resolve_command_motion_source(&request.to_string()).unwrap();
        label
            .validate_content(motion.command().paint().body())
            .unwrap();
    }
}

#[test]
fn circular_boundary_is_closed_and_nearby_outside_boxes_are_rejected() {
    let mut request = source();
    request["surface"]["body"]["size"] = json!({"width":14,"height":14});
    request["surface"]["body"]["surface"]["form"]["shape"] = json!("capsule");
    let paint = resolve_command_paint_source(&request.to_string()).unwrap();
    // The inset circle has radius 5; each box corner has offsets 3 and 4.
    label(&request, padding(4.0, 4.0), 8.0)
        .validate_content(paint.paint().body())
        .unwrap();
    assert_eq!(
        label(&request, padding(3.99, 3.99), 8.0).validate_content(paint.paint().body()),
        Err(CommandContentError::OutsideContent)
    );
    assert_eq!(
        label(&request, padding(4.0, 4.0), 8.02).validate_content(paint.paint().body()),
        Err(CommandContentError::OutsideContent)
    );
}
#[test]
fn containment_preserves_small_and_large_representable_extents() {
    for scale in [1.0e-80, 1.0, 1.0e80] {
        let mut request = source();
        let body = &mut request["surface"]["body"];
        body["size"] = json!({"width":64.0*scale,"height":40.0*scale});
        body["surface"]["form"]["elevation"] = json!("base");
        body["appearance"]["bands"]["elastomer"] =
            json!({"edgeWidth":scale,"highlightWidth":scale});
        let paint = resolve_command_paint_source(&request.to_string()).unwrap();
        label(&request, padding(2.0 * scale, 2.0 * scale), 32.0 * scale)
            .validate_content(paint.paint().body())
            .unwrap();
        assert_eq!(
            label(&request, padding(scale, scale), 32.0 * scale)
                .validate_content(paint.paint().body()),
            Err(CommandContentError::OutsideContent)
        );
    }
}
