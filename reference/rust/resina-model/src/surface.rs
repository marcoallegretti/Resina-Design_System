use crate::{ColorRole, MaterialRole, StateSet, SurfaceForm, TreatmentStack};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceIntent {
    #[serde(deserialize_with = "deserialize_binding_version")]
    schema_version: String,
    material_role: MaterialRole,
    color_role: ColorRole,
    form: SurfaceForm,
    states: StateSet,
    treatment_stack: TreatmentStack,
}

impl SurfaceIntent {
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
