use crate::{
    SpatialBinding,
    command_label::extent_sum,
    profile_dimension::{Bound, DimensionFault, px_dimension},
    resolve_minimum_hit_target,
};
use resina_environment::{DensityPreference, EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{ComponentLayoutProfile, SizeTokenPaths, SpatialRole, SurfaceSize};
use resina_tokens::{DocumentError, ResolvedToken, resolve_token_document};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

pub struct CommandLayoutInput<'a> {
    pub profile: &'a ComponentLayoutProfile,
    pub tokens: &'a BTreeMap<String, ResolvedToken>,
    pub space: &'a SpatialBinding,
    pub environment: &'a EnvironmentSnapshot,
    pub available_size: SurfaceSize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLayoutIr {
    schema_version: &'static str,
    density_preference: DensityPreference,
    layout_direction: LayoutDirection,
    minimum_size: SurfaceSize,
    maximum_size: SurfaceSize,
    padding: SafeArea,
    target_minimum: SurfaceSize,
}

impl CommandLayoutIr {
    pub fn minimum_size(&self) -> SurfaceSize {
        self.minimum_size
    }
    pub fn maximum_size(&self) -> SurfaceSize {
        self.maximum_size
    }
    pub fn padding(&self) -> &SafeArea {
        &self.padding
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
    pub fn target_minimum(&self) -> SurfaceSize {
        self.target_minimum
    }
}

#[derive(Debug)]
pub enum CommandLayoutError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    InvalidShape(&'static str),
    UnsupportedVersion,
    Tokens(Vec<DocumentError>),
    MissingExtentToken(String),
    ExtentTokenType { path: String, found: String },
    InvalidExtentToken { path: String, detail: String },
    ExtentTokenUnit(String),
    NonpositiveExtent(String),
    ExtentOrder(DensityPreference),
    InvalidAvailableSize,
    UnsupportedInsetUnit(SpatialRole),
    NumericRange,
    ContainerBelowMinimum,
    NoLabelExtent,
}

impl fmt::Display for CommandLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "command layout parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid command layout request: {error}"),
            Self::InvalidShape(field) => {
                write!(f, "command layout request {field} must be a JSON object")
            }
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::Tokens(errors) => {
                f.write_str("command layout token resolution failed")?;
                for error in errors {
                    write!(
                        f,
                        "
  {error}"
                    )?;
                }
                Ok(())
            }
            Self::MissingExtentToken(path) => write!(f, "missing extent token {path}"),
            Self::ExtentTokenType { path, found } => {
                write!(f, "extent token {path} must be dimension, found {found}")
            }
            Self::InvalidExtentToken { path, detail } => {
                write!(f, "invalid extent token {path}: {detail}")
            }
            Self::ExtentTokenUnit(path) => write!(f, "extent token {path} must use px"),
            Self::NonpositiveExtent(path) => {
                write!(f, "extent token {path} must be finite and positive")
            }
            Self::ExtentOrder(density) => {
                write!(f, "{density:?} minimumSize must not exceed maximumSize")
            }
            Self::InvalidAvailableSize => f.write_str("availableSize must be finite and positive"),
            Self::UnsupportedInsetUnit(role) => {
                write!(f, "{role:?} must resolve to a px dimension")
            }
            Self::NumericRange => f.write_str("text-scaled maximum size is not finite"),
            Self::ContainerBelowMinimum => {
                f.write_str("availableSize is smaller than the profile minimum")
            }
            Self::NoLabelExtent => f.write_str("profile insets leave no label extent"),
        }
    }
}

impl std::error::Error for CommandLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

fn extent_token(
    path: &str,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<f64, CommandLayoutError> {
    px_dimension(path, tokens, Bound::Positive).map_err(|fault| {
        let path = path.to_owned();
        match fault {
            DimensionFault::Missing => CommandLayoutError::MissingExtentToken(path),
            DimensionFault::Type(found) => CommandLayoutError::ExtentTokenType { path, found },
            DimensionFault::Invalid(detail) => {
                CommandLayoutError::InvalidExtentToken { path, detail }
            }
            DimensionFault::Unit => CommandLayoutError::ExtentTokenUnit(path),
            DimensionFault::Range => CommandLayoutError::NonpositiveExtent(path),
        }
    })
}

fn extent_size(
    paths: &SizeTokenPaths,
    tokens: &BTreeMap<String, ResolvedToken>,
) -> Result<SurfaceSize, CommandLayoutError> {
    Ok(SurfaceSize {
        width: extent_token(&paths.width, tokens)?,
        height: extent_token(&paths.height, tokens)?,
    })
}

pub fn resolve_command_layout(
    input: CommandLayoutInput<'_>,
) -> Result<CommandLayoutIr, CommandLayoutError> {
    let available = input.available_size;
    if !(available.width.is_finite()
        && available.height.is_finite()
        && available.width > 0.0
        && available.height > 0.0)
    {
        return Err(CommandLayoutError::InvalidAvailableSize);
    }
    let extents = input.profile.extents().try_map(|density, extent| {
        let minimum = extent_size(&extent.minimum_size, input.tokens)?;
        let maximum = extent_size(&extent.maximum_size, input.tokens)?;
        if minimum.width > maximum.width || minimum.height > maximum.height {
            return Err(CommandLayoutError::ExtentOrder(density));
        }
        Ok((minimum, maximum))
    })?;
    let environment = input.environment;
    let density = environment.density_preference();
    let (minimum, authored_maximum) = *extents.get(density);
    let roles = input.profile.padding();
    let inset = |role| -> Result<f64, CommandLayoutError> {
        let value = input.space.value_for(role, density);
        match (value["unit"].as_str(), value["value"].as_f64()) {
            (Some("px"), Some(distance)) => Ok(distance),
            _ => Err(CommandLayoutError::UnsupportedInsetUnit(role)),
        }
    };
    let padding = SafeArea {
        start: inset(roles.start)?,
        end: inset(roles.end)?,
        top: inset(roles.top)?,
        bottom: inset(roles.bottom)?,
    };
    let factor = environment.text_scale().max(1.0);
    let scaled = SurfaceSize {
        width: authored_maximum.width * factor,
        height: authored_maximum.height * factor,
    };
    if !scaled.width.is_finite() || !scaled.height.is_finite() {
        return Err(CommandLayoutError::NumericRange);
    }
    if available.width < minimum.width || available.height < minimum.height {
        return Err(CommandLayoutError::ContainerBelowMinimum);
    }
    let maximum = SurfaceSize {
        width: scaled.width.min(available.width),
        height: scaled.height.min(available.height),
    };
    let horizontal = extent_sum(padding.start, padding.end);
    let vertical = extent_sum(padding.top, padding.bottom);
    if !horizontal.is_some_and(|sum| sum < maximum.width)
        || !vertical.is_some_and(|sum| sum < maximum.height)
    {
        return Err(CommandLayoutError::NoLabelExtent);
    }
    let floor = resolve_minimum_hit_target(environment);
    Ok(CommandLayoutIr {
        schema_version: "0.1.0",
        density_preference: density,
        layout_direction: environment.layout_direction(),
        minimum_size: minimum,
        maximum_size: maximum,
        padding,
        target_minimum: SurfaceSize {
            width: f64::from(floor.minimum_width()),
            height: f64::from(floor.minimum_height()),
        },
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    profile: ComponentLayoutProfile,
    tokens: Value,
    spatial_binding: SpatialBinding,
    environment: EnvironmentSnapshot,
    available_size: SurfaceSize,
}

pub fn resolve_command_layout_source(source: &str) -> Result<CommandLayoutIr, CommandLayoutError> {
    let document =
        resina_tokens::parse_token_document(source).map_err(CommandLayoutError::Parse)?;
    if !document.is_object() {
        return Err(CommandLayoutError::InvalidShape("root"));
    }
    if document
        .get("schemaVersion")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|version| version != "0.1.0")
    {
        return Err(CommandLayoutError::UnsupportedVersion);
    }
    let request: Request = serde_json::from_value(document).map_err(CommandLayoutError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(CommandLayoutError::UnsupportedVersion);
    }
    let tokens = resolve_token_document(&request.tokens).map_err(CommandLayoutError::Tokens)?;
    resolve_command_layout(CommandLayoutInput {
        profile: &request.profile,
        tokens: &tokens,
        space: &request.spatial_binding,
        environment: &request.environment,
        available_size: request.available_size,
    })
}
