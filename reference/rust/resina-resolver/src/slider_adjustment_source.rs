use crate::{
    SliderAdjustment, SliderAdjustmentError, SliderAdjustmentInput, SliderAdjustmentIr,
    SliderValueError, resolve_slider_adjustment, resolve_slider_value_source,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug)]
pub enum SliderAdjustmentSourceError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    UnsupportedVersion,
    InvalidObject(&'static str),
    IntegerPrecision,
    Value(SliderValueError),
    Adjustment(SliderAdjustmentError),
}

impl fmt::Display for SliderAdjustmentSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "slider adjustment parse failed: {error}"),
            Self::Request(error) => write!(f, "invalid slider adjustment request: {error}"),
            Self::UnsupportedVersion => f.write_str("schemaVersion must be 0.1.0"),
            Self::InvalidObject(field) => {
                write!(f, "slider adjustment request {field} must be a JSON object")
            }
            Self::IntegerPrecision => {
                f.write_str("slider adjustment integer exceeds exact binary64 representation")
            }
            Self::Value(error) => write!(f, "slider adjustment current value: {error}"),
            Self::Adjustment(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SliderAdjustmentSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) | Self::Request(error) => Some(error),
            Self::Value(error) => Some(error),
            Self::Adjustment(error) => Some(error),
            Self::UnsupportedVersion | Self::InvalidObject(_) | Self::IntegerPrecision => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request<'a> {
    schema_version: String,
    #[serde(borrow)]
    current: &'a RawValue,
    enabled: bool,
    read_only: bool,
    #[serde(borrow)]
    adjustment: &'a RawValue,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum AdjustmentSource {
    SetValue { value: f64 },
    Increase { amount: f64 },
    Decrease { amount: f64 },
    Minimum {},
    Maximum {},
}

fn number(source: &str, field: &str, value: f64) -> Result<f64, SliderAdjustmentSourceError> {
    let fields: std::collections::BTreeMap<String, &RawValue> =
        serde_json::from_str(source).map_err(SliderAdjustmentSourceError::Request)?;
    let literal = fields[field].get();
    if !literal.contains(['.', 'e', 'E']) && format!("{value:.0}") != literal {
        return Err(SliderAdjustmentSourceError::IntegerPrecision);
    }
    Ok(value)
}

pub fn resolve_slider_adjustment_source(
    source: &str,
) -> Result<SliderAdjustmentIr, SliderAdjustmentSourceError> {
    let document = parse_token_document(source).map_err(SliderAdjustmentSourceError::Parse)?;
    if !document.is_object() {
        return Err(SliderAdjustmentSourceError::InvalidObject("root"));
    }
    for field in ["current", "adjustment"] {
        if !document[field].is_object() {
            return Err(SliderAdjustmentSourceError::InvalidObject(field));
        }
    }
    let request: Request<'_> =
        serde_json::from_str(source).map_err(SliderAdjustmentSourceError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(SliderAdjustmentSourceError::UnsupportedVersion);
    }
    let current = resolve_slider_value_source(request.current.get())
        .map_err(SliderAdjustmentSourceError::Value)?;
    let adjustment: AdjustmentSource = serde_json::from_str(request.adjustment.get())
        .map_err(SliderAdjustmentSourceError::Request)?;
    let source = request.adjustment.get();
    let adjustment = match adjustment {
        AdjustmentSource::SetValue { value } => {
            SliderAdjustment::SetValue(number(source, "value", value)?)
        }
        AdjustmentSource::Increase { amount } => {
            SliderAdjustment::Increase(number(source, "amount", amount)?)
        }
        AdjustmentSource::Decrease { amount } => {
            SliderAdjustment::Decrease(number(source, "amount", amount)?)
        }
        AdjustmentSource::Minimum {} => SliderAdjustment::Minimum,
        AdjustmentSource::Maximum {} => SliderAdjustment::Maximum,
    };
    resolve_slider_adjustment(SliderAdjustmentInput {
        current: &current,
        enabled: request.enabled,
        read_only: request.read_only,
        adjustment,
    })
    .map_err(SliderAdjustmentSourceError::Adjustment)
}
