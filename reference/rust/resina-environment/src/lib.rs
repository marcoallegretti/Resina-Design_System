use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry {
    pub width: f64,
    pub height: f64,
    pub safe_area: SafeArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
    Square,
}

impl Geometry {
    pub fn aspect_ratio(&self) -> f64 {
        self.width / self.height
    }

    pub fn orientation(&self) -> Orientation {
        if self.width > self.height {
            Orientation::Landscape
        } else if self.width < self.height {
            Orientation::Portrait
        } else {
            Orientation::Square
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafeArea {
    pub start: f64,
    pub end: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputCapability {
    FinePointer,
    CoarsePointer,
    Hover,
    DirectTouch,
    Stylus,
    Keyboard,
    DirectionalNavigation,
    Gamepad,
    VoiceAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewingProfile {
    Near,
    Desk,
    Couch,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DensityPreference {
    Compact,
    Standard,
    Comfortable,
    Immersive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessibilityPreferences {
    pub reduced_motion: bool,
    pub reduced_transparency: bool,
    pub high_contrast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutDirection {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RendererCapabilities {
    pub gradients: bool,
    pub inner_shadow: bool,
    pub advanced_shadow: bool,
    pub sdf_shapes: bool,
    pub backdrop_effect: bool,
    pub backdrop_blur: bool,
    pub shaped_backdrop: bool,
    pub dynamic_lighting: bool,
    pub deformation: bool,
    pub masks: bool,
    pub custom_shader: bool,
    pub wide_gamut: bool,
    pub hdr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityPolicy {
    Economy,
    Balanced,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    schema_version: String,
    geometry: Geometry,
    scale: f64,
    text_scale: f64,
    input_capabilities: Vec<InputCapability>,
    viewing_profile: ViewingProfile,
    density_preference: DensityPreference,
    accessibility_preferences: AccessibilityPreferences,
    locale: String,
    layout_direction: LayoutDirection,
    renderer_capabilities: RendererCapabilities,
    quality_policy: QualityPolicy,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvironmentSnapshotInput {
    pub schema_version: String,
    pub geometry: Geometry,
    pub scale: f64,
    pub text_scale: f64,
    pub input_capabilities: Vec<InputCapability>,
    pub viewing_profile: ViewingProfile,
    pub density_preference: DensityPreference,
    pub accessibility_preferences: AccessibilityPreferences,
    pub locale: String,
    pub layout_direction: LayoutDirection,
    pub renderer_capabilities: RendererCapabilities,
    pub quality_policy: QualityPolicy,
}

impl<'de> Deserialize<'de> for EnvironmentSnapshot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        EnvironmentSnapshotInput::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

impl TryFrom<EnvironmentSnapshotInput> for EnvironmentSnapshot {
    type Error = &'static str;

    fn try_from(input: EnvironmentSnapshotInput) -> Result<Self, Self::Error> {
        let snapshot = Self {
            schema_version: input.schema_version,
            geometry: input.geometry,
            scale: input.scale,
            text_scale: input.text_scale,
            input_capabilities: input.input_capabilities,
            viewing_profile: input.viewing_profile,
            density_preference: input.density_preference,
            accessibility_preferences: input.accessibility_preferences,
            locale: input.locale,
            layout_direction: input.layout_direction,
            renderer_capabilities: input.renderer_capabilities,
            quality_policy: input.quality_policy,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

impl EnvironmentSnapshot {
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn text_scale(&self) -> f64 {
        self.text_scale
    }

    pub fn input_capabilities(&self) -> &[InputCapability] {
        &self.input_capabilities
    }

    pub fn viewing_profile(&self) -> ViewingProfile {
        self.viewing_profile
    }

    pub fn density_preference(&self) -> DensityPreference {
        self.density_preference
    }

    pub fn accessibility_preferences(&self) -> &AccessibilityPreferences {
        &self.accessibility_preferences
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }

    pub fn renderer_capabilities(&self) -> &RendererCapabilities {
        &self.renderer_capabilities
    }

    pub fn quality_policy(&self) -> QualityPolicy {
        self.quality_policy
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != "0.1.0" {
            return Err("schemaVersion must be 0.1.0");
        }
        if !positive(self.geometry.width) {
            return Err("geometry.width must be finite and greater than zero");
        }
        if !positive(self.geometry.height) {
            return Err("geometry.height must be finite and greater than zero");
        }
        if !positive(self.geometry.aspect_ratio()) {
            return Err("geometry aspect ratio must be finite and greater than zero");
        }
        let area = &self.geometry.safe_area;
        for (value, message) in [
            (
                area.start,
                "geometry.safeArea.start must be finite and nonnegative",
            ),
            (
                area.end,
                "geometry.safeArea.end must be finite and nonnegative",
            ),
            (
                area.top,
                "geometry.safeArea.top must be finite and nonnegative",
            ),
            (
                area.bottom,
                "geometry.safeArea.bottom must be finite and nonnegative",
            ),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(message);
            }
        }
        if area.start + area.end >= self.geometry.width {
            return Err("geometry.safeArea start + end must be smaller than width");
        }
        if area.top + area.bottom >= self.geometry.height {
            return Err("geometry.safeArea top + bottom must be smaller than height");
        }
        if !positive(self.scale) {
            return Err("scale must be finite and greater than zero");
        }
        if !positive(self.text_scale) {
            return Err("textScale must be finite and greater than zero");
        }
        if self.locale.is_empty() {
            return Err("locale must be nonempty");
        }
        let mut capabilities = HashSet::new();
        if self
            .input_capabilities
            .iter()
            .any(|item| !capabilities.insert(item))
        {
            return Err("inputCapabilities must not contain duplicates");
        }
        Ok(())
    }
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn valid() -> Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/environment/valid-mixed-input.json"
        ))
        .unwrap()
    }

    fn rejection(mut value: Value, edit: impl FnOnce(&mut Value), expected: &str) {
        edit(&mut value);
        let error = serde_json::from_value::<EnvironmentSnapshot>(value).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }

    #[test]
    fn conformance_snapshots_round_trip() {
        for source in [
            include_str!("../../../../conformance/environment/valid-mixed-input.json"),
            include_str!("../../../../conformance/environment/valid-minimal-capabilities.json"),
        ] {
            let snapshot: EnvironmentSnapshot = serde_json::from_str(source).unwrap();
            let encoded = serde_json::to_value(&snapshot).unwrap();
            let reparsed: EnvironmentSnapshot = serde_json::from_value(encoded).unwrap();
            assert_eq!(snapshot, reparsed);
        }
    }

    #[test]
    fn conformance_rejections() {
        for (source, expected) in [
            (
                include_str!("../../../../conformance/environment/invalid-safe-area.json"),
                "start + end",
            ),
            (
                include_str!("../../../../conformance/environment/invalid-duplicate-input.json"),
                "duplicates",
            ),
        ] {
            let error = serde_json::from_str::<EnvironmentSnapshot>(source).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn rejects_missing_and_unknown_fields() {
        rejection(
            valid(),
            |v| {
                v.as_object_mut().unwrap().remove("qualityPolicy");
            },
            "qualityPolicy",
        );
        rejection(
            valid(),
            |v| {
                v["geometry"]["deviceClass"] = json!("desktop");
            },
            "deviceClass",
        );
        rejection(
            valid(),
            |v| {
                v["rendererCapabilities"]
                    .as_object_mut()
                    .unwrap()
                    .remove("gradients");
            },
            "gradients",
        );
    }

    #[test]
    fn rejects_invalid_geometry_and_scale() {
        rejection(
            valid(),
            |v| {
                v["geometry"]["width"] = json!(0);
            },
            "geometry.width",
        );
        rejection(
            valid(),
            |v| {
                v["geometry"]["width"] = json!(1e300);
                v["geometry"]["height"] = json!(1e-300);
            },
            "aspect ratio",
        );
        rejection(
            valid(),
            |v| {
                v["geometry"]["safeArea"]["start"] = json!(1280);
            },
            "start + end",
        );
        rejection(
            valid(),
            |v| {
                v["geometry"]["safeArea"]["top"] = json!(-1);
            },
            "safeArea.top",
        );
        rejection(
            valid(),
            |v| {
                v["scale"] = json!(0);
            },
            "scale",
        );
        rejection(
            valid(),
            |v| {
                v["textScale"] = json!(-1);
            },
            "textScale",
        );
    }

    #[test]
    fn orientation_and_aspect_ratio_follow_geometry() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/environment/geometry-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let mut source = valid();
            source["geometry"]["width"] = vector["width"].clone();
            source["geometry"]["height"] = vector["height"].clone();
            let snapshot: EnvironmentSnapshot = serde_json::from_value(source).unwrap();
            let geometry = snapshot.geometry();
            assert_eq!(
                geometry.aspect_ratio(),
                vector["aspectRatio"].as_f64().unwrap()
            );
            let orientation = match geometry.orientation() {
                Orientation::Portrait => "portrait",
                Orientation::Landscape => "landscape",
                Orientation::Square => "square",
            };
            assert_eq!(orientation, vector["orientation"].as_str().unwrap());
        }
    }

    #[test]
    fn rejects_ambiguous_inputs_and_unsupported_version() {
        rejection(
            valid(),
            |v| {
                v["inputCapabilities"] = json!(["keyboard", "keyboard"]);
            },
            "duplicates",
        );
        rejection(
            valid(),
            |v| {
                v["schemaVersion"] = json!("0.2.0");
            },
            "schemaVersion",
        );
        rejection(
            valid(),
            |v| {
                v["locale"] = json!("");
            },
            "locale",
        );
        rejection(
            valid(),
            |v| {
                v["layoutDirection"] = json!("auto");
            },
            "expected `ltr` or `rtl`",
        );
    }

    #[test]
    fn typed_input_obeys_the_same_validation() {
        let mut input: EnvironmentSnapshotInput = serde_json::from_value(valid()).unwrap();
        input.scale = f64::NAN;
        assert_eq!(
            EnvironmentSnapshot::try_from(input.clone()).unwrap_err(),
            "scale must be finite and greater than zero"
        );

        input.scale = 1.0;
        input.geometry.width = f64::INFINITY;
        assert_eq!(
            EnvironmentSnapshot::try_from(input.clone()).unwrap_err(),
            "geometry.width must be finite and greater than zero"
        );

        input.geometry.width = 1280.0;
        assert!(EnvironmentSnapshot::try_from(input).is_ok());
    }
}
