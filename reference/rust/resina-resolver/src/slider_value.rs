use resina_model::SliderValue;
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SliderValueIr {
    #[serde(flatten)]
    value: SliderValue,
    progress: f64,
}
impl SliderValueIr {
    pub fn value(&self) -> &SliderValue {
        &self.value
    }
    pub fn progress(&self) -> f64 {
        self.progress
    }
}
#[derive(Debug)]
pub enum SliderValueError {
    Parse(serde_json::Error),
    Request(serde_json::Error),
    NumericRange,
    IntegerPrecision { field: &'static str },
}
impl fmt::Display for SliderValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "slider value parse failed: {error}"),
            Self::Request(error) => write!(f, "slider value request failed: {error}"),
            Self::NumericRange => f.write_str("slider progress exceeds representable arithmetic"),
            Self::IntegerPrecision { field } => write!(
                f,
                "slider {field} integer exceeds exact binary64 representation"
            ),
        }
    }
}
impl std::error::Error for SliderValueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) | Self::Request(e) => Some(e),
            Self::NumericRange | Self::IntegerPrecision { .. } => None,
        }
    }
}
pub fn resolve_slider_value(value: &SliderValue) -> Result<SliderValueIr, SliderValueError> {
    let min = value.minimum();
    let max = value.maximum();
    let current = value.value();
    let progress = if current == min {
        0.0
    } else if current == max {
        1.0
    } else {
        let span = max - min;
        let low = current - min;
        let high = max - current;
        let (low, high, span) = if span.is_finite() {
            (low, high, span)
        } else {
            // Subtract before halving when possible to retain near-endpoint distances.
            let low = if low.is_finite() {
                low * 0.5
            } else {
                current * 0.5 - min * 0.5
            };
            let high = if high.is_finite() {
                high * 0.5
            } else {
                max * 0.5 - current * 0.5
            };
            (low, high, max * 0.5 - min * 0.5)
        };
        let result = if low <= high {
            low / span
        } else {
            1.0 - high / span
        };
        if !result.is_finite() || result <= 0.0 || result >= 1.0 {
            return Err(SliderValueError::NumericRange);
        }
        result
    };
    Ok(SliderValueIr {
        value: *value,
        progress,
    })
}
#[derive(Deserialize)]
struct SliderValueSource<'a> {
    #[serde(borrow)]
    minimum: &'a serde_json::value::RawValue,
    #[serde(borrow)]
    maximum: &'a serde_json::value::RawValue,
    #[serde(borrow)]
    value: &'a serde_json::value::RawValue,
}
pub fn resolve_slider_value_source(source: &str) -> Result<SliderValueIr, SliderValueError> {
    let document = parse_token_document(source).map_err(SliderValueError::Parse)?;
    let value: SliderValue = serde_json::from_value(document).map_err(SliderValueError::Request)?;
    let raw: SliderValueSource<'_> =
        serde_json::from_str(source).map_err(SliderValueError::Request)?;
    for (field, raw, number) in [
        ("minimum", raw.minimum, value.minimum()),
        ("maximum", raw.maximum, value.maximum()),
        ("value", raw.value, value.value()),
    ] {
        let literal = raw.get();
        if !literal.contains(['.', 'e', 'E']) && format!("{number:.0}") != literal {
            return Err(SliderValueError::IntegerPrecision { field });
        }
    }
    resolve_slider_value(&value)
}
