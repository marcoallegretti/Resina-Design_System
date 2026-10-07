use resina_resolver::resolve_hit_region_source;
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn invalid_environment_shapes_fail_embedded_source_and_command_publication() {
    let base: Value = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/hit-region-request.json"
    ))
    .unwrap();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/environment/source-shape-vectors.json"
    ))
    .unwrap();
    assert_eq!(vectors.len(), 11);
    for vector in vectors {
        let mut request = base.clone();
        let path = format!("/environment{}", vector["path"].as_str().unwrap());
        *request.pointer_mut(&path).unwrap() = vector["value"].clone();
        let source = request.to_string();
        assert!(
            resolve_hit_region_source(&source).is_err(),
            "{}",
            vector["name"]
        );
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-hit-region"))
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
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{}", vector["name"]);
        assert!(output.stdout.is_empty(), "{}", vector["name"]);
        assert!(!output.stderr.is_empty(), "{}", vector["name"]);
    }
}
