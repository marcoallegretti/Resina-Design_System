use resina_tokens::{ResolvedToken, validate_resolved_value};
use std::collections::BTreeMap;

pub(crate) enum DimensionFault {
    Missing,
    Type(String),
    Invalid(String),
    Unit,
    Range,
}

#[derive(Clone, Copy)]
pub(crate) enum Bound {
    Positive,
    Nonnegative,
}

pub(crate) fn px_dimension(
    path: &str,
    tokens: &BTreeMap<String, ResolvedToken>,
    bound: Bound,
) -> Result<f64, DimensionFault> {
    let token = tokens.get(path).ok_or(DimensionFault::Missing)?;
    if token.token_type != "dimension" {
        return Err(DimensionFault::Type(token.token_type.clone()));
    }
    validate_resolved_value("dimension", &token.value)
        .map_err(|error| DimensionFault::Invalid(error.to_string()))?;
    if token.value["unit"] != "px" {
        return Err(DimensionFault::Unit);
    }
    match (token.value["value"].as_f64(), bound) {
        (Some(value), Bound::Positive) if value.is_finite() && value > 0.0 => Ok(value),
        (Some(value), Bound::Nonnegative) if value.is_finite() && value >= 0.0 => Ok(value),
        _ => Err(DimensionFault::Range),
    }
}
