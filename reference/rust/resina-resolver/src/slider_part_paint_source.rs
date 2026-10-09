use crate::{
    OpaqueSurfaceError, SliderPartPaintError, SliderPartPaintInput, SliderPartPaintIr,
    ThemeResolutionError, resolve_slider_part_paint,
    srgb_input::{SrgbInput, SrgbInputError},
    theme_request::compile_theme_request_document,
};
use resina_color::{ContrastRangeError, OpaqueSrgbRange};
use resina_model::{
    ColorRole, OpaqueSurfaceAppearance, SliderAppearance, SliderPart, SurfaceIntent, SurfaceSize,
};
use resina_tokens::parse_token_document;
use serde::{
    Deserialize, Deserializer,
    de::{
        MapAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use serde_json::Value;
use std::fmt;

#[derive(Debug)]
pub enum SliderPartPaintSourceError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidRequestShape,
    UnsupportedVersion,
    Theme(ThemeResolutionError),
    Color(OpaqueSurfaceError),
    Range(ContrastRangeError),
    Paint(SliderPartPaintError),
}

impl fmt::Display for SliderPartPaintSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "slider part paint parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid slider part paint request: {error}"),
            Self::InvalidRequestShape => {
                f.write_str("slider part paint request must be a JSON object")
            }
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Theme(error) => error.fmt(f),
            Self::Color(error) => error.fmt(f),
            Self::Range(error) => write!(f, "slider part paint range: {error}"),
            Self::Paint(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SliderPartPaintSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) | Self::Request(e) => Some(e),
            Self::Theme(e) => Some(e),
            Self::Color(e) => Some(e),
            Self::Range(e) => Some(e),
            Self::Paint(e) => Some(e),
            Self::InvalidRequestShape | Self::UnsupportedVersion => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    #[serde(deserialize_with = "part_name")]
    part: SliderPart,
    read_only: bool,
    theme: Value,
    surface: SurfaceIntent,
    size: SurfaceSize,
    appearance: OpaqueSurfaceAppearance,
    interaction_appearance: SliderAppearance,
    #[serde(deserialize_with = "foreground_role_name")]
    foreground_role: ColorRole,
    #[serde(default, deserialize_with = "present_backdrop")]
    post_treatment_backdrop: Option<SrgbInput>,
    adjacent_ranges: Vec<RangeInput>,
    #[serde(default, deserialize_with = "present_ranges")]
    surrounding_ranges: Option<Vec<RangeInput>>,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

fn part_name<'de, D: Deserializer<'de>>(d: D) -> Result<SliderPart, D::Error> {
    SliderPart::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(d)?))
}

fn foreground_role_name<'de, D: Deserializer<'de>>(d: D) -> Result<ColorRole, D::Error> {
    ColorRole::deserialize(StringDeserializer::<D::Error>::new(String::deserialize(d)?))
}

fn present_backdrop<'de, D: Deserializer<'de>>(d: D) -> Result<Option<SrgbInput>, D::Error> {
    SrgbInput::deserialize(d).map(Some)
}

fn present_ranges<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<RangeInput>>, D::Error> {
    Vec::deserialize(d).map(Some)
}

struct RangeInput {
    lower: SrgbInput,
    upper: SrgbInput,
}

impl<'de> Deserialize<'de> for RangeInput {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Members {
            lower: SrgbInput,
            upper: SrgbInput,
        }
        struct RangeVisitor;
        impl<'de> Visitor<'de> for RangeVisitor {
            type Value = RangeInput;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an opaque sRGB range object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let members = Members::deserialize(MapAccessDeserializer::new(map))?;
                Ok(RangeInput {
                    lower: members.lower,
                    upper: members.upper,
                })
            }
        }
        d.deserialize_map(RangeVisitor)
    }
}

fn color(
    field: &'static str,
    input: SrgbInput,
) -> Result<crate::SrgbFallback, SliderPartPaintSourceError> {
    input.into_fallback().map_err(|e| {
        SliderPartPaintSourceError::Color(match e {
            SrgbInputError::InvalidColorSpace => OpaqueSurfaceError::InvalidColorSpace(field),
            SrgbInputError::Color(e) => OpaqueSurfaceError::Color(field, e),
        })
    })
}

fn ranges(inputs: Vec<RangeInput>) -> Result<Vec<OpaqueSrgbRange>, SliderPartPaintSourceError> {
    inputs
        .into_iter()
        .map(|input| {
            OpaqueSrgbRange::try_new(
                color("range lower", input.lower)?,
                color("range upper", input.upper)?,
            )
            .map_err(SliderPartPaintSourceError::Range)
        })
        .collect()
}

pub fn resolve_slider_part_paint_source(
    source: &str,
) -> Result<SliderPartPaintIr, SliderPartPaintSourceError> {
    let document = parse_token_document(source).map_err(SliderPartPaintSourceError::Parse)?;
    if !document.is_object() {
        return Err(SliderPartPaintSourceError::InvalidRequestShape);
    }
    let request: Request =
        serde_json::from_value(document).map_err(SliderPartPaintSourceError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SliderPartPaintSourceError::UnsupportedVersion);
    }
    let (theme, environment) =
        compile_theme_request_document(request.theme).map_err(SliderPartPaintSourceError::Theme)?;
    let backdrop = request
        .post_treatment_backdrop
        .map(|input| color("postTreatmentBackdrop", input))
        .transpose()?;
    let adjacent = ranges(request.adjacent_ranges)?;
    let surrounding = request.surrounding_ranges.map(ranges).transpose()?;
    resolve_slider_part_paint(
        &theme,
        &environment,
        SliderPartPaintInput {
            part: request.part,
            read_only: request.read_only,
            surface: &request.surface,
            size: request.size,
            appearance: &request.appearance,
            interaction_appearance: &request.interaction_appearance,
            foreground_role: request.foreground_role,
            post_treatment_backdrop: backdrop.as_ref(),
            adjacent_ranges: &adjacent,
            surrounding_ranges: surrounding.as_deref(),
            minimum_content_contrast: request.minimum_content_contrast,
            minimum_edge_contrast: request.minimum_edge_contrast,
        },
    )
    .map_err(SliderPartPaintSourceError::Paint)
}
