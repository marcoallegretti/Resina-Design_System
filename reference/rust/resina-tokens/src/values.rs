use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueErrorKind {
    UnsupportedType,
    InvalidValue,
    MissingProperty,
    UnknownProperty,
    OutOfRange,
    DepthExceeded,
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

pub fn validate_resolved_value(kind: &str, value: &Value) -> Result<(), ValueError> {
    validate(kind, value, "#/$value")
}

fn validate_composite(kind: &str, value: &Value, path: &str) -> Result<(), ValueError> {
    match kind {
        "strokeStyle" => validate_stroke_style(value, path),
        "border" => validate_fields(
            value,
            path,
            &[
                ("color", "color"),
                ("width", "dimension"),
                ("style", "strokeStyle"),
            ],
        ),
        "transition" => validate_fields(
            value,
            path,
            &[
                ("duration", "duration"),
                ("delay", "duration"),
                ("timingFunction", "cubicBezier"),
            ],
        ),
        "typography" => validate_fields(
            value,
            path,
            &[
                ("fontFamily", "fontFamily"),
                ("fontSize", "dimension"),
                ("fontWeight", "fontWeight"),
                ("letterSpacing", "dimension"),
                ("lineHeight", "number"),
            ],
        ),
        "shadow" => validate_shadow(value, path, 0),
        "gradient" => validate_gradient(value, path, 0),
        _ => Err(error(ValueErrorKind::UnsupportedType, path)),
    }
}

const MAX_COMPOSITE_DEPTH: usize = 64;

fn validate_shadow(value: &Value, path: &str, depth: usize) -> Result<(), ValueError> {
    if depth >= MAX_COMPOSITE_DEPTH {
        return Err(error(ValueErrorKind::DepthExceeded, path));
    }
    if let Some(items) = value.as_array() {
        require(!items.is_empty(), path)?;
        for (index, item) in items.iter().enumerate() {
            validate_shadow(item, &format!("{path}/{index}"), depth + 1)?;
        }
        return Ok(());
    }
    let required = ["color", "offsetX", "offsetY", "blur", "spread"];
    let properties = object(
        value,
        path,
        &required,
        &["color", "offsetX", "offsetY", "blur", "spread", "inset"],
    )?;
    validate("color", &properties["color"], &format!("{path}/color"))?;
    for field in ["offsetX", "offsetY", "blur", "spread"] {
        validate("dimension", &properties[field], &format!("{path}/{field}"))?;
    }
    if let Some(inset) = properties.get("inset") {
        require(inset.is_boolean(), &format!("{path}/inset"))?;
    }
    Ok(())
}

fn validate_gradient(value: &Value, path: &str, depth: usize) -> Result<(), ValueError> {
    if depth >= MAX_COMPOSITE_DEPTH {
        return Err(error(ValueErrorKind::DepthExceeded, path));
    }
    let stops = value
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| error(ValueErrorKind::InvalidValue, path))?;
    for (index, stop) in stops.iter().enumerate() {
        let location = format!("{path}/{index}");
        if stop.is_array() {
            validate_gradient(stop, &location, depth + 1)?;
            continue;
        }
        let properties = object(
            stop,
            &location,
            &["color", "position"],
            &["color", "position"],
        )?;
        validate("color", &properties["color"], &format!("{location}/color"))?;
        validate(
            "number",
            &properties["position"],
            &format!("{location}/position"),
        )?;
    }
    Ok(())
}

fn validate_fields(value: &Value, path: &str, fields: &[(&str, &str)]) -> Result<(), ValueError> {
    let names: Vec<&str> = fields.iter().map(|(name, _)| *name).collect();
    let properties = object(value, path, &names, &names)?;
    for (name, kind) in fields {
        let location = format!("{path}/{name}");
        validate(kind, &properties[*name], &location)?;
    }
    Ok(())
}

fn validate_stroke_style(value: &Value, path: &str) -> Result<(), ValueError> {
    if let Some(name) = value.as_str() {
        return require(
            [
                "solid", "dashed", "dotted", "double", "groove", "ridge", "outset", "inset",
            ]
            .contains(&name),
            path,
        );
    }
    let style = object(
        value,
        path,
        &["dashArray", "lineCap"],
        &["dashArray", "lineCap"],
    )?;
    let dashes = style["dashArray"]
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| error(ValueErrorKind::InvalidValue, &format!("{path}/dashArray")))?;
    for (index, dash) in dashes.iter().enumerate() {
        validate("dimension", dash, &format!("{path}/dashArray/{index}"))?;
    }
    require(
        style["lineCap"]
            .as_str()
            .is_some_and(|cap| ["round", "butt", "square"].contains(&cap)),
        &format!("{path}/lineCap"),
    )
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
        _ => validate_composite(kind, value, path),
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
            let result =
                validate_resolved_value(vector["type"].as_str().unwrap(), &vector["value"]);
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

    #[test]
    fn fixed_composite_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/fixed-composite-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                validate_resolved_value(vector["type"].as_str().unwrap(), &vector["value"]);
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

    #[test]
    fn array_composite_conformance_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/tokens/array-composite-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result =
                validate_resolved_value(vector["type"].as_str().unwrap(), &vector["value"]);
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
