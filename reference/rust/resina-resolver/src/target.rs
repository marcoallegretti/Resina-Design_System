use resina_environment::{EnvironmentSnapshot, InputCapability};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinimumHitTarget {
    minimum_width: u16,
    minimum_height: u16,
}

impl MinimumHitTarget {
    fn square(size: u16) -> Self {
        Self {
            minimum_width: size,
            minimum_height: size,
        }
    }

    pub fn minimum_width(self) -> u16 {
        self.minimum_width
    }

    pub fn minimum_height(self) -> u16 {
        self.minimum_height
    }
}

pub fn resolve_minimum_hit_target(environment: &EnvironmentSnapshot) -> MinimumHitTarget {
    let capabilities = environment.input_capabilities();
    if capabilities.contains(&InputCapability::CoarsePointer)
        || capabilities.contains(&InputCapability::DirectTouch)
    {
        MinimumHitTarget::square(48)
    } else {
        MinimumHitTarget::square(24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn minimum_target_conformance_vectors() {
        let base: Value = serde_json::from_str(include_str!(
            "../../../../conformance/environment/valid-mixed-input.json"
        ))
        .unwrap();
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/interaction/target-minimum-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let mut document = base.clone();
            for name in [
                "inputCapabilities",
                "densityPreference",
                "viewingProfile",
                "textScale",
                "qualityPolicy",
            ] {
                document[name] = vector[name].clone();
            }
            let environment: EnvironmentSnapshot = serde_json::from_value(document).unwrap();
            let minimum = resolve_minimum_hit_target(&environment);
            assert_eq!(
                serde_json::to_value(minimum).unwrap(),
                vector["expected"],
                "{}",
                vector["name"]
            );
        }
    }
}
