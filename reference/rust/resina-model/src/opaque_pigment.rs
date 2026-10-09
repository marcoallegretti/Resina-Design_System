use crate::MaterialFamily;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "OpaquePigmentProfileInput")]
pub struct OpaquePigmentProfile {
    side_shade: f64,
    highlight_lift: f64,
}

struct OpaquePigmentProfileInput {
    side_shade: f64,
    highlight_lift: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpaquePigmentProfileMembers {
    side_shade: f64,
    highlight_lift: f64,
}

impl<'de> Deserialize<'de> for OpaquePigmentProfileInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ProfileVisitor;
        impl<'de> Visitor<'de> for ProfileVisitor {
            type Value = OpaquePigmentProfileInput;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an opaque pigment profile object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input =
                    OpaquePigmentProfileMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(OpaquePigmentProfileInput {
                    side_shade: input.side_shade,
                    highlight_lift: input.highlight_lift,
                })
            }
        }
        deserializer.deserialize_map(ProfileVisitor)
    }
}

impl TryFrom<OpaquePigmentProfileInput> for OpaquePigmentProfile {
    type Error = &'static str;

    fn try_from(input: OpaquePigmentProfileInput) -> Result<Self, Self::Error> {
        if !input.side_shade.is_finite() || !(0.0..1.0).contains(&input.side_shade) {
            return Err("sideShade must be finite and in [0, 1)");
        }
        if !input.highlight_lift.is_finite() || !(0.0..1.0).contains(&input.highlight_lift) {
            return Err("highlightLift must be finite and in [0, 1)");
        }
        Ok(Self {
            side_shade: input.side_shade,
            highlight_lift: input.highlight_lift,
        })
    }
}

impl OpaquePigmentProfile {
    pub fn side_shade(&self) -> f64 {
        self.side_shade
    }

    pub fn highlight_lift(&self) -> f64 {
        self.highlight_lift
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaquePigmentProfiles {
    schema_version: String,
    profiles: FamilyProfiles,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpaquePigmentProfilesMembers {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    profiles: FamilyProfiles,
}

impl<'de> Deserialize<'de> for OpaquePigmentProfiles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ProfilesVisitor;
        impl<'de> Visitor<'de> for ProfilesVisitor {
            type Value = OpaquePigmentProfiles;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an opaque pigment profiles object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input =
                    OpaquePigmentProfilesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(OpaquePigmentProfiles {
                    schema_version: input.schema_version,
                    profiles: input.profiles,
                })
            }
        }
        deserializer.deserialize_map(ProfilesVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct FamilyProfiles {
    cast: OpaquePigmentProfile,
    frost: OpaquePigmentProfile,
    elastomer: OpaquePigmentProfile,
    gel: OpaquePigmentProfile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FamilyProfilesMembers {
    cast: OpaquePigmentProfile,
    frost: OpaquePigmentProfile,
    elastomer: OpaquePigmentProfile,
    gel: OpaquePigmentProfile,
}

impl<'de> Deserialize<'de> for FamilyProfiles {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FamiliesVisitor;
        impl<'de> Visitor<'de> for FamiliesVisitor {
            type Value = FamilyProfiles;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a material family pigment object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = FamilyProfilesMembers::deserialize(MapAccessDeserializer::new(map))?;
                Ok(FamilyProfiles {
                    cast: input.cast,
                    frost: input.frost,
                    elastomer: input.elastomer,
                    gel: input.gel,
                })
            }
        }
        deserializer.deserialize_map(FamiliesVisitor)
    }
}

impl OpaquePigmentProfiles {
    pub fn profile_for(&self, family: MaterialFamily) -> &OpaquePigmentProfile {
        match family {
            MaterialFamily::Cast => &self.profiles.cast,
            MaterialFamily::Frost => &self.profiles.frost,
            MaterialFamily::Elastomer => &self.profiles.elastomer,
            MaterialFamily::Gel => &self.profiles.gel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn pigment_profile_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/opaque-pigment-profile-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                serde_json::from_value::<OpaquePigmentProfiles>(vector["document"].clone());
            if vector["valid"] == true {
                let profiles = result.unwrap();
                for (name, family) in [
                    ("cast", MaterialFamily::Cast),
                    ("frost", MaterialFamily::Frost),
                    ("elastomer", MaterialFamily::Elastomer),
                    ("gel", MaterialFamily::Gel),
                ] {
                    let expected = &vector["document"]["profiles"][name];
                    assert_eq!(
                        profiles.profile_for(family).side_shade(),
                        expected["sideShade"].as_f64().unwrap()
                    );
                    assert_eq!(
                        profiles.profile_for(family).highlight_lift(),
                        expected["highlightLift"].as_f64().unwrap()
                    );
                }
            } else {
                assert!(result.is_err(), "{}", vector["name"]);
            }
        }
    }

    #[test]
    fn typed_profile_validation_rejects_nonfinite_parameters() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                OpaquePigmentProfile::try_from(OpaquePigmentProfileInput {
                    side_shade: value,
                    highlight_lift: 0.1,
                })
                .is_err()
            );
            assert!(
                OpaquePigmentProfile::try_from(OpaquePigmentProfileInput {
                    side_shade: 0.1,
                    highlight_lift: value,
                })
                .is_err()
            );
        }
        let profile: OpaquePigmentProfile = serde_json::from_value(json!({
            "sideShade": 0, "highlightLift": 0
        }))
        .unwrap();
        assert_eq!(profile.side_shade(), 0.0);
        assert_eq!(profile.highlight_lift(), 0.0);
    }
}
