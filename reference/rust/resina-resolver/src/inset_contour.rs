use crate::{CornerGeometryError, normalize_corner_radii};
use resina_model::{CornerRadius, LogicalCornerRadii, SurfaceSize};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InsetContourRequest {
    schema_version: String,
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    inset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsetContourResult {
    schema_version: &'static str,
    outer_radii: LogicalCornerRadii,
    inset: f64,
    size: SurfaceSize,
    radii: LogicalCornerRadii,
}

impl InsetContourResult {
    pub fn outer_radii(&self) -> LogicalCornerRadii {
        self.outer_radii
    }
    pub fn inset(&self) -> f64 {
        self.inset
    }
    pub fn size(&self) -> SurfaceSize {
        self.size
    }
    pub fn radii(&self) -> LogicalCornerRadii {
        self.radii
    }
}

#[derive(Debug)]
pub enum InsetContourError {
    Parse(serde_json::Error),
    InvalidRequestShape,
    Request(serde_json::Error),
    UnsupportedVersion,
    Geometry(CornerGeometryError),
    InvalidInset,
    InsetExceedsBounds,
}

impl fmt::Display for InsetContourError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(formatter, "inset contour parse failed: {error}"),
            Self::Request(error) => write!(formatter, "invalid inset contour request: {error}"),
            Self::InvalidRequestShape => {
                formatter.write_str("inset contour request must be a JSON object")
            }
            Self::UnsupportedVersion => formatter.write_str("schemaVersion must be 0.1.0"),
            Self::Geometry(error) => write!(formatter, "invalid contour geometry: {error}"),
            Self::InvalidInset => formatter.write_str("inset must be finite and nonnegative"),
            Self::InsetExceedsBounds => {
                formatter.write_str("inset exceeds half the surface bounds")
            }
        }
    }
}

impl std::error::Error for InsetContourError {}

pub fn resolve_inset_contour_source(source: &str) -> Result<InsetContourResult, InsetContourError> {
    let document = parse_token_document(source).map_err(InsetContourError::Parse)?;
    if !document.is_object() {
        return Err(InsetContourError::InvalidRequestShape);
    }
    let request: InsetContourRequest =
        serde_json::from_value(document).map_err(InsetContourError::Request)?;
    if request.schema_version != "0.1.0" {
        return Err(InsetContourError::UnsupportedVersion);
    }
    resolve_inset_contour(request.size, request.radii, request.inset)
}

pub fn resolve_inset_contour(
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    inset: f64,
) -> Result<InsetContourResult, InsetContourError> {
    let outer_radii = normalize_corner_radii(size, radii).map_err(InsetContourError::Geometry)?;
    if !inset.is_finite() || inset < 0.0 {
        return Err(InsetContourError::InvalidInset);
    }
    if inset > size.width / 2.0 || inset > size.height / 2.0 {
        return Err(InsetContourError::InsetExceedsBounds);
    }
    let inner_size = SurfaceSize {
        width: size.width - 2.0 * inset,
        height: size.height - 2.0 * inset,
    };
    let shrink = |corner: CornerRadius| CornerRadius {
        x: (corner.x - inset).max(0.0),
        y: (corner.y - inset).max(0.0),
    };
    let radii = normalize_corner_radii(
        inner_size,
        LogicalCornerRadii {
            top_start: shrink(outer_radii.top_start),
            top_end: shrink(outer_radii.top_end),
            bottom_end: shrink(outer_radii.bottom_end),
            bottom_start: shrink(outer_radii.bottom_start),
        },
    )
    .map_err(InsetContourError::Geometry)?;
    Ok(InsetContourResult {
        schema_version: "0.1.0",
        outer_radii,
        inset,
        size: inner_size,
        radii,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn public_contour_vectors() {
        let vectors: Vec<Value> = serde_json::from_str(include_str!(
            "../../../../conformance/geometry/inset-contour-vectors.json"
        ))
        .unwrap();
        for vector in vectors {
            let result = resolve_inset_contour_source(&vector["request"].to_string());
            if let Some(expected) = vector.get("expected") {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                compare_numbers(&actual, expected, vector["name"].as_str().unwrap());
            } else {
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains(vector["errorContains"].as_str().unwrap()),
                    "{error}"
                );
            }
        }
    }

    fn compare_numbers(actual: &Value, expected: &Value, name: &str) {
        match (actual, expected) {
            (Value::Object(actual), Value::Object(expected)) => {
                assert_eq!(actual.len(), expected.len(), "{name}");
                for (key, value) in expected {
                    compare_numbers(&actual[key], value, name);
                }
            }
            (Value::Number(actual), Value::Number(expected)) => {
                assert!(
                    (actual.as_f64().unwrap() - expected.as_f64().unwrap()).abs() <= 1e-12,
                    "{name}"
                );
            }
            _ => assert_eq!(actual, expected, "{name}"),
        }
    }

    #[test]
    fn typed_calls_reject_nonfinite_insets() {
        let size = SurfaceSize {
            width: 20.0,
            height: 10.0,
        };
        let radii = LogicalCornerRadii {
            top_start: CornerRadius { x: 2.0, y: 2.0 },
            top_end: CornerRadius { x: 2.0, y: 2.0 },
            bottom_end: CornerRadius { x: 2.0, y: 2.0 },
            bottom_start: CornerRadius { x: 2.0, y: 2.0 },
        };
        for inset in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                resolve_inset_contour(size, radii, inset),
                Err(InsetContourError::InvalidInset)
            ));
        }
    }

    #[test]
    fn inner_boundary_stays_within_outer_silhouette() {
        let size = SurfaceSize {
            width: 80.0,
            height: 40.0,
        };
        for values in [
            [(12.0, 12.0); 4],
            [(0.0, 0.0), (80.0, 40.0), (0.0, 0.0), (0.0, 0.0)],
            [(50.0, 7.0), (4.0, 35.0), (22.0, 11.0), (9.0, 40.0)],
            [(100.0, 2.0), (1.0, 60.0), (20.0, 40.0), (30.0, 8.0)],
        ] {
            let [top_start, top_end, bottom_end, bottom_start] =
                values.map(|(x, y)| CornerRadius { x, y });
            let radii = LogicalCornerRadii {
                top_start,
                top_end,
                bottom_end,
                bottom_start,
            };
            for inset in [0.0, 0.5, 2.0, 8.0, 19.0, 20.0] {
                let result = resolve_inset_contour(size, radii, inset).unwrap();
                let corners =
                    |r: LogicalCornerRadii| [r.top_start, r.top_end, r.bottom_end, r.bottom_start];
                for (index, corner) in corners(result.radii()).into_iter().enumerate() {
                    for step in 0..=32 {
                        let angle = f64::from(step) * std::f64::consts::FRAC_PI_2 / 32.0;
                        let dx = corner.x * (1.0 - angle.cos());
                        let dy = corner.y * (1.0 - angle.sin());
                        let x = if index == 0 || index == 3 {
                            inset + dx
                        } else {
                            size.width - inset - dx
                        };
                        let y = if index < 2 {
                            inset + dy
                        } else {
                            size.height - inset - dy
                        };
                        for (outer_index, outer) in
                            corners(result.outer_radii()).into_iter().enumerate()
                        {
                            if outer.x == 0.0 || outer.y == 0.0 {
                                continue;
                            }
                            let dx = if outer_index == 0 || outer_index == 3 {
                                x
                            } else {
                                size.width - x
                            };
                            let dy = if outer_index < 2 { y } else { size.height - y };
                            if dx < outer.x && dy < outer.y {
                                let ellipse = ((dx - outer.x) / outer.x).powi(2)
                                    + ((dy - outer.y) / outer.y).powi(2);
                                assert!(
                                    ellipse <= 1.0 + 1e-12,
                                    "inset {inset}, corner {index}, step {step}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn authored_shapes_support_nested_edge_and_highlight_contours() {
        use resina_model::{ShapeFallbackAssignments, ShapeIntent};
        use resina_tokens::resolve_token_document;

        let tokens = resolve_token_document(
            &serde_json::from_str(include_str!("../../../../tokens/foundation.json")).unwrap(),
        )
        .unwrap();
        let assignments: ShapeFallbackAssignments =
            serde_json::from_str(include_str!("../../../../definitions/tier0-shapes.json"))
                .unwrap();
        for shape in [
            ShapeIntent::Structural,
            ShapeIntent::Soft,
            ShapeIntent::Rounded,
            ShapeIntent::Capsule,
            ShapeIntent::Organic,
        ] {
            for size in [
                SurfaceSize {
                    width: 140.0,
                    height: 40.0,
                },
                SurfaceSize {
                    width: 20.0,
                    height: 20.0,
                },
            ] {
                let outer =
                    crate::resolve_shape_fallback(shape, size, &assignments, &tokens).unwrap();
                let edge = resolve_inset_contour(size, outer, 1.0).unwrap();
                let highlight = resolve_inset_contour(edge.size(), edge.radii(), 1.0).unwrap();
                assert_eq!(edge.outer_radii(), outer);
                assert_eq!(highlight.outer_radii(), edge.radii());
                assert!(highlight.size().width > 0.0 && highlight.size().height > 0.0);
            }
        }
    }
}
