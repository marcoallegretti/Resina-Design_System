use crate::{HeadlessResolution, SrgbFallback};
use resina_model::{
    ColorRole, FrostRepresentation, MaterialFamily, MaterialRole, StateSet, SurfaceForm,
    SurfaceIntent, TreatmentStack,
};
use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundSurface {
    schema_version: &'static str,
    material_role: MaterialRole,
    color_role: ColorRole,
    material_family: MaterialFamily,
    source_color: Value,
    color_fallback: SrgbFallback,
    form: SurfaceForm,
    states: StateSet,
    treatment_stack: TreatmentStack,
    #[serde(skip_serializing_if = "Option::is_none")]
    frost_representation: Option<FrostRepresentation>,
}

impl BoundSurface {
    pub fn material_role(&self) -> MaterialRole {
        self.material_role
    }

    pub fn color_role(&self) -> ColorRole {
        self.color_role
    }

    pub fn material_family(&self) -> MaterialFamily {
        self.material_family
    }

    pub fn source_color(&self) -> &Value {
        &self.source_color
    }

    pub fn color_fallback(&self) -> &SrgbFallback {
        &self.color_fallback
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

    pub fn frost_representation(&self) -> Option<FrostRepresentation> {
        self.frost_representation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceBindingError {
    MissingMaterial(MaterialRole),
    MissingColor(ColorRole),
    MissingColorFallback(ColorRole),
}

impl fmt::Display for SurfaceBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMaterial(role) => write!(formatter, "missing material role {role:?}"),
            Self::MissingColor(role) => write!(formatter, "missing color role {role:?}"),
            Self::MissingColorFallback(role) => {
                write!(formatter, "missing sRGB fallback for color role {role:?}")
            }
        }
    }
}

impl std::error::Error for SurfaceBindingError {}

pub fn bind_surface(
    intent: &SurfaceIntent,
    context: &HeadlessResolution,
) -> Result<BoundSurface, SurfaceBindingError> {
    let material_role = intent.material_role();
    let color_role = intent.color_role();
    let material_family = *context
        .materials()
        .get(&material_role)
        .ok_or(SurfaceBindingError::MissingMaterial(material_role))?;
    let source_color = context
        .colors()
        .get(&color_role)
        .ok_or(SurfaceBindingError::MissingColor(color_role))?
        .clone();
    let color_fallback = context
        .color_fallbacks()
        .get(&color_role)
        .ok_or(SurfaceBindingError::MissingColorFallback(color_role))?
        .clone();
    let frost_representation =
        (material_family == MaterialFamily::Frost).then(|| context.frost_representation());
    Ok(BoundSurface {
        schema_version: "0.2.0",
        material_role,
        color_role,
        material_family,
        source_color,
        color_fallback,
        form: intent.form().clone(),
        states: intent.states().clone(),
        treatment_stack: intent.treatment_stack().clone(),
        frost_representation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve_headless_source;
    use serde_json::{Value, json};

    const SOURCE: &str = include_str!("../../../../conformance/headless/valid-request.json");
    const VECTORS: &str = include_str!("../../../../conformance/surfaces/binding-vectors.json");

    #[test]
    fn surface_binding_conformance_vectors() {
        let context = resolve_headless_source(SOURCE).unwrap();
        let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
        for vector in vectors {
            let intent = serde_json::from_value::<SurfaceIntent>(vector["document"].clone());
            if let Some(expected) = vector.get("expected") {
                let intent = intent.unwrap();
                let bound = bind_surface(&intent, &context).unwrap();
                assert_eq!(
                    serde_json::to_value(bound).unwrap(),
                    *expected,
                    "{}",
                    vector["name"]
                );
            } else {
                let error = intent.unwrap_err();
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
    fn accessibility_changes_frost_binding_only() {
        let mut source: Value = serde_json::from_str(SOURCE).unwrap();
        source["environment"]["accessibilityPreferences"]["highContrast"] = json!(true);
        let context = resolve_headless_source(&source.to_string()).unwrap();
        let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
        let frost: SurfaceIntent = serde_json::from_value(vectors[0]["document"].clone()).unwrap();
        let elastomer: SurfaceIntent =
            serde_json::from_value(vectors[1]["document"].clone()).unwrap();
        assert_eq!(
            bind_surface(&frost, &context)
                .unwrap()
                .frost_representation(),
            Some(FrostRepresentation::OpaqueDimensional)
        );
        assert_eq!(
            bind_surface(&elastomer, &context)
                .unwrap()
                .frost_representation(),
            None
        );
    }

    #[test]
    fn binding_keeps_source_color_and_its_portable_fallback_distinct() {
        let mut source: Value = serde_json::from_str(SOURCE).unwrap();
        let authored = json!({
            "colorSpace":"display-p3",
            "components":[0.25,0.5,0.75],
            "hex":"#336699",
            "alpha":0.35
        });
        source["tokens"]["palette"]["focus"] = json!({"$value": authored});
        source["colorAssignments"]["roles"]["focus"] = json!("palette.focus");
        let context = resolve_headless_source(&source.to_string()).unwrap();
        let vectors: Vec<Value> = serde_json::from_str(VECTORS).unwrap();
        let mut intent = vectors[1]["document"].clone();
        intent["colorRole"] = json!("focus");
        let intent: SurfaceIntent = serde_json::from_value(intent).unwrap();
        let bound = bind_surface(&intent, &context).unwrap();
        assert_eq!(bound.color_role(), ColorRole::Focus);
        assert_eq!(bound.source_color(), &authored);
        assert_eq!(bound.color_fallback().components(), [0.2, 0.4, 0.6]);
        assert_eq!(bound.color_fallback().alpha(), 0.35);
    }
}
