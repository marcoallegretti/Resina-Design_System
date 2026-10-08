use crate::{
    ColorRole, ElevationRole, MaterialRole, ShapeIntent, StateSet, SurfaceForm, SurfaceIntent,
    TypographyRole,
};
use serde::{
    Deserialize, Deserializer,
    de::{
        MapAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use std::{fmt, marker::PhantomData};

fn deserialize_anatomy_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(
        deserializer,
    )?))
}

fn deserialize_anatomy_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Object<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a component anatomy object")
        }
        fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(PhantomData))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum ControlRole {
    #[serde(rename = "control.passive")]
    Passive,
    #[serde(rename = "control.interactive")]
    Interactive,
    #[serde(rename = "control.primary")]
    Primary,
}

impl From<ControlRole> for MaterialRole {
    fn from(role: ControlRole) -> Self {
        match role {
            ControlRole::Passive => Self::ControlPassive,
            ControlRole::Interactive => Self::ControlInteractive,
            ControlRole::Primary => Self::ControlPrimary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum CommandComponent {
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ToggleComponent {
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandAnatomy {
    schema_version: String,
    component: CommandComponent,
    variants: CommandVariants,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandAnatomyInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    component: CommandComponent,
    #[serde(deserialize_with = "deserialize_anatomy_object")]
    variants: CommandVariants,
}

impl<'de> Deserialize<'de> for CommandAnatomy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = deserialize_anatomy_object::<D, CommandAnatomyInput>(deserializer)?;
        Ok(Self {
            schema_version: input.schema_version,
            component: input.component,
            variants: input.variants,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandVariants {
    standard: CommandVariant,
    primary: CommandVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEmphasis {
    Standard,
    Primary,
}

impl CommandAnatomy {
    pub fn variant(&self, emphasis: CommandEmphasis) -> &CommandVariant {
        match emphasis {
            CommandEmphasis::Standard => &self.variants.standard,
            CommandEmphasis::Primary => &self.variants.primary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandVariant {
    body: CommandBody,
    label: EmbeddedLabel,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandVariantInput {
    body: CommandBody,
    #[serde(deserialize_with = "deserialize_anatomy_object")]
    label: EmbeddedLabel,
}

impl<'de> Deserialize<'de> for CommandVariant {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = deserialize_anatomy_object::<D, CommandVariantInput>(deserializer)?;
        Ok(Self {
            body: input.body,
            label: input.label,
        })
    }
}

impl CommandVariant {
    pub fn body(&self) -> &CommandBody {
        &self.body
    }
    pub fn label_typography(&self) -> TypographyRole {
        self.label.typography_role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBody {
    material_role: ControlRole,
    color_role: ColorRole,
    shape: ShapeIntent,
    elevation: ElevationRole,
    content_role: ColorRole,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandBodyInput {
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    material_role: ControlRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    color_role: ColorRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    shape: ShapeIntent,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    elevation: ElevationRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    content_role: ColorRole,
}

impl<'de> Deserialize<'de> for CommandBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = deserialize_anatomy_object::<D, CommandBodyInput>(deserializer)?;
        Ok(Self {
            material_role: input.material_role,
            color_role: input.color_role,
            shape: input.shape,
            elevation: input.elevation,
            content_role: input.content_role,
        })
    }
}

impl CommandBody {
    pub fn content_role(&self) -> ColorRole {
        self.content_role
    }
    pub fn intent(&self, states: StateSet) -> SurfaceIntent {
        SurfaceIntent::untreated(
            self.material_role.into(),
            self.color_role,
            SurfaceForm::new(self.shape, self.elevation),
            states,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbeddedLabel {
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    typography_role: TypographyRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToggleAnatomy {
    schema_version: String,
    component: ToggleComponent,
    track: SelectablePart,
    thumb: SelectablePart,
    label: ExternalLabel,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ToggleAnatomyInput {
    #[serde(deserialize_with = "crate::deserialize_version")]
    schema_version: String,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    component: ToggleComponent,
    track: SelectablePart,
    thumb: SelectablePart,
    #[serde(deserialize_with = "deserialize_anatomy_object")]
    label: ExternalLabel,
}

impl<'de> Deserialize<'de> for ToggleAnatomy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = deserialize_anatomy_object::<D, ToggleAnatomyInput>(deserializer)?;
        Ok(Self {
            schema_version: input.schema_version,
            component: input.component,
            track: input.track,
            thumb: input.thumb,
            label: input.label,
        })
    }
}

impl ToggleAnatomy {
    pub fn track(&self) -> &SelectablePart {
        &self.track
    }
    pub fn thumb(&self) -> &SelectablePart {
        &self.thumb
    }
    pub fn label_typography(&self) -> TypographyRole {
        self.label.typography_role
    }
    pub fn label_color(&self) -> ColorRole {
        self.label.color_role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectablePart {
    material_role: ControlRole,
    color_role: ColorRole,
    checked_color_role: ColorRole,
    shape: ShapeIntent,
    elevation: ElevationRole,
    content_role: ColorRole,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectablePartInput {
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    material_role: ControlRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    color_role: ColorRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    checked_color_role: ColorRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    shape: ShapeIntent,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    elevation: ElevationRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    content_role: ColorRole,
}

impl<'de> Deserialize<'de> for SelectablePart {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let input = deserialize_anatomy_object::<D, SelectablePartInput>(deserializer)?;
        Ok(Self {
            material_role: input.material_role,
            color_role: input.color_role,
            checked_color_role: input.checked_color_role,
            shape: input.shape,
            elevation: input.elevation,
            content_role: input.content_role,
        })
    }
}

impl SelectablePart {
    pub fn checked_color_role(&self) -> ColorRole {
        self.checked_color_role
    }
    pub fn content_role(&self) -> ColorRole {
        self.content_role
    }
    /// The unchecked role stays on the intent; part paint binds the checked role
    /// itself when the states contain checked.
    pub fn intent(&self, states: StateSet) -> SurfaceIntent {
        SurfaceIntent::untreated(
            self.material_role.into(),
            self.color_role,
            SurfaceForm::new(self.shape, self.elevation),
            states,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExternalLabel {
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    typography_role: TypographyRole,
    #[serde(deserialize_with = "deserialize_anatomy_string")]
    color_role: ColorRole,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const COMMAND: &str = include_str!("../../../../definitions/components/command.json");
    const TOGGLE: &str = include_str!("../../../../definitions/components/toggle.json");

    fn states(names: &[&str]) -> StateSet {
        serde_json::from_value(json!({"schemaVersion": "0.1.0", "states": names})).unwrap()
    }

    #[test]
    fn authored_anatomies_build_untreated_intents_for_actual_states() {
        let command: CommandAnatomy = serde_json::from_str(COMMAND).unwrap();
        let primary = command.variant(CommandEmphasis::Primary);
        let intent = primary.body().intent(states(&["pressed", "focused"]));
        assert_eq!(intent.material_role(), MaterialRole::ControlPrimary);
        assert_eq!(intent.color_role(), ColorRole::AccentPrimary);
        assert_eq!(intent.form().shape(), ShapeIntent::Rounded);
        assert_eq!(intent.states(), &states(&["pressed", "focused"]));
        assert_eq!(
            intent.treatment_stack().treatments(),
            [crate::OpticalTreatment::None]
        );
        assert_eq!(primary.body().content_role(), ColorRole::ContentInverse);
        assert_eq!(primary.label_typography(), TypographyRole::Label);
        let reparsed: SurfaceIntent =
            serde_json::from_value(serde_json::to_value(&intent).unwrap()).unwrap();
        assert_eq!(reparsed, intent);

        let toggle: ToggleAnatomy = serde_json::from_str(TOGGLE).unwrap();
        let track = toggle.track().intent(states(&["rest", "checked"]));
        assert_eq!(track.color_role(), ColorRole::SurfaceLow);
        assert_eq!(toggle.track().checked_color_role(), ColorRole::Selection);
        assert_eq!(
            toggle.thumb().intent(states(&["hover"])).material_role(),
            MaterialRole::ControlInteractive
        );
        assert_eq!(toggle.thumb().content_role(), ColorRole::ContentInverse);
        assert_eq!(toggle.label_typography(), TypographyRole::Label);
        assert_eq!(toggle.label_color(), ColorRole::ContentPrimary);
    }

    #[test]
    fn incomplete_or_unknown_anatomy_members_are_rejected() {
        let command: Value = serde_json::from_str(COMMAND).unwrap();
        let toggle: Value = serde_json::from_str(TOGGLE).unwrap();
        for (pointer, value) in [
            ("/schemaVersion", json!("0.2.0")),
            ("/component", json!("toggle")),
            (
                "/variants/standard/body/materialRole",
                json!("surface.base"),
            ),
            (
                "/variants/standard/body/materialRole",
                json!("feedback.drag"),
            ),
            ("/variants/primary/body/shape", json!("pill")),
            ("/variants/primary/label/typographyRole", json!("small")),
        ] {
            let mut invalid = command.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(serde_json::from_value::<CommandAnatomy>(invalid).is_err());
        }
        let mut missing = command.clone();
        missing["variants"]
            .as_object_mut()
            .unwrap()
            .remove("primary");
        assert!(serde_json::from_value::<CommandAnatomy>(missing).is_err());
        let mut extra = command.clone();
        extra["variants"]["standard"]["body"]["checkedColorRole"] = json!("selection");
        assert!(serde_json::from_value::<CommandAnatomy>(extra).is_err());
        let mut missing = toggle.clone();
        missing["thumb"]
            .as_object_mut()
            .unwrap()
            .remove("checkedColorRole");
        assert!(serde_json::from_value::<ToggleAnatomy>(missing).is_err());
        let mut missing = toggle.clone();
        missing["thumb"]
            .as_object_mut()
            .unwrap()
            .remove("contentRole");
        assert!(serde_json::from_value::<ToggleAnatomy>(missing).is_err());
        let mut extra = toggle.clone();
        extra["track"]["foregroundRole"] = json!("content.primary");
        assert!(serde_json::from_value::<ToggleAnatomy>(extra).is_err());
        assert!(serde_json::from_str::<ToggleAnatomy>(COMMAND).is_err());
        assert!(serde_json::from_str::<CommandAnatomy>(TOGGLE).is_err());
    }
}
