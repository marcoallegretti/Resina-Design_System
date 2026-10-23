use resina_resolver::{
    resolve_surface_deformation_source, resolve_surface_emergence_source,
    resolve_surface_outline_source,
};
use serde_json::{Value, json};

type Resolver = fn(&str) -> Result<Value, String>;

fn deformation(source: &str) -> Result<Value, String> {
    resolve_surface_deformation_source(source)
        .map(|ir| serde_json::to_value(ir).unwrap())
        .map_err(|error| error.to_string())
}

fn emergence(source: &str) -> Result<Value, String> {
    resolve_surface_emergence_source(source)
        .map(|ir| serde_json::to_value(ir).unwrap())
        .map_err(|error| error.to_string())
}

fn deformation_baseline() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/motion/surface-deformation-request.json"
    ))
    .unwrap()
}

fn emergence_baseline() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/motion/surface-emergence-request.json"
    ))
    .unwrap()
}

fn deformation_cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/motion/surface-deformation-cases.json"
    ))
    .unwrap()
}

fn emergence_cases() -> Vec<Value> {
    serde_json::from_str(include_str!(
        "../../../../conformance/motion/surface-emergence-cases.json"
    ))
    .unwrap()
}

fn set(document: &mut Value, pointer: &str, value: Value) {
    *document.pointer_mut(pointer).unwrap() = value;
}

fn with(base: &Value, changes: &[(&str, Value)]) -> Value {
    let mut request = base.clone();
    for (pointer, value) in changes {
        set(&mut request, pointer, value.clone());
    }
    request
}

fn case_request(base: &Value, case: &Value) -> Value {
    let mut request = base.clone();
    for change in case["requestChanges"].as_array().unwrap() {
        set(
            &mut request,
            change["path"].as_str().unwrap(),
            change["value"].clone(),
        );
    }
    request
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn same(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        _ => actual == expected,
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-12_f64.max(1e-12 * expected.abs())
}

fn check_public_cases(base: &Value, cases: &[Value], resolve: Resolver) {
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let outcome = resolve(&case_request(base, case).to_string());
        match case.get("errorContains") {
            Some(fragment) => {
                let error = outcome.expect_err(name);
                if case["requestSchemaValid"].as_bool().unwrap() {
                    assert!(
                        error
                            .to_lowercase()
                            .contains(&fragment.as_str().unwrap().to_lowercase()),
                        "{name}: {error}"
                    );
                }
            }
            None => {
                let result = outcome.unwrap_or_else(|error| panic!("{name}: {error}"));
                assert_eq!(result["policy"], case["expectedPolicy"], "{name}");
            }
        }
    }
}

#[test]
fn public_deformation_cases_resolve_or_fail_as_stated() {
    check_public_cases(&deformation_baseline(), &deformation_cases(), deformation);
}

#[test]
fn public_emergence_cases_resolve_or_fail_as_stated() {
    check_public_cases(&emergence_baseline(), &emergence_cases(), emergence);
}

fn sample_times() -> Vec<f64> {
    (0..=40).map(|step| f64::from(step) * 0.0125).collect()
}

#[test]
fn deformation_keeps_the_centre_inside_bounds_and_inside_the_reservation() {
    let base = deformation_baseline();
    let mut projected = 0;
    for damping in [24.0, 56.0, 80.0] {
        for time in sample_times() {
            let mut request = with(&base, &[("/time", json!(time))]);
            for channel in ["widthScale", "heightScale"] {
                set(
                    &mut request,
                    &format!("/channels/{channel}/dynamics/damping"),
                    json!(damping),
                );
            }
            let result = deformation(&request.to_string()).unwrap();
            let size = &result["outline"]["size"];
            let (width, height) = (number(&size["width"]), number(&size["height"]));
            assert!(close(number(&result["origin"]["x"]) + width / 2.0, 32.0));
            assert!(close(number(&result["origin"]["y"]) + height / 2.0, 20.0));
            assert!((64.0 * 0.94..=64.0 * 1.06 + 1e-9).contains(&width));
            assert!((40.0 * 0.94..=40.0 * 1.06 + 1e-9).contains(&height));
            let reservation = &result["reservation"];
            let (x, y) = (
                number(&result["origin"]["x"]),
                number(&result["origin"]["y"]),
            );
            for (dx, dy) in [(0.0, 0.0), (2.4, 3.1)] {
                assert!(x + dx >= number(&reservation["x"]) - 1e-9);
                assert!(y + dy >= number(&reservation["y"]) - 1e-9);
                assert!(
                    x + width + dx
                        <= number(&reservation["x"]) + number(&reservation["width"]) + 1e-9
                );
                assert!(
                    y + height + dy
                        <= number(&reservation["y"]) + number(&reservation["height"]) + 1e-9
                );
            }
            projected += result["projection"]
                .as_object()
                .unwrap()
                .values()
                .filter(|marker| *marker != "none")
                .count();
        }
    }
    assert!(projected > 0, "the sweep never reached a projection bound");
}

#[test]
fn deformation_retains_the_unprojected_state_for_retargeting() {
    let base = deformation_baseline();
    let request = with(
        &base,
        &[
            ("/time", json!(0.08)),
            (
                "/channels/widthScale",
                json!({
                    "dynamics": base["channels"]["widthScale"]["dynamics"],
                    "initial": {"position": 1, "velocity": 0},
                    "target": 1.06
                }),
            ),
            ("/channels/widthScale/dynamics/damping", json!(24)),
        ],
    );
    let first = deformation(&request.to_string()).unwrap();
    assert_eq!(first["projection"]["widthScale"], "upperBound");
    let state = first["widthScale"]["state"].clone();
    assert!(number(&state["position"]) > 1.06);
    assert!(number(&state["velocity"]).abs() > 0.0);

    let mut restart = request.clone();
    set(&mut restart, "/time", json!(0));
    set(&mut restart, "/channels/widthScale/initial", state.clone());
    let resumed = deformation(&restart.to_string()).unwrap();
    assert_eq!(resumed["widthScale"]["state"], state);
    assert_eq!(resumed["projection"]["widthScale"], "upperBound");

    let mut continued = request.clone();
    set(&mut continued, "/time", json!(0.12));
    let mut resumed_later = restart.clone();
    set(&mut resumed_later, "/time", json!(0.04));
    let direct = deformation(&continued.to_string()).unwrap();
    let after = deformation(&resumed_later.to_string()).unwrap();
    for member in ["position", "velocity"] {
        let (a, b) = (
            number(&direct["widthScale"]["state"][member]),
            number(&after["widthScale"]["state"][member]),
        );
        assert!((a - b).abs() <= 1e-9_f64.max(1e-9 * a.abs()), "{member}");
    }
}

#[test]
fn immediate_deformation_gives_the_exact_endpoint_without_velocity() {
    let base = deformation_baseline();
    for changes in [
        vec![("/reducedMotion", json!(true)), ("/time", json!(0))],
        vec![("/reducedMotion", json!(true)), ("/time", json!(0.01))],
    ] {
        let result = deformation(&with(&base, &changes).to_string()).unwrap();
        for (channel, target) in [("widthScale", 1.04), ("heightScale", 0.94)] {
            assert_eq!(result[channel]["state"]["position"], json!(target));
            assert_eq!(result[channel]["state"]["velocity"], json!(0.0));
            assert_eq!(result[channel]["settled"], json!(true));
            assert_eq!(result[channel]["representation"], "immediate");
        }
        assert_eq!(result["policy"], "reducedMotion");
        assert!(close(
            number(&result["outline"]["size"]["width"]),
            64.0 * 1.04
        ));
        assert!(close(
            number(&result["outline"]["size"]["height"]),
            40.0 * 0.94
        ));
    }
}

#[test]
fn each_deformed_outline_is_the_surface_outline_of_the_sampled_size() {
    let base = deformation_baseline();
    for time in sample_times() {
        let request = with(&base, &[("/time", json!(time))]);
        let result = deformation(&request.to_string()).unwrap();
        let mut outline = base["outline"].clone();
        set(&mut outline, "/size", result["outline"]["size"].clone());
        let direct = resolve_surface_outline_source(&outline.to_string()).unwrap();
        assert_eq!(result["outline"], serde_json::to_value(direct).unwrap());
    }
}

#[test]
fn deformation_has_no_position_channel_or_authored_content_scale() {
    let base = deformation_baseline();
    let mut channels = base["channels"].clone();
    channels["position"] = base["channels"]["widthScale"].clone();
    assert!(deformation(&with(&base, &[("/channels", channels)]).to_string()).is_err());
    let mut content = base["content"].clone();
    content["scale"] = json!(1.1);
    assert!(deformation(&with(&base, &[("/content", content)]).to_string()).is_err());
}

#[test]
fn deformation_domain_is_checked_at_every_corner_for_content() {
    let base = deformation_baseline();
    let fits = with(
        &base,
        &[(
            "/content",
            json!({"x": 7.8, "y": 10, "width": 36, "height": 20}),
        )],
    );
    assert!(deformation(&fits.to_string()).is_ok());
    let smallest_only = with(
        &base,
        &[(
            "/content",
            json!({"x": 7.7, "y": 10, "width": 36, "height": 20}),
        )],
    );
    let error = deformation(&smallest_only.to_string()).unwrap_err();
    assert!(error.contains("deformation domain"), "{error}");
}

fn end_outline(request: &Value, end: &str) -> Value {
    request[end]["outline"].clone()
}

#[test]
fn emergence_endpoints_equal_the_validated_outlines() {
    let base = emergence_baseline();
    let at_source = with(&base, &[("/time", json!(0))]);
    let result = emergence(&at_source.to_string()).unwrap();
    let source = serde_json::to_value(
        resolve_surface_outline_source(&end_outline(&base, "from").to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(result["outline"], source);
    assert!(same(&result["origin"], &base["from"]["origin"]));
    assert_eq!(result["fromContent"]["opacity"], json!(1.0));
    assert_eq!(result["toContent"], Value::Null);

    let settled = with(&base, &[("/time", json!(2))]);
    let result = emergence(&settled.to_string()).unwrap();
    let destination = serde_json::to_value(
        resolve_surface_outline_source(&end_outline(&base, "to").to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(result["outline"], destination);
    assert!(same(&result["origin"], &base["to"]["origin"]));
    assert_eq!(result["fromContent"], Value::Null);
    assert_eq!(result["toContent"]["opacity"], json!(1.0));
}

#[test]
fn emergence_content_is_swapped_never_scaled_and_never_stale() {
    let base = emergence_baseline();
    for emerged in [true, false] {
        for time in sample_times() {
            let request = with(
                &base,
                &[("/time", json!(time)), ("/emerged", json!(emerged))],
            );
            let result = emergence(&request.to_string()).unwrap();
            for (member, end) in [("fromContent", "from"), ("toContent", "to")] {
                let content = &result[member];
                if !content.is_null() {
                    assert!(same(&content["bounds"], &base[end]["content"]), "{member}");
                    let opacity = number(&content["opacity"]);
                    assert!(opacity > 0.0 && opacity <= 1.0);
                }
            }
            let shown = |member: &str| number(&result[member]["opacity"]);
            if !result["fromContent"].is_null() && !result["toContent"].is_null() {
                assert!(close(shown("fromContent") + shown("toContent"), 1.0));
            }
            assert!(
                !result["fromContent"].is_null() || !result["toContent"].is_null(),
                "some content is always published"
            );
        }
    }
}

#[test]
fn a_cancelled_emergence_leaves_no_destination_content() {
    let base = emergence_baseline();
    let mid_flight = with(&base, &[("/time", json!(0.02))]);
    let flight = emergence(&mid_flight.to_string()).unwrap();
    let mut retargeted = mid_flight.clone();
    set(&mut retargeted, "/emerged", json!(false));
    set(&mut retargeted, "/time", json!(0));
    set(
        &mut retargeted,
        "/channels/progress/initial",
        flight["progress"]["state"].clone(),
    );
    set(
        &mut retargeted,
        "/channels/opacity/initial",
        flight["opacity"]["state"].clone(),
    );
    let resumed = emergence(&retargeted.to_string()).unwrap();
    assert_eq!(resumed["progress"]["state"], flight["progress"]["state"]);
    assert_eq!(resumed["outline"], flight["outline"]);
    assert_ne!(resumed["progress"]["target"], flight["progress"]["target"]);

    set(&mut retargeted, "/time", json!(2));
    let settled = emergence(&retargeted.to_string()).unwrap();
    assert_eq!(settled["toContent"], Value::Null);
    assert_eq!(settled["fromContent"]["opacity"], json!(1.0));
    assert!(same(
        &settled["outline"]["size"],
        &base["from"]["outline"]["size"]
    ));
}

#[test]
fn emergence_progress_is_projected_but_its_state_is_retained() {
    let base = emergence_baseline();
    let request = with(
        &base,
        &[
            ("/time", json!(0.08)),
            ("/channels/progress/dynamics/damping", json!(24)),
        ],
    );
    let result = emergence(&request.to_string()).unwrap();
    assert_eq!(result["projection"]["progress"], "upperBound");
    assert!(number(&result["progress"]["state"]["position"]) > 1.0);
    assert_eq!(result["outline"]["size"]["width"], json!(96.0));
    assert!(same(&result["origin"], &base["to"]["origin"]));
}

#[test]
fn an_initial_opacity_below_zero_hides_the_destination_until_it_crosses_zero() {
    let base = emergence_baseline();
    let delayed = |time: f64| {
        let request = with(
            &base,
            &[
                ("/time", json!(time)),
                (
                    "/channels/opacity/initial",
                    json!({"position": -1, "velocity": 0}),
                ),
            ],
        );
        emergence(&request.to_string()).unwrap()
    };
    let start = delayed(0.0);
    assert_eq!(start["projection"]["opacity"], "lowerBound");
    assert_eq!(start["opacity"]["state"]["position"], json!(-1.0));
    assert_eq!(start["toContent"], Value::Null);
    let later = delayed(0.1);
    assert_eq!(later["projection"]["opacity"], "none");
    assert!(!later["toContent"].is_null());
}

#[test]
fn emergence_opacity_does_not_overshoot_from_rest() {
    let base = emergence_baseline();
    for time in sample_times() {
        let request = with(&base, &[("/time", json!(time))]);
        let result = emergence(&request.to_string()).unwrap();
        let position = number(&result["opacity"]["state"]["position"]);
        assert!((0.0..=1.0).contains(&position), "{time}: {position}");
        assert_eq!(result["projection"]["opacity"], "none");
    }
}

#[test]
fn emergence_reservation_covers_every_sampled_outline() {
    let base = emergence_baseline();
    for time in sample_times() {
        let request = with(&base, &[("/time", json!(time))]);
        let result = emergence(&request.to_string()).unwrap();
        let (x, y) = (
            number(&result["origin"]["x"]),
            number(&result["origin"]["y"]),
        );
        let size = &result["outline"]["size"];
        let (width, height) = (number(&size["width"]), number(&size["height"]));
        let reservation = &result["reservation"];
        assert!(x >= number(&reservation["x"]) - 1e-9);
        assert!(y >= number(&reservation["y"]) - 1e-9);
        assert!(
            x + width + 2.4 <= number(&reservation["x"]) + number(&reservation["width"]) + 1e-9
        );
        assert!(
            y + height + 3.1 <= number(&reservation["y"]) + number(&reservation["height"]) + 1e-9
        );
    }
}

#[test]
fn emergence_rejects_an_end_whose_own_content_does_not_fit() {
    let base = emergence_baseline();
    let request = with(
        &base,
        &[(
            "/to/content",
            json!({"x": -40, "y": 10, "width": 48, "height": 20}),
        )],
    );
    let error = emergence(&request.to_string()).unwrap_err();
    assert!(error.contains("its own outline"), "{error}");
}
