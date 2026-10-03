use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerGeometryError {
    InvalidWidth,
    InvalidHeight,
    InvalidRadius(&'static str),
}

impl fmt::Display for CornerGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWidth => formatter.write_str("width must be finite and nonnegative"),
            Self::InvalidHeight => formatter.write_str("height must be finite and nonnegative"),
            Self::InvalidRadius(name) => {
                write!(formatter, "{name} must be finite and nonnegative")
            }
        }
    }
}

impl std::error::Error for CornerGeometryError {}

pub fn normalize_corner_radii(
    size: SurfaceSize,
    radii: LogicalCornerRadii,
) -> Result<LogicalCornerRadii, CornerGeometryError> {
    if !size.width.is_finite() || size.width < 0.0 {
        return Err(CornerGeometryError::InvalidWidth);
    }
    if !size.height.is_finite() || size.height < 0.0 {
        return Err(CornerGeometryError::InvalidHeight);
    }
    for (name, value) in [
        ("topStart.x", radii.top_start.x),
        ("topStart.y", radii.top_start.y),
        ("topEnd.x", radii.top_end.x),
        ("topEnd.y", radii.top_end.y),
        ("bottomEnd.x", radii.bottom_end.x),
        ("bottomEnd.y", radii.bottom_end.y),
        ("bottomStart.x", radii.bottom_start.x),
        ("bottomStart.y", radii.bottom_start.y),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(CornerGeometryError::InvalidRadius(name));
        }
    }

    let scale = [
        side_scale(size.width, radii.top_start.x, radii.top_end.x),
        side_scale(size.width, radii.bottom_start.x, radii.bottom_end.x),
        side_scale(size.height, radii.top_start.y, radii.bottom_start.y),
        side_scale(size.height, radii.top_end.y, radii.bottom_end.y),
    ]
    .into_iter()
    .min_by(|left, right| {
        left.factor
            .total_cmp(&right.factor)
            .then_with(|| left.log_factor.total_cmp(&right.log_factor))
    })
    .unwrap();

    Ok(LogicalCornerRadii {
        top_start: scaled(radii.top_start, scale),
        top_end: scaled(radii.top_end, scale),
        bottom_end: scaled(radii.bottom_end, scale),
        bottom_start: scaled(radii.bottom_start, scale),
    })
}

#[derive(Clone, Copy)]
struct Scale {
    factor: f64,
    log_factor: f64,
    limit: f64,
    largest: f64,
    denominator: f64,
}

fn side_scale(limit: f64, first: f64, second: f64) -> Scale {
    if first <= limit && second <= limit - first {
        return Scale {
            factor: 1.0,
            log_factor: 0.0,
            limit: 1.0,
            largest: 1.0,
            denominator: 1.0,
        };
    }
    let largest = first.max(second);
    let denominator = first / largest + second / largest;
    Scale {
        factor: (limit / largest) / denominator,
        log_factor: if limit == 0.0 {
            f64::NEG_INFINITY
        } else {
            limit.ln() - largest.ln() - denominator.ln()
        },
        limit,
        largest,
        denominator,
    }
}

fn scaled(radius: CornerRadius, scale: Scale) -> CornerRadius {
    let apply = |value: f64| {
        if scale.factor > 0.0 || scale.limit == 0.0 {
            value * scale.factor
        } else {
            (value / scale.largest * scale.limit) / scale.denominator
        }
    };
    CornerRadius {
        x: apply(radius.x),
        y: apply(radius.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn corner_radius_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/corner-radius-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let size: SurfaceSize = serde_json::from_value(vector["size"].clone()).unwrap();
            let radii: LogicalCornerRadii =
                serde_json::from_value(vector["radii"].clone()).unwrap();
            let actual = normalize_corner_radii(size, radii);
            if let Some(expected) = vector.get("expected") {
                let expected: LogicalCornerRadii =
                    serde_json::from_value(expected.clone()).unwrap();
                let actual = actual.unwrap();
                for (corner, got, want) in [
                    ("topStart", actual.top_start, expected.top_start),
                    ("topEnd", actual.top_end, expected.top_end),
                    ("bottomEnd", actual.bottom_end, expected.bottom_end),
                    ("bottomStart", actual.bottom_start, expected.bottom_start),
                ] {
                    for (axis, actual, expected) in [("x", got.x, want.x), ("y", got.y, want.y)] {
                        let tolerance = 1e-12 * expected.abs();
                        assert!(
                            (actual - expected).abs() <= tolerance,
                            "{}: {corner}.{axis}: expected {expected}, got {actual}",
                            vector["name"]
                        );
                    }
                }
            } else {
                assert_eq!(
                    actual.unwrap_err().to_string(),
                    vector["error"].as_str().unwrap(),
                    "{}",
                    vector["name"]
                );
            }
        }
    }

    #[test]
    fn rejects_nonfinite_input() {
        let radii = LogicalCornerRadii {
            top_start: CornerRadius { x: 1.0, y: 1.0 },
            top_end: CornerRadius { x: 1.0, y: 1.0 },
            bottom_end: CornerRadius { x: 1.0, y: 1.0 },
            bottom_start: CornerRadius { x: 1.0, y: 1.0 },
        };
        assert_eq!(
            normalize_corner_radii(
                SurfaceSize {
                    width: f64::NAN,
                    height: 1.0
                },
                radii
            ),
            Err(CornerGeometryError::InvalidWidth)
        );
        let mut invalid = radii;
        invalid.bottom_end.y = f64::INFINITY;
        assert_eq!(
            normalize_corner_radii(
                SurfaceSize {
                    width: 1.0,
                    height: 1.0
                },
                invalid
            ),
            Err(CornerGeometryError::InvalidRadius("bottomEnd.y"))
        );
    }

    #[test]
    fn overflow_safe_side_sums() {
        let huge = f64::MAX;
        let radii = LogicalCornerRadii {
            top_start: CornerRadius { x: huge, y: 0.0 },
            top_end: CornerRadius { x: huge, y: 0.0 },
            bottom_end: CornerRadius { x: 0.0, y: 0.0 },
            bottom_start: CornerRadius { x: 0.0, y: 0.0 },
        };
        let normalized = normalize_corner_radii(
            SurfaceSize {
                width: huge,
                height: 1.0,
            },
            radii,
        )
        .unwrap();
        assert!(normalized.top_start.x.is_finite());
        assert_eq!(normalized.top_start.x, huge / 2.0);
        assert_eq!(normalized.top_end.x, huge / 2.0);
    }

    #[test]
    fn subnormal_scale_preserves_representable_radii() {
        let radii = LogicalCornerRadii {
            top_start: CornerRadius { x: 1e300, y: 0.0 },
            top_end: CornerRadius { x: 1e300, y: 0.0 },
            bottom_end: CornerRadius { x: 0.0, y: 0.0 },
            bottom_start: CornerRadius { x: 0.0, y: 0.0 },
        };
        let normalized = normalize_corner_radii(
            SurfaceSize {
                width: 1e-100,
                height: 1.0,
            },
            radii,
        )
        .unwrap();
        assert_eq!(normalized.top_start.x, 5e-101);
        assert_eq!(normalized.top_end.x, 5e-101);
    }
}
