use resina_motion::{resolve_spring_source, resolve_spring_trajectory_source};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn resolve(trajectory: bool, source: &str) -> Result<Value, String> {
    if trajectory {
        resolve_spring_trajectory_source(source).map(|result| serde_json::to_value(result).unwrap())
    } else {
        resolve_spring_source(source).map(|result| serde_json::to_value(result).unwrap())
    }
    .map_err(|error| error.to_string())
}

fn run(trajectory: bool, source: &str) -> std::process::Output {
    let binary = if trajectory {
        env!("CARGO_BIN_EXE_resina-spring-trajectory")
    } else {
        env!("CARGO_BIN_EXE_resina-spring")
    };
    let mut child = Command::new(binary)
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
    child.wait_with_output().unwrap()
}

#[test]
fn spring_sources_reject_positional_records_before_either_motion_policy() {
    for trajectory in [false, true] {
        let source = if trajectory {
            include_str!("../../../../conformance/motion/spring-trajectory-request.json")
        } else {
            include_str!("../../../../conformance/motion/spring-request.json")
        };
        let baseline: Value = serde_json::from_str(source).unwrap();
        let fields: &[(&str, &[&str])] = if trajectory {
            &[
                (
                    "",
                    &[
                        "schemaVersion",
                        "dynamics",
                        "initial",
                        "target",
                        "time",
                        "reducedMotion",
                    ],
                ),
                (
                    "/dynamics",
                    &[
                        "schemaVersion",
                        "mass",
                        "stiffness",
                        "damping",
                        "positionThreshold",
                        "velocityThreshold",
                    ],
                ),
                ("/initial", &["position", "velocity"]),
            ]
        } else {
            &[
                ("", &["schemaVersion", "spring", "time", "reducedMotion"]),
                (
                    "/spring",
                    &[
                        "schemaVersion",
                        "mass",
                        "stiffness",
                        "damping",
                        "initialVelocity",
                        "positionThreshold",
                        "velocityThreshold",
                    ],
                ),
            ]
        };
        for reduced in [false, true] {
            let mut valid = baseline.clone();
            valid["reducedMotion"] = json!(reduced);
            let canonical = resolve(trajectory, &valid.to_string()).unwrap();
            let output = run(trajectory, &valid.to_string());
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                canonical
            );
            for (pointer, members) in fields {
                let mut invalid = valid.clone();
                let object = invalid.pointer(pointer).unwrap();
                let array = Value::Array(
                    members
                        .iter()
                        .map(|member| object[*member].clone())
                        .collect(),
                );
                *invalid.pointer_mut(pointer).unwrap() = array;
                let source = invalid.to_string();
                assert!(
                    resolve(trajectory, &source).is_err(),
                    "{pointer} reduced={reduced}"
                );
                let output = run(trajectory, &source);
                assert_eq!(output.status.code(), Some(1), "{pointer} reduced={reduced}");
                assert!(output.stdout.is_empty());
                assert!(!output.stderr.is_empty());
            }
        }
    }
}
