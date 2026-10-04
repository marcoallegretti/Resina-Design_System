use resina_model::{SpringDynamics, SpringParameters, SpringState};
use resina_motion::{sample_spring, sample_spring_trajectory};

fn dynamics(damping: f64) -> SpringDynamics {
    SpringDynamics::try_new(1.0, 4.0, damping, 1e-14, 1e-14).unwrap()
}
fn state(position: f64, velocity: f64) -> SpringState {
    SpringState::try_new(position, velocity).unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn normalized_contract_uses_the_same_trajectory_without_result_changes() {
    for damping in [0.0, 0.4, 4.0, 8.0] {
        for velocity in [-2.0, 0.0, 3.0] {
            let spring =
                SpringParameters::try_new(1.0, 4.0, damping, velocity, 1e-14, 1e-14).unwrap();
            for time in [0.0, 0.1, 1.0, 10.0, 100.0] {
                for reduced in [false, true] {
                    let old = sample_spring(&spring, time, reduced).unwrap();
                    let new = sample_spring_trajectory(
                        &spring.dynamics(),
                        state(0.0, velocity),
                        1.0,
                        time,
                        reduced,
                    )
                    .unwrap();
                    assert_eq!(new.state(), state(old.position(), old.velocity()));
                    assert_eq!(new.representation(), old.representation());
                    assert_eq!(new.settled(), old.settled());
                }
            }
        }
    }
}

#[test]
fn arbitrary_initial_conditions_agree_with_independent_runge_kutta() {
    for damping in [0.0, 0.4, 4.0, 8.0] {
        for (position, velocity, target) in [(3.0, -2.0, -1.0), (-4.0, 3.0, 2.0), (2.0, 5.0, 2.0)] {
            let initial = state(position, velocity);
            let (mut x, mut v) = (position, velocity);
            let h = 0.0005;
            let derivative = |x: f64, v: f64| (v, -damping * v - 4.0 * (x - target));
            for step in 1..=4000 {
                let a = derivative(x, v);
                let b = derivative(x + h * a.0 / 2.0, v + h * a.1 / 2.0);
                let c = derivative(x + h * b.0 / 2.0, v + h * b.1 / 2.0);
                let d = derivative(x + h * c.0, v + h * c.1);
                x += h * (a.0 + 2.0 * b.0 + 2.0 * c.0 + d.0) / 6.0;
                v += h * (a.1 + 2.0 * b.1 + 2.0 * c.1 + d.1) / 6.0;
                if step % 100 == 0 {
                    let result = sample_spring_trajectory(
                        &dynamics(damping),
                        initial,
                        target,
                        step as f64 * h,
                        false,
                    )
                    .unwrap();
                    near(result.state().position(), x);
                    near(result.state().velocity(), v);
                }
            }
        }
    }
}

#[test]
fn retargeting_preserves_position_and_velocity_and_restarting_is_frame_independent() {
    for damping in [0.0, 0.4, 4.0, 8.0] {
        let dynamics = dynamics(damping);
        let initial = state(-2.0, 3.0);
        let moving = sample_spring_trajectory(&dynamics, initial, 4.0, 0.37, false).unwrap();
        for target in [-5.0, 0.0, 7.0] {
            let changed =
                sample_spring_trajectory(&dynamics, moving.state(), target, 0.0, false).unwrap();
            assert_eq!(changed.state(), moving.state());
            assert!(!changed.settled());
            let future =
                sample_spring_trajectory(&dynamics, moving.state(), target, 0.25, false).unwrap();
            assert_ne!(future.state(), changed.state());
        }
        let later = sample_spring_trajectory(&dynamics, moving.state(), 4.0, 0.21, false).unwrap();
        let direct = sample_spring_trajectory(&dynamics, initial, 4.0, 0.58, false).unwrap();
        near(later.state().position(), direct.state().position());
        near(later.state().velocity(), direct.state().velocity());
    }
}

#[test]
fn reduced_motion_and_numeric_range_failures_are_explicit() {
    let extreme = SpringDynamics::try_new(f64::from_bits(1), f64::MAX, 0.0, 1e-6, 1e-6).unwrap();
    for initial in [state(f64::MAX, -f64::MAX), state(0.0, 0.0)] {
        let immediate = sample_spring_trajectory(&extreme, initial, -f64::MAX, 0.0, true).unwrap();
        assert_eq!(immediate.state(), state(-f64::MAX, 0.0));
        assert!(immediate.settled());
    }
    for target in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            sample_spring_trajectory(&dynamics(4.0), state(0.0, 0.0), target, 0.0, true).is_err()
        );
    }
    for time in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            sample_spring_trajectory(&dynamics(4.0), state(0.0, 0.0), 1.0, time, true).is_err()
        );
    }
    assert!(
        sample_spring_trajectory(&dynamics(4.0), state(f64::MAX, 0.0), -f64::MAX, 1.0, false)
            .unwrap_err()
            .to_string()
            .contains("initial displacement")
    );
    let high = SpringDynamics::try_new(1.0, 100.0, 20.0, 1e-140, 1e-140).unwrap();
    let result = sample_spring_trajectory(&high, state(f64::MAX, 0.0), 0.0, 100.0, false).unwrap();
    assert!(!result.settled());
    let expected = -((f64::MAX.ln() + 10000.0_f64.ln()) - 1000.0).exp();
    assert!((result.state().velocity() / expected - 1.0).abs() < 1e-10);
}

#[test]
fn same_target_with_velocity_is_not_an_immediate_endpoint() {
    let result = sample_spring_trajectory(
        &dynamics(0.0),
        state(2.0, 4.0),
        2.0,
        std::f64::consts::FRAC_PI_4,
        false,
    )
    .unwrap();
    near(result.state().position(), 4.0);
    near(result.state().velocity(), 0.0);
    assert!(!result.settled());
}

#[test]
fn energy_bounds_settle_once_and_remain_settled() {
    for damping in [0.2, 4.0, 8.0] {
        let dynamics = SpringDynamics::try_new(1.0, 4.0, damping, 0.001, 0.001).unwrap();
        let mut settled = false;
        let mut energy = f64::INFINITY;
        for step in 0..4000 {
            let result = sample_spring_trajectory(
                &dynamics,
                state(-4.0, 3.0),
                -2.0,
                step as f64 * 0.1,
                false,
            )
            .unwrap();
            let current = (result.state().position() + 2.0).powi(2)
                + (result.state().velocity() / 2.0).powi(2);
            assert!(current <= energy + 1e-12);
            energy = current;
            assert!(!settled || result.settled());
            settled = result.settled();
            if settled {
                assert_eq!(result.state(), state(-2.0, 0.0));
            }
        }
        assert!(settled);
    }
}

#[test]
fn public_trajectory_cases_and_diagnostics() {
    use resina_motion::resolve_spring_trajectory_source;
    use serde_json::Value;
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/motion/spring-trajectory-cases.json"
    ))
    .unwrap();
    for case in cases {
        let actual = resolve_spring_trajectory_source(&case["request"].to_string());
        if let Some(expected) = case.get("expected") {
            let actual = actual.unwrap();
            near(
                actual.state().position(),
                expected["state"]["position"].as_f64().unwrap(),
            );
            near(
                actual.state().velocity(),
                expected["state"]["velocity"].as_f64().unwrap(),
            );
            assert_eq!(actual.target(), expected["target"].as_f64().unwrap());
            assert_eq!(actual.settled(), expected["settled"].as_bool().unwrap());
            assert_eq!(
                serde_json::to_value(actual.representation()).unwrap(),
                expected["representation"]
            );
        } else {
            let error = actual.unwrap_err().to_string();
            assert!(
                error.contains(case["errorContains"].as_str().unwrap()),
                "{}: {error}",
                case["name"]
            );
        }
    }
}
#[test]
fn strict_trajectory_cli_has_atomic_utf8_and_bounded_protocol() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let program = env!("CARGO_BIN_EXE_resina-spring-trajectory");
    let source = include_bytes!("../../../../conformance/motion/spring-trajectory-request.json");
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../conformance/motion/spring-trajectory-request.json"
    );
    let file = Command::new(program).arg(path).output().unwrap();
    assert!(file.status.success());
    for bytes in [
        source.to_vec(),
        vec![0xff],
        vec![b' '; 1024 * 1024 + 1],
        b"{".to_vec(),
    ] {
        let mut child = Command::new(program)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        let result = child.wait_with_output().unwrap();
        if bytes == source {
            assert_eq!(result.stdout, file.stdout);
            assert!(result.stderr.is_empty());
            assert!(result.status.success());
        } else {
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(!result.stderr.is_empty());
        }
    }
    assert_eq!(
        Command::new(program).output().unwrap().status.code(),
        Some(2)
    );
    assert_eq!(
        Command::new(program)
            .args(["-", "extra"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert!(
        Command::new(program)
            .arg("--help")
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn changing_coordinate_units_preserves_trajectory_and_settling() {
    for damping in [0.0, 0.4, 4.0, 8.0] {
        let base = SpringDynamics::try_new(1.0, 4.0, damping, 0.001, 0.002).unwrap();
        for scale in [0.001, 1000.0] {
            let scaled =
                SpringDynamics::try_new(1.0, 4.0, damping, 0.001 * scale, 0.002 * scale).unwrap();
            for time in [0.0, 0.37, 2.0, 100.0] {
                let a =
                    sample_spring_trajectory(&base, state(-2.0, 3.0), 4.0, time, false).unwrap();
                let b = sample_spring_trajectory(
                    &scaled,
                    state(-2.0 * scale, 3.0 * scale),
                    4.0 * scale,
                    time,
                    false,
                )
                .unwrap();
                near(b.state().position() / scale, a.state().position());
                near(b.state().velocity() / scale, a.state().velocity());
                assert_eq!(a.settled(), b.settled());
            }
        }
    }
}
