use serde::de::Error;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

fn member<'a>(source: &'a str, path: &[&str]) -> Result<Option<&'a RawValue>, serde_json::Error> {
    let mut value: &RawValue = serde_json::from_str(source)?;
    for name in path {
        let object: BTreeMap<String, &RawValue> = serde_json::from_str(value.get())?;
        let Some(next) = object.get(*name) else {
            return Ok(None);
        };
        value = next;
    }
    Ok(Some(value))
}

pub(crate) fn validate_pigment_source(
    source: &str,
    path: &[&str],
) -> Result<(), serde_json::Error> {
    let Some(profiles) = member(source, path)? else {
        return Ok(());
    };
    for family in ["cast", "frost", "elastomer", "gel"] {
        for coefficient in ["sideShade", "highlightLift"] {
            let Some(value) = member(profiles.get(), &["profiles", family, coefficient])? else {
                continue;
            };
            validate_nonnegative_coefficient(value.get(), &format!("{family}.{coefficient}"))?;
        }
    }
    Ok(())
}

/// Validates the original ambient and interaction strength sources of a light channels object,
/// including channels that policy or phase later suppresses.
pub(crate) fn validate_light_channel_source(
    source: &str,
    path: &[&str],
) -> Result<(), serde_json::Error> {
    let Some(channels) = member(source, path)? else {
        return Ok(());
    };
    if let Some(ambient) = member(channels.get(), &["ambient"])?
        && ambient.get() != "null"
    {
        validate_nonnegative_coefficient(ambient.get(), "ambient")?;
    }
    if let Some(interaction) = member(channels.get(), &["interaction"])?
        && interaction.get() != "null"
        && let Some(strength) = member(interaction.get(), &["strength"])?
    {
        validate_nonnegative_coefficient(strength.get(), "interaction strength")?;
    }
    Ok(())
}

pub(crate) fn validate_nonnegative_coefficient(
    literal: &str,
    field: &str,
) -> Result<(), serde_json::Error> {
    let numeric: f64 = serde_json::from_str(literal)?;
    let nonzero = literal
        .bytes()
        .take_while(|byte| !matches!(byte, b'e' | b'E'))
        .any(|byte| matches!(byte, b'1'..=b'9'));
    if numeric == 0.0 && literal.starts_with('-') && nonzero {
        return Err(serde_json::Error::custom(format!(
            "{field} must be nonnegative before numeric rounding"
        )));
    }
    Ok(())
}
