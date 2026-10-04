mod slider_value;
pub use slider_value::SliderValue;

mod command_appearance;
pub use command_appearance::{
    CommandAppearance, CommandPhase, CommandResponse, resolve_command_phase,
};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
mod activation;
pub use activation::{ActivationEvent, ActivationKey, ActivationState, PressHold};
mod spring;
pub use spring::{SpringDynamics, SpringParameters, SpringState};

mod color;
pub use color::{ColorAssignments, ColorRole, OpaqueColorAssignments};
mod geometry;
pub use geometry::{
    ContourSegment, CornerRadius, LogicalCornerRadii, PhysicalBounds, PhysicalVector, SurfaceSize,
};
mod key_light;
pub use key_light::KeyLight;
mod elevation;
pub use elevation::ElevationDepthAssignments;
mod frost;
pub use frost::FrostPigment;
mod opaque_pigment;
pub use opaque_pigment::{OpaquePigmentProfile, OpaquePigmentProfiles};
mod spatial;
pub use spatial::{SpatialAssignments, SpatialRole};
mod shape_fallback;
pub use shape_fallback::{CornerTokenPaths, ShapeFallbackAssignments, ShapeFallbackProfile};
mod surface;
pub use surface::SurfaceIntent;
mod surface_appearance;
pub use surface_appearance::{OpaqueSurfaceAppearance, SurfaceBandProfile};
mod typography;
pub use typography::{FontFamilyRole, TypographyAssignments, TypographyRole, TypographyRoleSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InteractionState {
    Rest,
    Hover,
    Focused,
    Pressed,
    Active,
    Selected,
    Checked,
    Disabled,
    Busy,
    Dragging,
    Error,
    Warning,
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "StateSetInput"
)]
pub struct StateSet {
    schema_version: String,
    states: Vec<InteractionState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateSetInput {
    pub schema_version: String,
    pub states: Vec<InteractionState>,
}

impl TryFrom<StateSetInput> for StateSet {
    type Error = &'static str;

    fn try_from(mut input: StateSetInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if input.states.is_empty() {
            return Err("states must not be empty");
        }
        input.states.sort_unstable();
        if input.states.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("states must not contain duplicates");
        }
        Ok(Self {
            schema_version: input.schema_version,
            states: input.states,
        })
    }
}

impl StateSet {
    pub fn states(&self) -> &[InteractionState] {
        &self.states
    }

    pub fn contains(&self, state: InteractionState) -> bool {
        self.states.binary_search(&state).is_ok()
    }

    pub fn compose(&self) -> StateComposition {
        let mut result = StateComposition {
            schema_version: "0.1.0",
            availability: Vec::new(),
            validation: Vec::new(),
            selection: Vec::new(),
            interaction: Vec::new(),
            navigation: Vec::new(),
            activity: Vec::new(),
            base: Vec::new(),
        };
        for state in &self.states {
            match state {
                InteractionState::Disabled => result.availability.push(*state),
                InteractionState::Error | InteractionState::Warning | InteractionState::Success => {
                    result.validation.push(*state)
                }
                InteractionState::Active
                | InteractionState::Selected
                | InteractionState::Checked => result.selection.push(*state),
                InteractionState::Hover
                | InteractionState::Pressed
                | InteractionState::Dragging => result.interaction.push(*state),
                InteractionState::Focused => result.navigation.push(*state),
                InteractionState::Busy => result.activity.push(*state),
                InteractionState::Rest => result.base.push(*state),
            }
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateComposition {
    schema_version: &'static str,
    availability: Vec<InteractionState>,
    validation: Vec<InteractionState>,
    selection: Vec<InteractionState>,
    interaction: Vec<InteractionState>,
    navigation: Vec<InteractionState>,
    activity: Vec<InteractionState>,
    base: Vec<InteractionState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateLayer {
    Availability,
    Validation,
    Selection,
    Interaction,
    Navigation,
    Activity,
    Base,
}

impl StateComposition {
    pub fn states(&self, layer: StateLayer) -> &[InteractionState] {
        match layer {
            StateLayer::Availability => &self.availability,
            StateLayer::Validation => &self.validation,
            StateLayer::Selection => &self.selection,
            StateLayer::Interaction => &self.interaction,
            StateLayer::Navigation => &self.navigation,
            StateLayer::Activity => &self.activity,
            StateLayer::Base => &self.base,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialFamily {
    Cast,
    Frost,
    Elastomer,
    Gel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeIntent {
    Structural,
    Soft,
    Rounded,
    Capsule,
    Organic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElevationRole {
    Embedded,
    Base,
    Raised,
    Floating,
    Overlay,
    Modal,
}

impl ElevationRole {
    pub const ALL: [Self; 6] = [
        Self::Embedded,
        Self::Base,
        Self::Raised,
        Self::Floating,
        Self::Overlay,
        Self::Modal,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceForm {
    #[serde(deserialize_with = "deserialize_version")]
    schema_version: String,
    shape: ShapeIntent,
    elevation: ElevationRole,
}

impl SurfaceForm {
    pub fn shape(&self) -> ShapeIntent {
        self.shape
    }

    pub fn elevation(&self) -> ElevationRole {
        self.elevation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpticalTreatment {
    None,
    Lens,
    FocusLens,
    HighlightLens,
}

impl OpticalTreatment {
    fn is_lens(self) -> bool {
        matches!(self, Self::Lens | Self::FocusLens | Self::HighlightLens)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "TreatmentStackInput"
)]
pub struct TreatmentStack {
    schema_version: String,
    treatments: Vec<OpticalTreatment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreatmentStackInput {
    pub schema_version: String,
    pub treatments: Vec<OpticalTreatment>,
}

impl TryFrom<TreatmentStackInput> for TreatmentStack {
    type Error = &'static str;

    fn try_from(input: TreatmentStackInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if input.treatments.is_empty() {
            return Err("treatments must not be empty");
        }
        if input
            .treatments
            .iter()
            .filter(|item| item.is_lens())
            .count()
            > 1
        {
            return Err("lens treatments cannot be nested");
        }
        Ok(Self {
            schema_version: input.schema_version,
            treatments: input.treatments,
        })
    }
}

impl TreatmentStack {
    pub fn treatments(&self) -> &[OpticalTreatment] {
        &self.treatments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrostRepresentation {
    ShapedBackdrop,
    RegularBackdrop,
    TranslucentPigmented,
    OpaqueDimensional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MaterialRole {
    #[serde(rename = "surface.base")]
    SurfaceBase,
    #[serde(rename = "surface.content")]
    SurfaceContent,
    #[serde(rename = "surface.chrome")]
    SurfaceChrome,
    #[serde(rename = "surface.raised")]
    SurfaceRaised,
    #[serde(rename = "surface.transient")]
    SurfaceTransient,
    #[serde(rename = "control.passive")]
    ControlPassive,
    #[serde(rename = "control.interactive")]
    ControlInteractive,
    #[serde(rename = "control.primary")]
    ControlPrimary,
    #[serde(rename = "feedback.focus")]
    FeedbackFocus,
    #[serde(rename = "feedback.selection")]
    FeedbackSelection,
    #[serde(rename = "feedback.drag")]
    FeedbackDrag,
}

impl MaterialRole {
    pub const ALL: [Self; 11] = [
        Self::SurfaceBase,
        Self::SurfaceContent,
        Self::SurfaceChrome,
        Self::SurfaceRaised,
        Self::SurfaceTransient,
        Self::ControlPassive,
        Self::ControlInteractive,
        Self::ControlPrimary,
        Self::FeedbackFocus,
        Self::FeedbackSelection,
        Self::FeedbackDrag,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialAssignments {
    #[serde(deserialize_with = "deserialize_material_version")]
    schema_version: String,
    surface: SurfaceAssignments,
    control: ControlAssignments,
    feedback: FeedbackAssignments,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceAssignments {
    #[serde(deserialize_with = "deserialize_structural_material")]
    base: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    content: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    chrome: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    raised: MaterialFamily,
    transient: MaterialFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlAssignments {
    #[serde(deserialize_with = "deserialize_structural_material")]
    passive: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    interactive: MaterialFamily,
    #[serde(deserialize_with = "deserialize_structural_material")]
    primary: MaterialFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedbackAssignments {
    focus: MaterialFamily,
    selection: MaterialFamily,
    drag: MaterialFamily,
}

impl MaterialAssignments {
    pub fn material_for(&self, role: MaterialRole) -> MaterialFamily {
        match role {
            MaterialRole::SurfaceBase => self.surface.base,
            MaterialRole::SurfaceContent => self.surface.content,
            MaterialRole::SurfaceChrome => self.surface.chrome,
            MaterialRole::SurfaceRaised => self.surface.raised,
            MaterialRole::SurfaceTransient => self.surface.transient,
            MaterialRole::ControlPassive => self.control.passive,
            MaterialRole::ControlInteractive => self.control.interactive,
            MaterialRole::ControlPrimary => self.control.primary,
            MaterialRole::FeedbackFocus => self.feedback.focus,
            MaterialRole::FeedbackSelection => self.feedback.selection,
            MaterialRole::FeedbackDrag => self.feedback.drag,
        }
    }
}

fn deserialize_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version == "0.1.0" {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 0.1.0"))
    }
}

fn deserialize_material_version<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<String, D::Error> {
    let version = String::deserialize(deserializer)?;
    if version == "0.2.0" {
        Ok(version)
    } else {
        Err(D::Error::custom("schemaVersion must be 0.2.0"))
    }
}

fn deserialize_structural_material<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<MaterialFamily, D::Error> {
    let material = MaterialFamily::deserialize(deserializer)?;
    if material == MaterialFamily::Gel {
        Err(D::Error::custom(
            "gel cannot be a default structural material",
        ))
    } else {
        Ok(material)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn surface_form_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/surface-form-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<SurfaceForm>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let form = result.unwrap();
                assert_eq!(
                    serde_json::to_value(form.shape()).unwrap(),
                    expected["shape"],
                    "{}",
                    vector["name"]
                );
                assert_eq!(
                    serde_json::to_value(form.elevation()).unwrap(),
                    expected["elevation"],
                    "{}",
                    vector["name"]
                );
                assert_eq!(
                    serde_json::to_value(&form).unwrap(),
                    vector["document"],
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn treatment_stack_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/treatment-stack-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<TreatmentStack>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let stack = result.unwrap();
                assert_eq!(
                    serde_json::to_value(stack.treatments()).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
                assert_eq!(
                    serde_json::from_value::<TreatmentStack>(serde_json::to_value(&stack).unwrap())
                        .unwrap(),
                    stack
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn state_set_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/states/state-set-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<StateSet>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let states = result.unwrap();
                assert_eq!(
                    serde_json::to_value(states.states()).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
                assert!(states.contains(states.states()[0]));
                let encoded = serde_json::to_value(&states).unwrap();
                assert_eq!(encoded["states"], *expected);
                assert_eq!(serde_json::from_value::<StateSet>(encoded).unwrap(), states);
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn state_composition_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/states/composition-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let states: StateSet = serde_json::from_value(vector["states"].clone()).unwrap();
            assert_eq!(
                serde_json::to_value(states.compose()).unwrap(),
                vector["expected"],
                "{}",
                vector["name"]
            );
        }
    }

    #[test]
    fn material_assignment_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/materials/role-assignment-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = serde_json::from_value::<MaterialAssignments>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let assignments = result.unwrap();
                for (role, family) in expected.as_object().unwrap() {
                    let role: MaterialRole = serde_json::from_value(json!(role)).unwrap();
                    assert_eq!(
                        serde_json::to_value(assignments.material_for(role)).unwrap(),
                        *family,
                        "{}: {role:?}",
                        vector["name"]
                    );
                }
                assert_eq!(
                    serde_json::to_value(&assignments).unwrap(),
                    vector["document"],
                    "{}",
                    vector["name"]
                );
            } else {
                let error = result.unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains(vector["error"].as_str().unwrap()),
                    "{}: {error}",
                    vector["name"]
                );
            }
        }
    }
}
