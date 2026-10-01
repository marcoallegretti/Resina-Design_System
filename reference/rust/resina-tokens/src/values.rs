use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueErrorKind {
    UnsupportedType,
    InvalidValue,
    MissingProperty,
    UnknownProperty,
    OutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError {
    pub kind: ValueErrorKind,
    pub location: String,
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?} at {}", self.kind, self.location)
    }
}

impl std::error::Error for ValueError {}

pub fn validate_resolved_primitive_value(kind: &str, value: &Value) -> Result<(), ValueError> {
    validate(kind, value, "#/$value")
}

fn validate(kind: &str, value: &Value, path: &str) -> Result<(), ValueError> {
    match kind {
        "number" => require(value.is_number(), path),
        "dimension" => validate_measure(value, path, &["px", "rem"]),
        "duration" => validate_measure(value, path, &["ms", "s"]),
        "fontFamily" => match value {
            Value::String(_) => Ok(()),
            Value::Array(items) if !items.is_empty() => {
                for (index, item) in items.iter().enumerate() {
                    require(item.is_string(), &format!("{path}/{index}"))?;
                }
                Ok(())
            }
            _ => invalid(path),
        },
        "fontWeight" => {
            if let Some(number) = value.as_f64() {
                range(number, 1.0, 1000.0, path)
            } else {
                require(
                    value.as_str().is_some_and(|name| {
                        [
                            "thin",
                            "hairline",
                            "extra-light",
                            "ultra-light",
                            "light",
                            "normal",
                            "regular",
                            "book",
                            "medium",
                            "semi-bold",
                            "demi-bold",
                            "bold",
                            "extra-bold",
                            "ultra-bold",
                            "black",
                            "heavy",
                            "extra-black",
                            "ultra-black",
                        ]
                        .contains(&name)
                    }),
                    path,
                )
            }
        }
        "cubicBezier" => {
            let Some(points) = value.as_array().filter(|points| points.len() == 4) else {
                return invalid(path);
            };
            for (index, point) in points.iter().enumerate() {
                let position = format!("{path}/{index}");
                let Some(number) = point.as_f64() else {
                    return invalid(&position);
                };
                if index == 0 || index == 2 {
                    range(number, 0.0, 1.0, &position)?;
                }
            }
            Ok(())
        }
        "color" => validate_color(value, path),
        _ => Err(error(ValueErrorKind::UnsupportedType, path)),
    }
}

fn validate_measure(value: &Value, path: &str, units: &[&str]) -> Result<(), ValueError> {
    let object = object(value, path, &["value", "unit"], &["value", "unit"])?;
    require(object["value"].is_number(), &format!("{path}/value"))?;
    require(
        object["unit"]
            .as_str()
            .is_some_and(|unit| units.contains(&unit)),
        &format!("{path}/unit"),
    )
}

fn validate_color(value: &Value, path: &str) -> Result<(), ValueError> {
    let color = object(
        value,
        path,
        &["colorSpace", "components"],
        &["colorSpace", "components", "alpha", "hex"],
    )?;
    let space = color["colorSpace"]
        .as_str()
        .ok_or_else(|| error(ValueErrorKind::InvalidValue, &format!("{path}/colorSpace")))?;
    let components = color["components"]
        .as_array()
        .filter(|items| items.len() == 3)
        .ok_or_else(|| error(ValueErrorKind::InvalidValue, &format!("{path}/components")))?;
    let ranges: [(f64, Option<f64>); 3] = match space {
        "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020"
        | "xyz-d65" | "xyz-d50" => [(0.0, Some(1.0)); 3],
        "hsl" | "hwb" => [(0.0, Some(360.0)), (0.0, Some(100.0)), (0.0, Some(100.0))],
        "lab" => [
            (0.0, Some(100.0)),
            (f64::NEG_INFINITY, None),
            (f64::NEG_INFINITY, None),
        ],
        "lch" => [(0.0, Some(100.0)), (0.0, None), (0.0, Some(360.0))],
        "oklab" => [
            (0.0, Some(1.0)),
            (f64::NEG_INFINITY, None),
            (f64::NEG_INFINITY, None),
        ],
        "oklch" => [(0.0, Some(1.0)), (0.0, None), (0.0, Some(360.0))],
        _ => return invalid(&format!("{path}/colorSpace")),
    };
    for (index, component) in components.iter().enumerate() {
        if component == "none" {
            continue;
        }
        let location = format!("{path}/components/{index}");
        let Some(number) = component.as_f64() else {
            return invalid(&location);
        };
        let (min, max) = ranges[index];
        let exclusive_max = (matches!(space, "hsl" | "hwb") && index == 0)
            || (matches!(space, "lch" | "oklch") && index == 2);
        if number < min
            || max.is_some_and(|max| {
                if exclusive_max {
                    number >= max
                } else {
                    number > max
                }
            })
        {
            return Err(error(ValueErrorKind::OutOfRange, &location));
        }
    }
    if let Some(alpha) = color.get("alpha") {
        let location = format!("{path}/alpha");
        let Some(number) = alpha.as_f64() else {
            return invalid(&location);
        };
        range(number, 0.0, 1.0, &location)?;
    }
    if let Some(hex) = color.get("hex") {
        let location = format!("{path}/hex");
        let Some(text) = hex.as_str() else {
            return invalid(&location);
        };
        require(
            text.len() == 7
                && text.starts_with('#')
                && text[1..].bytes().all(|byte| byte.is_ascii_hexdigit()),
            &location,
        )?;
    }
    Ok(())
}

fn object<'a>(
    value: &'a Value,
    path: &str,
    required: &[&str],
    allowed: &[&str],
) -> Result<&'a Map<String, Value>, ValueError> {
    let Some(object) = value.as_object() else {
        return invalid(path);
    };
    for key in required {
        if !object.contains_key(*key) {
            return Err(error(
                ValueErrorKind::MissingProperty,
                &format!("{path}/{key}"),
            ));
        }
    }
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(error(
                ValueErrorKind::UnknownProperty,
                &format!("{path}/{}", crate::escape_pointer_segment(key)),
            ));
        }
    }
    Ok(object)
}

fn range(number: f64, minimum: f64, maximum: f64, path: &str) -> Result<(), ValueError> {
    if number < minimum || number > maximum {
        Err(error(ValueErrorKind::OutOfRange, path))
    } else {
        Ok(())
    }
}

fn require(condition: bool, path: &str) -> Result<(), ValueError> {
    if condition { Ok(()) } else { invalid(path) }
}

fn invalid<T>(path: &str) -> Result<T, ValueError> {
    Err(error(ValueErrorKind::InvalidValue, path))
}

fn error(kind: ValueErrorKind, path: &str) -> ValueError {
    ValueError {
        kind,
        location: path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_value_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/primitive-value-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = validate_resolved_primitive_value(
                vector["type"].as_str().unwrap(),
                &vector["value"],
            );
            if let Some(expected) = vector.get("error") {
                let error = result.unwrap_err();
                assert_eq!(
                    format!("{:?}", error.kind),
                    expected.as_str().unwrap(),
                    "{}: {error}",
                    vector["name"]
                );
            } else {
                assert!(result.is_ok(), "{}: {result:?}", vector["name"]);
            }
        }
    }
}
