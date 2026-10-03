use resina_model::SpringParameters;
use resina_motion::{resolve_spring_source, sample_spring};
use serde_json::Value;

fn parameters(m: f64, k: f64, c: f64, v: f64) -> SpringParameters {
    SpringParameters::try_new(m, k, c, v, 1e-14, 1e-14).unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn changes(document: &mut Value, changes: &Value) {
    for change in changes.as_array().unwrap() {
        *document
            .pointer_mut(change["path"].as_str().unwrap())
            .unwrap() = change["value"].clone();
    }
}

#[test]
fn public_equation_vectors() {
    let baseline: Value = serde_json::from_str(include_str!(
        "../../../../conformance/motion/spring-request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../conformance/motion/spring-expected.json"
    ))
    .unwrap();
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/motion/spring-cases.json"
    ))
    .unwrap();
    for case in cases {
        let mut request = baseline.clone();
        changes(&mut request, &case["requestChanges"]);
        let result = resolve_spring_source(&request.to_string());
        if let Some(error) = case["errorContains"].as_str() {
            assert!(
                result.unwrap_err().to_string().contains(error),
                "{}",
                case["name"]
            );
        } else {
            let mut expected = case
                .get("expected")
                .cloned()
                .unwrap_or_else(|| expected.clone());
            if let Some(delta) = case.get("expectedChanges") {
                changes(&mut expected, delta);
            }
            let result = serde_json::to_value(result.unwrap()).unwrap();
            near(
                result["position"].as_f64().unwrap(),
                expected["position"].as_f64().unwrap(),
            );
            near(
                result["velocity"].as_f64().unwrap(),
                expected["velocity"].as_f64().unwrap(),
            );
            for field in ["schemaVersion", "representation", "settled"] {
                assert_eq!(result[field], expected[field], "{}: {field}", case["name"]);
            }
        }
    }
}

#[test]
fn agrees_with_independent_runge_kutta_integration() {
    for damping in [0.0, 0.4, 4.0, 8.0] {
        for initial_velocity in [-2.0, 0.0, 3.0] {
            let spring = parameters(1.0, 4.0, damping, initial_velocity);
            let (mut x, mut v) = (0.0, initial_velocity);
            let h = 0.0005;
            let derivative = |x: f64, v: f64| (v, -damping * v - 4.0 * (x - 1.0));
            for step in 1..=4000 {
                let a = derivative(x, v);
                let b = derivative(x + h * a.0 / 2.0, v + h * a.1 / 2.0);
                let c = derivative(x + h * b.0 / 2.0, v + h * b.1 / 2.0);
                let d = derivative(x + h * c.0, v + h * c.1);
                x += h * (a.0 + 2.0 * b.0 + 2.0 * c.0 + d.0) / 6.0;
                v += h * (a.1 + 2.0 * b.1 + 2.0 * c.1 + d.1) / 6.0;
                if step % 100 == 0 {
                    let sample = sample_spring(&spring, step as f64 * h, false).unwrap();
                    near(sample.position(), x);
                    near(sample.velocity(), v);
                }
            }
        }
    }
}

#[test]
fn common_coefficient_scale_and_critical_limit_preserve_response() {
    for damping in [0.0, 1.0, 4.0, 9.0] {
        let expected = sample_spring(&parameters(1.0, 4.0, damping, 2.0), 0.7, false).unwrap();
        for scale in [1e-200, 1e-100, 1e100, 1e200] {
            let actual = sample_spring(
                &parameters(scale, 4.0 * scale, damping * scale, 2.0),
                0.7,
                false,
            )
            .unwrap();
            near(actual.position(), expected.position());
            near(actual.velocity(), expected.velocity());
        }
    }
    for damping in [2.0 - 1e-10, 2.0, 2.0 + 1e-10] {
        let actual = sample_spring(&parameters(1.0, 1.0, damping, 0.0), 1.0, false).unwrap();
        near(actual.position(), 1.0 - 2.0 / std::f64::consts::E);
        near(actual.velocity(), 1.0 / std::f64::consts::E);
    }
}

#[test]
fn energy_decreases_and_settled_samples_stay_settled() {
    for damping in [0.2, 2.0, 10.0] {
        let spring = SpringParameters::try_new(1.0, 1.0, damping, -2.0, 0.001, 0.001).unwrap();
        let mut previous_energy = f64::INFINITY;
        let mut was_settled = false;
        for step in 0..=4000 {
            let sample = sample_spring(&spring, step as f64 * 0.1, false).unwrap();
            let energy = (sample.position() - 1.0).powi(2) + sample.velocity().powi(2);
            assert!(energy <= previous_energy + 1e-12);
            assert!(!was_settled || sample.settled());
            previous_energy = energy;
            was_settled = sample.settled();
        }
        assert!(was_settled);
    }
}

#[test]
fn large_coefficients_survive_exponential_underflow() {
    let spring = SpringParameters::try_new(1.0, 1.0, 2.0, f64::MAX, 1e-130, 1e-130).unwrap();
    let sample = sample_spring(&spring, 1000.0, false).unwrap();
    assert!(!sample.settled());
    let expected_velocity = -((f64::MAX.ln() + 999.0_f64.ln()) - 1000.0).exp();
    assert!((sample.velocity() / expected_velocity - 1.0).abs() < 1e-10);
    let spring = parameters(1.0, 1.0, f64::MAX, 0.0);
    let sample = sample_spring(&spring, f64::MAX, false).unwrap();
    near(sample.position(), 1.0 - 1.0 / std::f64::consts::E);
    assert!(sample.velocity() > 0.0);
}

#[test]
fn reduced_motion_is_immediate_but_time_must_be_valid() {
    let spring = parameters(f64::from_bits(1), f64::MAX, 0.0, 0.0);
    for time in [0.0, 1.0, f64::MAX] {
        let sample = sample_spring(&spring, time, true).unwrap();
        assert_eq!(
            (sample.position(), sample.velocity(), sample.settled()),
            (1.0, 0.0, true)
        );
    }
    for time in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(sample_spring(&spring, time, true).is_err());
    }
}

#[test]
fn physical_velocity_bound_survives_normalized_underflow() {
    let spring = SpringParameters::try_new(1e-300, 1e300, 2.0, 0.0, 1e-50, 1e-50).unwrap();
    let sample = sample_spring(&spring, 8e-298, false).unwrap();
    assert!(!sample.settled());
    let expected = (1e300_f64.ln() + 800.0_f64.ln() - 800.0).exp();
    assert!((sample.velocity() / expected - 1.0).abs() < 1e-10);
}

#[test]
fn strict_source_rejects_duplicates_unknown_and_missing_fields() {
    let baseline = include_str!("../../../../conformance/motion/spring-request.json");
    for (from, to, diagnostic) in [
        (
            "\"schemaVersion\": \"0.1.0\"",
            "\"schemaVersion\": \"0.1.0\", \"schemaVersion\": \"0.1.0\"",
            "duplicate",
        ),
        ("\"mass\": 1", "\"mass\": 1, \"mass\": 1", "duplicate"),
        ("\"time\": 1", "\"unknown\": 1", "unknown field"),
        ("\"reducedMotion\": false", "\"time\": 1", "duplicate"),
    ] {
        assert!(baseline.contains(from));
        let error = resolve_spring_source(&baseline.replacen(from, to, 1)).unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{error}");
    }
    let mut request: Value = serde_json::from_str(baseline).unwrap();
    request.as_object_mut().unwrap().remove("reducedMotion");
    assert!(
        resolve_spring_source(&request.to_string())
            .unwrap_err()
            .to_string()
            .contains("missing field")
    );
}
