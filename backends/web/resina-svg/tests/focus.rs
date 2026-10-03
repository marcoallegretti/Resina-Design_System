use resina_resolver::resolve_focus_ir_source;
use std::{
    io::Write,
    process::{Command, Stdio},
};

const REQUEST: &str = include_str!("../../../../conformance/ir/focus-ir-request.json");

#[test]
fn complete_ring_retains_fractional_color_negative_origin_and_hole() {
    let mut request: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
    let mut theme: serde_json::Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["palette"]["base"]["$value"]["components"] = serde_json::json!([0.8, 0.9, 1]);
    request["theme"]["themeSource"] = serde_json::Value::String(theme.to_string());
    let ir = resolve_focus_ir_source(&request.to_string()).unwrap();
    let svg = resina_svg::render_focus(&ir).unwrap();
    assert!(svg.contains("viewBox=\"-4 -4 28 22\""));
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(svg.contains("fill=\"rgb(80% 90% 100%)\""));
    assert_eq!(svg.matches("M ").count(), 2);
    assert_eq!(svg.matches(" Z").count(), 2);
    assert!(svg.contains(" A 4 4 0 0 1"));
    assert!(svg.contains(" A 2 2 0 0 1"));
    assert!(svg.contains("aria-hidden=\"true\" focusable=\"false\""));
    assert!(!svg.contains("stroke="));
    assert_eq!(svg, resina_svg::render_focus(&ir).unwrap());
}

#[test]
fn cli_preserves_complete_output_and_rejects_invalid_requests_before_writing() {
    let program = env!("CARGO_BIN_EXE_resina-focus-svg");
    let file = Command::new(program)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../conformance/ir/focus-ir-request.json"
        ))
        .output()
        .unwrap();
    assert!(file.status.success());
    assert!(file.stderr.is_empty());
    assert_eq!(
        file.stdout,
        resina_svg::render_focus(&resolve_focus_ir_source(REQUEST).unwrap())
            .unwrap()
            .as_bytes()
    );
    for (input, valid) in [
        (REQUEST.as_bytes(), true),
        (b"{}".as_slice(), false),
        (b"\xff".as_slice(), false),
    ] {
        let mut child = Command::new(program)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.success(), valid);
        if valid {
            assert_eq!(result.stdout, file.stdout);
        } else {
            assert!(result.stdout.is_empty());
            assert!(!result.stderr.is_empty());
        }
    }
}

#[test]
fn valid_ir_with_browser_unrepresentable_navigation_band_fails_explicitly() {
    let mut request: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
    request["size"]["width"] = serde_json::json!(100_000_000);
    let ir = resolve_focus_ir_source(&request.to_string()).unwrap();
    assert!(matches!(
        resina_svg::render_focus(&ir),
        Err(resina_svg::SvgError::CoordinatePrecision(_))
    ));
}
