use resina_model::{SpringDynamics, SpringState};
use resina_motion::sample_spring_trajectory;
use std::path::Path;

fn dynamics(name: &str) -> SpringDynamics {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../definitions/motion")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn state(position: f64, velocity: f64) -> SpringState {
    SpringState::try_new(position, velocity).unwrap()
}

#[test]
fn authored_responses_meet_step_and_interruption_bounds() {
    for (name, ninety_limit, settle_limit, peak_lower, peak_upper) in [
        ("frost-body", 0.100, 0.310, 1.0, 1.0),
        ("frost-depth", 0.100, 0.250, 1.0, 1.0),
        ("elastomer-body", 0.080, 0.320, 1.010, 1.020),
        ("elastomer-depth", 0.075, 0.280, 1.040, 1.050),
        ("toggle-travel", 0.100, 0.250, 1.0, 1.0),
    ] {
        let dynamics = dynamics(name);
        let initial = state(0.0, 0.0);
        let mut ninety = None;
        let mut settled = None;
        let mut peak = 0.0_f64;
        let mut previous = 0.0;
        for index in 0..=10_000 {
            let time = f64::from(index) / 10_000.0;
            let sample = sample_spring_trajectory(&dynamics, initial, 1.0, time, false).unwrap();
            let position = sample.state().position();
            if peak_upper == 1.0 {
                assert!(position >= previous, "{name}: nonmonotone step at {time}");
            }
            previous = position;
            peak = peak.max(position);
            if position >= 0.9 && ninety.is_none() {
                ninety = Some(time);
            }
            if sample.settled() {
                assert_eq!(sample.state(), state(1.0, 0.0));
                settled.get_or_insert(time);
            }
        }
        assert!(ninety.unwrap() <= ninety_limit, "{name}: slow response");
        assert!(settled.unwrap() <= settle_limit, "{name}: slow settling");
        assert!(
            (peak_lower..=peak_upper).contains(&peak),
            "{name}: peak {peak}"
        );
        for elapsed in [0.016, 0.033, 0.05, 0.075, 0.1, 0.15] {
            let retained = sample_spring_trajectory(&dynamics, initial, 1.0, elapsed, false)
                .unwrap()
                .state();
            let restarted = sample_spring_trajectory(&dynamics, retained, 0.0, 0.0, false).unwrap();
            assert_eq!(restarted.state(), retained, "{name}: retarget lost state");
            let terminal =
                sample_spring_trajectory(&dynamics, retained, 0.0, settle_limit, false).unwrap();
            assert!(
                terminal.settled(),
                "{name}: reversal at {elapsed} did not settle"
            );
            assert_eq!(terminal.state(), state(0.0, 0.0));
            let reduced = sample_spring_trajectory(&dynamics, retained, 0.0, 0.0, true).unwrap();
            assert!(reduced.settled());
            assert_eq!(reduced.state(), state(0.0, 0.0));
        }
    }
}
