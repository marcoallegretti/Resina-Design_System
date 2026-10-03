use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalVector {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PhysicalBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContourSegment {
    Line {
        from: PhysicalVector,
        to: PhysicalVector,
    },
    /// Clockwise ellipse arc; start and end are unit radial parameters, not points.
    Arc {
        center: PhysicalVector,
        radii: CornerRadius,
        start: PhysicalVector,
        end: PhysicalVector,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CornerRadius {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogicalCornerRadii {
    pub top_start: CornerRadius,
    pub top_end: CornerRadius,
    pub bottom_end: CornerRadius,
    pub bottom_start: CornerRadius,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceSize {
    pub width: f64,
    pub height: f64,
}
