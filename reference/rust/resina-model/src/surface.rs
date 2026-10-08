use crate::{ColorRole, MaterialRole, StateSet, SurfaceForm, TreatmentStack};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error, MapAccess, Visitor, value::MapAccessDeserializer, value::StringDeserializer},
};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceIntent {
    schema_version: String,
    material_role: MaterialRole,
    color_role: ColorRole,
    form: SurfaceForm,
    states: StateSet,
    treatment_stack: TreatmentStack,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SurfaceIntentInput {
    #[serde(deserialize_with = "deserialize_binding_version")]
    schema_version: String,
    #[serde(deserialize_with = "deserialize_role_string")]
    material_role: MaterialRole,
    #[serde(deserialize_with = "deserialize_role_string")]
    color_role: ColorRole,
    form: SurfaceForm,
    states: StateSet,
    treatment_stack: TreatmentStack,
}

impl<'de> Deserialize<'de> for SurfaceIntent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Intent;
        impl<'de> Visitor<'de> for Intent {
            type Value = SurfaceIntent;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a surface intent object")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = SurfaceIntentInput::deserialize(MapAccessDeserializer::new(map))?;
                Ok(SurfaceIntent {
                    schema_version: input.schema_version,
                    material_role: input.material_role,
                    color_role: input.color_role,
                    form: input.form,
                    states: input.states,
                    treatment_stack: input.treatment_stack,
                })
            }
        }
        deserializer.deserialize_map(Intent)
    }
}

fn deserialize_role_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(
        deserializer,
    )?))
}

impl SurfaceIntent {
    pub(crate) fn untreated(
        material_role: MaterialRole,
        color_role: ColorRole,
        form: SurfaceForm,
        states: StateSet,
    ) -> Self {
        Self {
            schema_version: "0.2.0".to_owned(),
            material_role,
            color_role,
            form,
            states,
            treatment_stack: TreatmentStack::untreated(),
        }
    }

    pub fn with_color_role(mut self, color_role: ColorRole) -> Self {
        self.color_role = color_role;
        self
    }

    pub fn with_states(mut self, states: StateSet) -> Self {
        self.states = states;
        self
    }

    pub fn material_role(&self) -> MaterialRole {
        self.material_role
    }

    pub fn color_role(&self) -> ColorRole {
        self.color_role
    }

    pub fn form(&self) -> &SurfaceForm {
        &self.form
    }

    pub fn states(&self) -> &StateSet {
        &self.states
    }

    pub fn treatment_stack(&self) -> &TreatmentStack {
        &self.treatment_stack
    }
}

fn deserialize_binding_version<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version == "0.2.0" {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 0.2.0"))
    }
}
