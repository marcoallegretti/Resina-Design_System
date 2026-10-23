use crate::{
    ToggleLayoutError, ToggleLayoutInput,
    profile_dimension::{Bound, DimensionFault, px_dimension},
    resolve_toggle_layout,
};
use resina_environment::{DensityPreference, EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{SizeTokenPaths, SurfaceSize, ToggleLayoutProfile};
use resina_tokens::{DocumentError, ResolvedToken, resolve_token_document};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

pub struct ToggleProfileInput<'a> {
    pub profile: &'a ToggleLayoutProfile,
    pub tokens: &'a BTreeMap<String, ResolvedToken>,
    pub environment: &'a EnvironmentSnapshot,
    pub available_size: SurfaceSize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleProfileIr {
    schema_version: &'static str,
    density_preference: DensityPreference,
    layout_direction: LayoutDirection,
    track_size: SurfaceSize,
    thumb_size: SurfaceSize,
    insets: SafeArea,
}

impl ToggleProfileIr {
    pub fn track_size(&self) -> SurfaceSize {
        self.track_size
    }
    pub fn thumb_size(&self) -> SurfaceSize {
        self.thumb_size
    }
    pub fn insets(&self) -> &SafeArea {
        &self.insets
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
}

#[derive(Debug)]
pub enum ToggleProfileError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidShape(&'static str),
    UnsupportedVersion,
    Tokens(Vec<DocumentError>),
    MissingToken(String),
    TokenType { path: String, found: String },
    InvalidToken { path: String, detail: String },
    TokenUnit(String),
    TokenRange { path: String, bound: &'static str },
    Allocation(DensityPreference, ToggleLayoutError),
    InvalidAvailableSize,
    ContainerBelowTrack,
}

impl fmt::Display for ToggleProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "toggle profile parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid toggle profile request: {error}"),
            Self::InvalidShape(field) => {
                write!(f, "toggle profile request {field} must be a JSON object")
            }
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Tokens(errors) => {
                f.write_str("toggle profile token resolution failed")?;
                for error in errors {
                    write!(f, "\n  {error}")?;
                }
                Ok(())
            }
            Self::MissingToken(path) => write!(f, "missing toggle token {path}"),
            Self::TokenType { path, found } => {
                write!(f, "toggle token {path} must be dimension, found {found}")
            }
            Self::InvalidToken { path, detail } => {
                write!(f, "invalid toggle token {path}: {detail}")
            }
            Self::TokenUnit(path) => write!(f, "toggle token {path} must use px"),
            Self::TokenRange { path, bound } => {
                write!(f, "toggle token {path} must be finite and {bound}")
            }
            Self::Allocation(density, error) => {
                write!(
                    f,
                    "{density:?} track and thumb cannot be allocated: {error}"
                )
            }
            Self::InvalidAvailableSize => f.write_str("availableSize must be finite and positive"),
            Self::ContainerBelowTrack => {
                f.write_str("availableSize is smaller than the profile track")
            }
        }
    }
}

impl std::error::Error for ToggleProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Allocation(_, error) => Some(error),
            _ => None,
        }
    }
}

fn dimension(
    path: &str,
    tokens: &BTreeMap<String, ResolvedToken>,
    bound: Bound,
) -> Result<f64, ToggleProfileError> {
    px_dimension(path, tokens, bound).map_err(|fault| {
        let path = path.to_owned();
        match fault {
            DimensionFault::Missing => ToggleProfileError::MissingToken(path),
            DimensionFault::Type(found) => ToggleProfileError::TokenType { path, found },
            DimensionFault::Invalid(detail) => ToggleProfileError::InvalidToken { path, detail },
            DimensionFault::Unit => ToggleProfileError::TokenUnit(path),
            DimensionFault::Range => ToggleProfileError::TokenRange {
                path,
                bound: match bound {
                    Bound::Positive => "positive",
                    Bound::Nonnegative => "nonnegative",
                },
            },
        }
    })
}

fn size(
    paths: &SizeTokenPaths,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<SurfaceSize, ToggleProfileError> {
    Ok(SurfaceSize {
        width: dimension(&paths.width, tokens, Bound::Positive)?,
        height: dimension(&paths.height, tokens, Bound::Positive)?,
    })
}

pub fn resolve_toggle_profile(
    input: ToggleProfileInput<'_>,
) -> Result<ToggleProfileIr, ToggleProfileError> {
    let available = input.available_size;
    if !(available.width.is_finite()
        && available.height.is_finite()
        && available.width > 0.0
        && available.height > 0.0)
    {
        return Err(ToggleProfileError::InvalidAvailableSize);
    }
    let modes = input.profile.parts().try_map(|density, parts| {
        let track_size = size(&parts.track_size, input.tokens)?;
        let thumb_size = size(&parts.thumb_size, input.tokens)?;
        let inset = dimension(&parts.thumb_inset, input.tokens, Bound::Nonnegative)?;
        let insets = SafeArea {
            start: inset,
            end: inset,
            top: inset,
            bottom: inset,
        };
        resolve_toggle_layout(ToggleLayoutInput {
            track_size,
            thumb_size,
            insets: &insets,
            layout_direction: LayoutDirection::Ltr,
            checked: false,
        })
        .map_err(|error| ToggleProfileError::Allocation(density, error))?;
        Ok((track_size, thumb_size, insets))
    })?;
    let environment = input.environment;
    let density = environment.density_preference();
    let (track_size, thumb_size, insets) = modes.get(density).clone();
    if available.width < track_size.width || available.height < track_size.height {
        return Err(ToggleProfileError::ContainerBelowTrack);
    }
    Ok(ToggleProfileIr {
        schema_version: "0.1.0",
        density_preference: density,
        layout_direction: environment.layout_direction(),
        track_size,
        thumb_size,
        insets,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    profile: ToggleLayoutProfile,
    tokens: Value,
    environment: EnvironmentSnapshot,
    available_size: SurfaceSize,
}

pub fn resolve_toggle_profile_source(source: &str) -> Result<ToggleProfileIr, ToggleProfileError> {
    let document =
        resina_tokens::parse_token_document(source).map_err(ToggleProfileError::Parse)?;
    if !document.is_object() {
        return Err(ToggleProfileError::InvalidShape("root"));
    }
    if document
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(ToggleProfileError::UnsupportedVersion);
    }
    let request: Request = serde_json::from_value(document).map_err(ToggleProfileError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ToggleProfileError::UnsupportedVersion);
    }
    let tokens = resolve_token_document(&request.tokens).map_err(ToggleProfileError::Tokens)?;
    resolve_toggle_profile(ToggleProfileInput {
        profile: &request.profile,
        tokens: &tokens,
        environment: &request.environment,
        available_size: request.available_size,
    })
}
