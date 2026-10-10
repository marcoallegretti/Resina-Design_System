use cxx_qt_lib::QString;
use resina_model::{PhysicalBounds, PhysicalVector};
use resina_resolver::{HitRegionIr, resolve_hit_region_source};

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
    }
    extern "Rust" {
        type PointerTarget;
        fn prepare_target(
            source: &QString,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        ) -> Result<Box<PointerTarget>>;
        fn target_contains(target: &PointerTarget, x: f64, y: f64) -> Result<bool>;
        fn target_publication(target: &PointerTarget) -> QString;
    }
}

pub struct PointerTarget(HitRegionIr);

fn prepare_target(
    source: &QString,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<Box<PointerTarget>, String> {
    let source = source.to_string();
    if source.len() > 1024 * 1024 {
        return Err("Qt pointer target request exceeds 1 MiB".into());
    }
    let target = resolve_hit_region_source(&source).map_err(|error| error.to_string())?;
    // Validate native coverage with the same exact endpoint predicates, rather
    // than rounded toolkit right/bottom comparisons.
    let carrier = serde_json::json!({
        "schemaVersion":"0.1.0",
        "environment": serde_json::from_str::<serde_json::Value>(&source)
            .map_err(|error| error.to_string())?["environment"],
        "visualBounds":target.bounds(),
        "availableBounds":PhysicalBounds { x, y, width, height },
        "componentMinimum":target.minimum_size(), "occupiedRegions":[]
    });
    let checked =
        resolve_hit_region_source(&carrier.to_string()).map_err(|error| error.to_string())?;
    if checked != target {
        return Err("native carrier changed the reserved target".into());
    }
    Ok(Box::new(PointerTarget(target)))
}

fn target_contains(target: &PointerTarget, x: f64, y: f64) -> Result<bool, String> {
    target
        .0
        .contains(PhysicalVector { x, y })
        .map_err(|error| error.to_string())
}

fn target_publication(target: &PointerTarget) -> QString {
    QString::from(serde_json::to_string(&target.0).expect("validated hit region"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn binding_membership_preserves_public_exact_endpoint_vectors() {
        let environment: Value = serde_json::from_str(include_str!(
            "../../../../conformance/headless/valid-request.json"
        ))
        .unwrap();
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/interaction/hit-membership-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let bounds = PhysicalBounds {
                x: vector["bounds"]["x"].as_f64().unwrap(),
                y: vector["bounds"]["y"].as_f64().unwrap(),
                width: vector["bounds"]["width"].as_f64().unwrap(),
                height: vector["bounds"]["height"].as_f64().unwrap(),
            };
            let mut environment = environment["environment"].clone();
            environment["inputCapabilities"] = json!(["finePointer"]);
            let source = QString::from(
                json!({
                    "schemaVersion":"0.1.0", "environment":environment,
                    "visualBounds":bounds, "availableBounds":bounds,
                    "componentMinimum":{"width":24,"height":24}, "occupiedRegions":[]
                })
                .to_string(),
            );
            let target =
                prepare_target(&source, bounds.x, bounds.y, bounds.width, bounds.height).unwrap();
            for point in vector["points"].as_array().unwrap() {
                assert_eq!(
                    target_contains(
                        &target,
                        point["x"].as_f64().unwrap(),
                        point["y"].as_f64().unwrap()
                    )
                    .unwrap(),
                    point["expected"].as_bool().unwrap(),
                    "{}: {point}",
                    vector["name"]
                );
            }
            assert!(target_contains(&target, f64::NAN, 0.0).is_err());
            assert!(
                prepare_target(
                    &source,
                    bounds.x,
                    bounds.y,
                    bounds.width / 2.0,
                    bounds.height
                )
                .is_err()
            );
        }
    }
}
