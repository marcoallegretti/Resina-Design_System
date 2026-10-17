use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconKind {
    Symbolic,
    Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconLayerRole {
    #[serde(rename = "content")]
    Content,
    #[serde(rename = "accent.primary")]
    AccentPrimary,
    #[serde(rename = "accent.secondary")]
    AccentSecondary,
    #[serde(rename = "accent.tertiary")]
    AccentTertiary,
    #[serde(rename = "status.error")]
    StatusError,
    #[serde(rename = "status.warning")]
    StatusWarning,
    #[serde(rename = "status.success")]
    StatusSuccess,
    #[serde(rename = "status.info")]
    StatusInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconDirection {
    Mirror,
    Invariant,
    DirectionalVariants { ltr: String, rtl: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IconContentInput", into = "IconContentInput")]
pub struct IconContent {
    reference: String,
    kind: IconKind,
    grid: u32,
    ink_bounds: IconBounds,
    optical_bounds: IconBounds,
    direction: IconDirection,
    layers: Vec<IconLayerRole>,
    baked_light: bool,
}

impl IconContent {
    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn kind(&self) -> IconKind {
        self.kind
    }

    pub fn grid(&self) -> u32 {
        self.grid
    }

    pub fn ink_bounds(&self) -> IconBounds {
        self.ink_bounds
    }

    pub fn optical_bounds(&self) -> IconBounds {
        self.optical_bounds
    }

    pub fn direction(&self) -> &IconDirection {
        &self.direction
    }

    pub fn layers(&self) -> &[IconLayerRole] {
        &self.layers
    }

    pub fn baked_light(&self) -> bool {
        self.baked_light
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DirectionBehavior {
    Mirror,
    Invariant,
    DirectionalVariants,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectionInput {
    behavior: DirectionBehavior,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ltr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rtl: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerInput {
    role: IconLayerRole,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IconContentInput {
    schema_version: String,
    reference: String,
    kind: IconKind,
    grid: f64,
    ink_bounds: IconBounds,
    optical_bounds: IconBounds,
    direction: DirectionInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layers: Option<Vec<LayerInput>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baked_light: Option<bool>,
}

impl TryFrom<IconContentInput> for IconContent {
    type Error = String;

    fn try_from(input: IconContentInput) -> Result<Self, Self::Error> {
        if input.schema_version != "0.1.0" {
            return Err("unsupported icon content version".into());
        }
        validate_reference(&input.reference)?;
        if !(input.grid >= 1.0 && input.grid.fract() == 0.0 && input.grid <= f64::from(u32::MAX)) {
            return Err("icon grid must be a positive integer".into());
        }
        validate_bounds("inkBounds", input.ink_bounds, input.grid)?;
        validate_bounds("opticalBounds", input.optical_bounds, input.grid)?;
        let direction = direction(input.direction, &input.reference)?;
        let (layers, baked_light) = match input.kind {
            IconKind::Symbolic => {
                if input.baked_light.is_some() {
                    return Err("symbolic icon must not declare bakedLight".into());
                }
                (layers(input.layers.unwrap_or_default())?, false)
            }
            IconKind::Color => {
                if input.layers.is_some() {
                    return Err("color icon must not declare layers".into());
                }
                let baked_light = input.baked_light.ok_or("color icon requires bakedLight")?;
                if baked_light && direction == IconDirection::Mirror {
                    return Err("color icon with baked light must not mirror".into());
                }
                (Vec::new(), baked_light)
            }
        };
        Ok(Self {
            reference: input.reference,
            kind: input.kind,
            grid: input.grid as u32,
            ink_bounds: input.ink_bounds,
            optical_bounds: input.optical_bounds,
            direction,
            layers,
            baked_light,
        })
    }
}

impl From<IconContent> for IconContentInput {
    fn from(content: IconContent) -> Self {
        let (behavior, ltr, rtl) = match content.direction {
            IconDirection::Mirror => (DirectionBehavior::Mirror, None, None),
            IconDirection::Invariant => (DirectionBehavior::Invariant, None, None),
            IconDirection::DirectionalVariants { ltr, rtl } => {
                (DirectionBehavior::DirectionalVariants, Some(ltr), Some(rtl))
            }
        };
        let symbolic = content.kind == IconKind::Symbolic;
        Self {
            schema_version: "0.1.0".into(),
            reference: content.reference,
            kind: content.kind,
            grid: f64::from(content.grid),
            ink_bounds: content.ink_bounds,
            optical_bounds: content.optical_bounds,
            direction: DirectionInput { behavior, ltr, rtl },
            layers: symbolic.then(|| {
                content
                    .layers
                    .into_iter()
                    .map(|role| LayerInput { role })
                    .collect()
            }),
            baked_light: (!symbolic).then_some(content.baked_light),
        }
    }
}

fn validate_reference(reference: &str) -> Result<(), String> {
    let valid = reference.split_once(':').is_some_and(|(namespace, name)| {
        namespace.split('.').all(|segment| {
            segment.starts_with(|c: char| c.is_ascii_lowercase())
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        }) && name.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
    });
    if valid {
        Ok(())
    } else {
        Err(format!("invalid icon reference: {reference:?}"))
    }
}

fn validate_bounds(field: &str, bounds: IconBounds, grid: f64) -> Result<(), String> {
    if bounds.x < 0.0 || bounds.y < 0.0 {
        return Err(format!("{field} must have a nonnegative origin"));
    }
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Err(format!("{field} must have a positive extent"));
    }
    if bounds.x + bounds.width > grid || bounds.y + bounds.height > grid {
        return Err(format!("{field} exceeds the icon grid"));
    }
    Ok(())
}

fn direction(input: DirectionInput, reference: &str) -> Result<IconDirection, String> {
    match (input.behavior, input.ltr, input.rtl) {
        (DirectionBehavior::Mirror, None, None) => Ok(IconDirection::Mirror),
        (DirectionBehavior::Invariant, None, None) => Ok(IconDirection::Invariant),
        (DirectionBehavior::Mirror | DirectionBehavior::Invariant, _, _) => {
            Err("mirror and invariant directions take no variants".into())
        }
        (DirectionBehavior::DirectionalVariants, Some(ltr), Some(rtl)) => {
            validate_reference(&ltr)?;
            validate_reference(&rtl)?;
            if ltr == rtl {
                return Err("directional variants must be distinct".into());
            }
            if ltr == reference || rtl == reference {
                return Err("directional variant must not repeat the icon reference".into());
            }
            Ok(IconDirection::DirectionalVariants { ltr, rtl })
        }
        (DirectionBehavior::DirectionalVariants, _, _) => {
            Err("directional variants require ltr and rtl".into())
        }
    }
}

fn layers(input: Vec<LayerInput>) -> Result<Vec<IconLayerRole>, String> {
    let roles: Vec<_> = input.into_iter().map(|layer| layer.role).collect();
    match roles.first() {
        None => return Err("symbolic icon requires layers".into()),
        Some(IconLayerRole::Content) => {}
        Some(_) => return Err("first icon layer must use the content role".into()),
    }
    for (index, role) in roles.iter().enumerate().skip(1) {
        if *role == IconLayerRole::Content {
            return Err("content role is only valid for the first layer".into());
        }
        if roles[..index].contains(role) {
            return Err("duplicate icon layer role".into());
        }
    }
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn content_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/content/icon-content-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let name = vector["name"].as_str().unwrap();
            for result in [
                serde_json::from_str::<IconContent>(&vector["document"].to_string()),
                serde_json::from_value::<IconContent>(vector["document"].clone()),
            ] {
                match vector.get("error") {
                    None => {
                        let content = result.unwrap();
                        let written = serde_json::to_value(&content).unwrap();
                        assert_eq!(
                            serde_json::from_value::<IconContent>(written).unwrap(),
                            content,
                            "{name}"
                        );
                    }
                    Some(expected) => {
                        let error = result.unwrap_err().to_string();
                        assert!(
                            error.contains(expected.as_str().unwrap()),
                            "{name}: {error}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn symbolic_icon_exposes_its_metadata() {
        let content: IconContent = serde_json::from_str(
            r#"{"schemaVersion":"0.1.0","reference":"resina.fixture:undo","kind":"symbolic","grid":24,
            "inkBounds":{"x":3,"y":4,"width":18,"height":16},
            "opticalBounds":{"x":2.5,"y":3,"width":19,"height":18},
            "direction":{"behavior":"directionalVariants","ltr":"resina.fixture:undo-ltr","rtl":"resina.fixture:undo-rtl"},
            "layers":[{"role":"content"},{"role":"status.warning"}]}"#,
        )
        .unwrap();
        assert_eq!(content.reference(), "resina.fixture:undo");
        assert_eq!(content.kind(), IconKind::Symbolic);
        assert_eq!(content.grid(), 24);
        assert_eq!(content.ink_bounds().width, 18.0);
        assert_eq!(content.optical_bounds().x, 2.5);
        assert_eq!(
            content.direction(),
            &IconDirection::DirectionalVariants {
                ltr: "resina.fixture:undo-ltr".into(),
                rtl: "resina.fixture:undo-rtl".into()
            }
        );
        assert_eq!(
            content.layers(),
            [IconLayerRole::Content, IconLayerRole::StatusWarning]
        );
        assert!(!content.baked_light());
    }

    #[test]
    fn duplicate_members_are_rejected() {
        for source in [
            r#"{"schemaVersion":"0.1.0","reference":"resina.fixture:clock","reference":"resina.fixture:other","kind":"symbolic","grid":24,"inkBounds":{"x":0,"y":0,"width":24,"height":24},"opticalBounds":{"x":0,"y":0,"width":24,"height":24},"direction":{"behavior":"invariant"},"layers":[{"role":"content"}]}"#,
            r#"{"schemaVersion":"0.1.0","reference":"resina.fixture:clock","kind":"symbolic","grid":24,"inkBounds":{"x":0,"x":1,"y":0,"width":20,"height":24},"opticalBounds":{"x":0,"y":0,"width":24,"height":24},"direction":{"behavior":"invariant"},"layers":[{"role":"content"}]}"#,
            r#"{"schemaVersion":"0.1.0","reference":"resina.fixture:clock","kind":"symbolic","grid":24,"inkBounds":{"x":0,"y":0,"width":24,"height":24},"opticalBounds":{"x":0,"y":0,"width":24,"height":24},"direction":{"behavior":"invariant","behavior":"mirror"},"layers":[{"role":"content"}]}"#,
        ] {
            let error = serde_json::from_str::<IconContent>(source).unwrap_err();
            assert!(error.to_string().contains("duplicate field"), "{error}");
        }
    }
}
