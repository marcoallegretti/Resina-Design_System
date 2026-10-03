use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn invoke(input: &[u8], options: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_resina-surface-raster"))
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

const REQUEST: &[u8] = include_bytes!("../../../ir/opaque-surface-request.json");
const OPTIONS: &[&str] = &["24", "18", "-2", "-2", "1", "2"];

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
            "/../../ir/opaque-surface-request.json"
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
