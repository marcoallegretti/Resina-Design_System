#![cfg(feature = "png")]

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn invoke(input: &[u8], options: &[&str]) -> Output {
    invoke_program(env!("CARGO_BIN_EXE_resina-surface-raster"), input, options)
}

fn invoke_program(program: &str, input: &[u8], options: &[&str]) -> Output {
    let mut child = Command::new(program)
        .arg("-")
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

const REQUEST: &[u8] = include_bytes!("../../../../conformance/ir/opaque-surface-request.json");
const OPTIONS: &[&str] = &["24", "18", "-2", "-2", "1", "2"];

#[test]
fn complete_paint_cli_contains_body_ring_and_transparent_gap() {
    let mut body: serde_json::Value = serde_json::from_slice(REQUEST).unwrap();
    body["surface"]["states"]["states"] = serde_json::json!(["focused"]);
    let mut request = serde_json::json!({"schemaVersion": "0.1.0", "body": body,
        "surroundingColor": {"colorSpace": "srgb", "components": [0, 0, 0], "alpha": 1}});
    let program = env!("CARGO_BIN_EXE_resina-paint-raster");
    let options = ["40", "32", "-8", "-8", "1", "4"];
    let output = invoke_program(program, request.to_string().as_bytes(), &options);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let mut reader = png::Decoder::new(std::io::Cursor::new(output.stdout))
        .read_info()
        .unwrap();
    assert_eq!(
        reader.info().srgb,
        Some(png::SrgbRenderingIntent::Perceptual)
    );
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!((frame.width, frame.height), (40, 32));
    for (x, y, expected) in [
        (18, 14, [255; 4]),
        (18, 4, [255; 4]),
        (18, 6, [0; 4]),
        (0, 0, [0; 4]),
        (8, 8, [0, 0, 0, 255]),
    ] {
        let offset = (y * 40 + x) * 4;
        assert_eq!(&rgba[offset..offset + 4], &expected);
    }
    request.as_object_mut().unwrap().remove("surroundingColor");
    let output = invoke_program(program, request.to_string().as_bytes(), &options);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("surroundingColor"));
}

#[test]
fn focus_cli_preserves_png_and_rejects_unfocused_intent() {
    let input = include_bytes!("../../../../conformance/ir/focus-ir-request.json");
    let program = env!("CARGO_BIN_EXE_resina-focus-raster");
    let options = ["32", "26", "-6", "-6", "1", "4"];
    let result = invoke_program(program, input, &options);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let encoded = result.stdout;
    let mut reader = png::Decoder::new(std::io::Cursor::new(&encoded))
        .read_info()
        .unwrap();
    assert_eq!(
        reader.info().srgb,
        Some(png::SrgbRenderingIntent::Perceptual)
    );
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!((frame.width, frame.height), (32, 26));
    assert_eq!(&rgba[(12 * 32 + 16) * 4..(12 * 32 + 17) * 4], &[0; 4]);
    assert_eq!(&rgba[(3 * 32 + 16) * 4..(3 * 32 + 17) * 4], &[255; 4]);
    let file = Command::new(program)
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../conformance/ir/focus-ir-request.json"
        ))
        .args(options)
        .output()
        .unwrap();
    assert!(file.status.success());
    assert_eq!(file.stdout, encoded);
    let mut request: serde_json::Value = serde_json::from_slice(input).unwrap();
    request["surface"]["states"]["states"] = serde_json::json!(["rest"]);
    let invalid = invoke_program(program, request.to_string().as_bytes(), &options);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("focused"));
}

#[test]
fn invalid_numeric_arguments_identify_the_parameter() {
    for (index, name) in [
        (0, "width"),
        (1, "height"),
        (2, "origin x"),
        (3, "origin y"),
        (4, "pixels per unit"),
        (5, "samples per axis"),
    ] {
        let mut options = OPTIONS.to_vec();
        options[index] = "invalid";
        let result = Command::new(env!("CARGO_BIN_EXE_resina-surface-raster"))
            .arg("-")
            .args(options)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains(name));
    }
}

#[test]
fn stdin_and_file_paths_produce_the_same_png() {
    let stdin = invoke(REQUEST, OPTIONS);
    assert!(
        stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&stdin.stderr)
    );
    assert!(stdin.stdout.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(stdin.stderr.is_empty());
    let file = Command::new(env!("CARGO_BIN_EXE_resina-surface-raster"))
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../conformance/ir/opaque-surface-request.json"
        ))
        .args(OPTIONS)
        .output()
        .unwrap();
    assert!(file.status.success());
    assert_eq!(file.stdout, stdin.stdout);
}

#[test]
fn malformed_input_and_invalid_render_options_emit_no_png() {
    for (input, options, diagnostic) in [
        (&b"{}"[..], OPTIONS, ""),
        (&b"\xff"[..], OPTIONS, ""),
        (REQUEST, &["24", "18", "-2", "-2", "NaN", "2"][..], "scale"),
        (REQUEST, &["24", "18", "-2", "-2", "1", "9"][..], "samples"),
        (
            REQUEST,
            &["4294967295", "4294967295", "0", "0", "1", "8"][..],
            "limit",
        ),
    ] {
        let result = invoke(input, options);
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains(diagnostic));
    }
    let oversized = invoke(&vec![b' '; 1_048_577], OPTIONS);
    assert!(!oversized.status.success());
    assert!(oversized.stdout.is_empty());
    assert!(String::from_utf8_lossy(&oversized.stderr).contains("request size limit exceeded"));
}

#[test]
fn command_cli_paints_pressed_body_and_independent_focus_without_partial_failure() {
    let mut request: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../conformance/ir/command-paint-request.json"
    ))
    .unwrap();
    request["surface"]["body"]["surface"]["states"]["states"] =
        serde_json::json!(["pressed", "focused"]);
    let program = env!("CARGO_BIN_EXE_resina-command-raster");
    let options = ["40", "32", "-8", "-8", "1", "4"];
    let output = invoke_program(program, request.to_string().as_bytes(), &options);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(output.stdout))
        .read_info()
        .unwrap();
    assert_eq!(
        reader.info().srgb,
        Some(png::SrgbRenderingIntent::Perceptual)
    );
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!((frame.width, frame.height), (40, 32));
    for (x, y, expected) in [
        (18, 14, [186, 162, 139, 255]),
        (18, 4, [255; 4]),
        (18, 6, [0; 4]),
        (0, 0, [0; 4]),
    ] {
        let offset = (y * 40 + x) * 4;
        assert_eq!(&rgba[offset..offset + 4], &expected);
    }
    request["surface"]
        .as_object_mut()
        .unwrap()
        .remove("surroundingColor");
    let failure = invoke_program(program, request.to_string().as_bytes(), &options);
    assert_eq!(failure.status.code(), Some(1));
    assert!(failure.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failure.stderr).contains("surroundingColor"));
}
