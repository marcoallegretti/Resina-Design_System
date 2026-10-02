use crate::{ColorRole, MaterialRole, StateSet, SurfaceForm, deserialize_version};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceIntent {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    material_role: MaterialRole,
    color_role: ColorRole,
    form: SurfaceForm,
    states: StateSet,
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
}
