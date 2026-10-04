use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{PhysicalBounds, SurfaceSize};
use serde::{Deserialize, Serialize};
use std::fmt;

pub struct ToggleLayoutInput<'a> {
    pub track_size: SurfaceSize,
    pub thumb_size: SurfaceSize,
    pub insets: &'a SafeArea,
    pub layout_direction: LayoutDirection,
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleLayoutIr {
    schema_version: &'static str,
    layout_direction: LayoutDirection,
    checked: bool,
    track_bounds: PhysicalBounds,
    off_thumb_bounds: PhysicalBounds,
    on_thumb_bounds: PhysicalBounds,
    thumb_bounds: PhysicalBounds,
}
impl ToggleLayoutIr {
    pub fn track_bounds(&self) -> PhysicalBounds {
        self.track_bounds
    }
    pub fn off_thumb_bounds(&self) -> PhysicalBounds {
        self.off_thumb_bounds
    }
    pub fn on_thumb_bounds(&self) -> PhysicalBounds {
        self.on_thumb_bounds
    }
    pub fn thumb_bounds(&self) -> PhysicalBounds {
        self.thumb_bounds
    }
    pub fn checked(&self) -> bool {
        self.checked
    }
    pub fn layout_direction(&self) -> LayoutDirection {
        self.layout_direction
    }
}

#[derive(Debug)]
pub enum ToggleLayoutError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidSize,
    InvalidInsets,
    DoesNotFit,
    NoTravel,
    NumericRange,
}
impl fmt::Display for ToggleLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "toggle layout parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid toggle layout request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidSize => f.write_str("track and thumb sizes must be finite and positive"),
            Self::InvalidInsets => f.write_str("insets must be finite and nonnegative"),
            Self::DoesNotFit => f.write_str("thumb must fit within the inset track"),
            Self::NoTravel => f.write_str("toggle requires distinct off/on travel endpoints"),
            Self::NumericRange => f.write_str("toggle layout exceeds representable arithmetic"),
        }
    }
}
impl std::error::Error for ToggleLayoutError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

fn subtract(left: f64, right: f64) -> Result<f64, ToggleLayoutError> {
    if right > left {
        return Err(ToggleLayoutError::DoesNotFit);
    }
    let result = left - right;
    if right > 0.0 && result == left {
        return Err(ToggleLayoutError::NumericRange);
    }
    Ok(result)
}

pub fn resolve_toggle_layout(
    input: ToggleLayoutInput<'_>,
) -> Result<ToggleLayoutIr, ToggleLayoutError> {
    let track = input.track_size;
    let thumb = input.thumb_size;
    if [track.width, track.height, thumb.width, thumb.height]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(ToggleLayoutError::InvalidSize);
    }
    let p = input.insets;
    if [p.start, p.end, p.top, p.bottom]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
    {
        return Err(ToggleLayoutError::InvalidInsets);
    }
    let interior_width = subtract(subtract(track.width, p.start)?, p.end)?;
    let interior_height = subtract(subtract(track.height, p.top)?, p.bottom)?;
    let travel = subtract(interior_width, thumb.width)?;
    let slack = subtract(interior_height, thumb.height)?;
    if travel <= 0.0 {
        return Err(ToggleLayoutError::NoTravel);
    }
    let half_slack = slack * 0.5;
    let y = p.top + half_slack;
    if !y.is_finite() || (slack > 0.0 && (half_slack == 0.0 || y == p.top)) {
        return Err(ToggleLayoutError::NumericRange);
    }
    let start_x = match input.layout_direction {
        LayoutDirection::Ltr => p.start,
        LayoutDirection::Rtl => subtract(subtract(track.width, p.start)?, thumb.width)?,
    };
    let end_x = match input.layout_direction {
        LayoutDirection::Ltr => subtract(subtract(track.width, p.end)?, thumb.width)?,
        LayoutDirection::Rtl => p.end,
    };
    if start_x == end_x {
        return Err(ToggleLayoutError::NumericRange);
    }
    let part = |x| PhysicalBounds {
        x,
        y,
        width: thumb.width,
        height: thumb.height,
    };
    let off = part(start_x);
    let on = part(end_x);
    for bounds in [off, on] {
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        if !right.is_finite() || !bottom.is_finite() || right <= bounds.x || bottom <= bounds.y {
            return Err(ToggleLayoutError::NumericRange);
        }
        if right > track.width || bottom > track.height {
            return Err(ToggleLayoutError::DoesNotFit);
        }
    }
    Ok(ToggleLayoutIr {
        schema_version: "0.1.0",
        layout_direction: input.layout_direction,
        checked: input.checked,
        track_bounds: PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: track.width,
            height: track.height,
        },
        off_thumb_bounds: off,
        on_thumb_bounds: on,
        thumb_bounds: if input.checked { on } else { off },
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    track_size: SurfaceSize,
    thumb_size: SurfaceSize,
    insets: SafeArea,
    layout_direction: LayoutDirection,
    checked: bool,
}
pub fn resolve_toggle_layout_source(source: &str) -> Result<ToggleLayoutIr, ToggleLayoutError> {
    let document = resina_tokens::parse_token_document(source).map_err(ToggleLayoutError::Parse)?;
    let request: Request = serde_json::from_value(document).map_err(ToggleLayoutError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(ToggleLayoutError::UnsupportedVersion);
    }
    resolve_toggle_layout(ToggleLayoutInput {
        track_size: request.track_size,
        thumb_size: request.thumb_size,
        insets: &request.insets,
        layout_direction: request.layout_direction,
        checked: request.checked,
    })
}
