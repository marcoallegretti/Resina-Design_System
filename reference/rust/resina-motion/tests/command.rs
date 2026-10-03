use std::io::Write;
use std::process::{Command, Stdio};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_resina-spring"))
}

#[test]
fn file_and_stdin_protocol_match_and_fail_without_results() {
    let source = include_str!("../../../../conformance/motion/spring-request.json");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../conformance/motion/spring-request.json"
    );
    let file = command().arg(path).output().unwrap();
    assert!(file.status.success());
    for (request, valid) in [
        (source.to_owned(), true),
        ("{".to_owned(), false),
        (" ".repeat(1024 * 1024 + 1), false),
    ] {
        let mut child = command()
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        if valid {
            assert!(output.status.success());
            assert_eq!(output.stdout, file.stdout);
            assert!(output.stderr.is_empty());
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
        }
    }
    assert_eq!(command().output().unwrap().status.code(), Some(2));
    assert_eq!(
        command()
            .args(["-", "extra"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert!(command().arg("--help").output().unwrap().status.success());
    let missing = command()
        .arg("nonexistent-spring-request.json")
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
}
