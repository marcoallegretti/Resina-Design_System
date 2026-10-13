use resina_resolver::*;
use serde_json::Value;

type Resolve = fn(&str) -> Result<(), String>;

fn boundaries() -> Vec<(&'static str, Value, &'static str, Resolve)> {
    macro_rules! boundary {
        ($name:literal, $file:literal, $path:literal, $resolve:ident) => {
            (
                $name,
                serde_json::from_str(include_str!(concat!("../../../../conformance/ir/", $file)))
                    .unwrap(),
                $path,
                (|source| {
                    $resolve(source)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }) as Resolve,
            )
        };
    }
    let pigment: Value = serde_json::from_str(include_str!(
        "../../../../conformance/materials/opaque-pigment-vectors.json"
    ))
    .unwrap();
    let relief: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/relief-paint-vectors.json"
    ))
    .unwrap();
    let mut boundaries = vec![
        (
            "pigment",
            pigment[0]["request"].clone(),
            "/profiles/profiles",
            (|s| {
                resolve_opaque_pigment_source(s)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }) as Resolve,
        ),
        (
            "relief",
            relief[0]["request"].clone(),
            "/pigment/profiles/profiles",
            (|s| {
                resolve_relief_paint_source(s)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }) as Resolve,
        ),
        boundary!(
            "opaque",
            "opaque-surface-request.json",
            "/appearance/pigmentProfiles/profiles",
            resolve_opaque_surface_source
        ),
        boundary!(
            "command",
            "command-paint-request.json",
            "/surface/body/appearance/pigmentProfiles/profiles",
            resolve_command_paint_source
        ),
        boundary!(
            "command motion",
            "command-motion-request.json",
            "/surface/body/appearance/pigmentProfiles/profiles",
            resolve_command_motion_source
        ),
        boundary!(
            "toggle",
            "toggle-part-paint-request.json",
            "/surface/body/appearance/pigmentProfiles/profiles",
            resolve_toggle_part_paint_source
        ),
        boundary!(
            "toggle motion",
            "toggle-part-motion-request.json",
            "/surface/body/appearance/pigmentProfiles/profiles",
            resolve_toggle_part_motion_source
        ),
        boundary!(
            "slider",
            "slider-part-paint-request.json",
            "/appearance/pigmentProfiles/profiles",
            resolve_slider_part_paint_source
        ),
    ];
    let surface = boundaries
        .iter()
        .find(|(name, ..)| *name == "command")
        .unwrap()
        .1["surface"]
        .clone();
    boundaries.push((
        "surface",
        surface,
        "/body/appearance/pigmentProfiles/profiles",
        (|s| {
            resolve_surface_paint_source(s)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }) as Resolve,
    ));
    let appearance = boundaries
        .iter()
        .find(|(name, ..)| *name == "opaque")
        .unwrap()
        .1["appearance"]
        .clone();
    boundaries.push((
        "appearance",
        appearance,
        "/pigmentProfiles/profiles",
        (|s| {
            parse_opaque_surface_appearance_source(s)
                .map(|_| ())
                .map_err(|e| e.to_string())
        }) as Resolve,
    ));
    boundaries
}

#[test]
fn every_pigment_source_boundary_rejects_negative_coefficients_before_rounding() {
    let mut accepted = Vec::new();
    for (name, request, path, resolve) in boundaries() {
        resolve(&request.to_string()).unwrap_or_else(|e| panic!("{name} baseline: {e}"));
        for family in ["cast", "frost", "elastomer", "gel"] {
            for coefficient in ["sideShade", "highlightLift"] {
                let mut changed = request.clone();
                *changed
                    .pointer_mut(&format!("{path}/{family}/{coefficient}"))
                    .unwrap() = Value::String("coefficient-source".into());
                let source = changed
                    .to_string()
                    .replace("\"coefficient-source\"", "-1e-400");
                if resolve(&source).is_ok() {
                    accepted.push(format!("{name}/{family}/{coefficient}"));
                }
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "negative coefficients accepted: {accepted:?}"
    );
}

#[test]
fn source_zero_and_positive_underflow_remain_valid_pigment_proportions() {
    for (name, request, path, resolve) in boundaries() {
        for literal in ["0e-400", "-0.000e-400", "1e-400"] {
            let mut changed = request.clone();
            *changed
                .pointer_mut(&format!("{path}/gel/sideShade"))
                .unwrap() = Value::String("coefficient-source".into());
            let source = changed
                .to_string()
                .replace("\"coefficient-source\"", literal);
            resolve(&source).unwrap_or_else(|e| panic!("{name}/{literal}: {e}"));
        }
    }
}

#[test]
fn pigment_source_validation_accepts_escaped_object_member_names() {
    for (name, request, _, resolve) in boundaries() {
        let source = request
            .to_string()
            .replace("\"cast\":", "\"\\u0063ast\":")
            .replace("\"sideShade\":", "\"\\u0073ideShade\":")
            .replace("\"appearance\":", "\"\\u0061ppearance\":");
        resolve(&source).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}
