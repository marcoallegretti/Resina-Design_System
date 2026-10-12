use crate::normalize_corner_radii;
use resina_environment::LayoutDirection;
use resina_model::{CornerRadius, LogicalCornerRadii, PhysicalBounds, PhysicalVector, SurfaceSize};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutlineCurvature {
    Circular,
    Quartic,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalCorners {
    top_left: CornerRadius,
    top_right: CornerRadius,
    bottom_right: CornerRadius,
    bottom_left: CornerRadius,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceOutline {
    schema_version: &'static str,
    curvature: OutlineCurvature,
    size: SurfaceSize,
    corners: PhysicalCorners,
    content_bounds: PhysicalBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceOutlineError(String);

impl fmt::Display for SurfaceOutlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "surface outline: {}", self.0)
    }
}
impl std::error::Error for SurfaceOutlineError {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OutlineRequest {
    schema_version: String,
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    layout_direction: LayoutDirection,
    curvature: OutlineCurvature,
    minimum_inset: f64,
}

pub fn resolve_surface_outline_source(source: &str) -> Result<SurfaceOutline, SurfaceOutlineError> {
    let document = parse_token_document(source).map_err(|e| SurfaceOutlineError(e.to_string()))?;
    for pointer in [
        "",
        "/size",
        "/radii",
        "/radii/topStart",
        "/radii/topEnd",
        "/radii/bottomEnd",
        "/radii/bottomStart",
    ] {
        if !document
            .pointer(pointer)
            .is_some_and(serde_json::Value::is_object)
        {
            return Err(SurfaceOutlineError(format!("{pointer} must be an object")));
        }
    }
    let request: OutlineRequest =
        serde_json::from_value(document).map_err(|e| SurfaceOutlineError(e.to_string()))?;
    if request.schema_version != "0.1.0" {
        return Err(SurfaceOutlineError("schemaVersion must be 0.1.0".into()));
    }
    resolve_surface_outline(
        request.size,
        request.radii,
        request.layout_direction,
        request.curvature,
        request.minimum_inset,
    )
}

pub fn resolve_surface_outline(
    size: SurfaceSize,
    radii: LogicalCornerRadii,
    direction: LayoutDirection,
    curvature: OutlineCurvature,
    minimum_inset: f64,
) -> Result<SurfaceOutline, SurfaceOutlineError> {
    let normalized =
        normalize_corner_radii(size, radii).map_err(|e| SurfaceOutlineError(e.to_string()))?;
    if size.width <= 0.0 || size.height <= 0.0 {
        return Err(SurfaceOutlineError(
            "size must enclose positive area".into(),
        ));
    }
    if !minimum_inset.is_finite() || minimum_inset < 0.0 {
        return Err(SurfaceOutlineError(
            "minimumInset must be finite and nonnegative".into(),
        ));
    }
    for (authored, resolved) in [
        (radii.top_start, normalized.top_start),
        (radii.top_end, normalized.top_end),
        (radii.bottom_end, normalized.bottom_end),
        (radii.bottom_start, normalized.bottom_start),
    ] {
        if (authored.x > 0.0 && resolved.x == 0.0) || (authored.y > 0.0 && resolved.y == 0.0) {
            return Err(SurfaceOutlineError(
                "normalization loses a positive corner extent".into(),
            ));
        }
    }
    let square = |r: CornerRadius| {
        if r.x == 0.0 || r.y == 0.0 {
            CornerRadius { x: 0.0, y: 0.0 }
        } else {
            r
        }
    };
    let physical = match direction {
        LayoutDirection::Ltr => [
            normalized.top_start,
            normalized.top_end,
            normalized.bottom_end,
            normalized.bottom_start,
        ],
        LayoutDirection::Rtl => [
            normalized.top_end,
            normalized.top_start,
            normalized.bottom_start,
            normalized.bottom_end,
        ],
    }
    .map(square);
    for r in physical {
        if (r.x > 0.0 && size.width - r.x == size.width)
            || (r.y > 0.0 && size.height - r.y == size.height)
        {
            return Err(SurfaceOutlineError(
                "corner extent is unrepresentable in these bounds".into(),
            ));
        }
    }
    let diagonal = match curvature {
        OutlineCurvature::Circular => 1.0 - 0.5_f64.sqrt(),
        OutlineCurvature::Quartic => 1.0 - 0.5_f64.sqrt().sqrt(),
    } + 4.0 * f64::EPSILON;
    let guard = |a: f64, b: f64| {
        let extent = a.max(b);
        if extent == 0.0 {
            minimum_inset
        } else {
            (extent * diagonal).next_up().max(minimum_inset)
        }
    };
    let left = guard(physical[0].x, physical[3].x);
    let right = guard(physical[1].x, physical[2].x);
    let top = guard(physical[0].y, physical[1].y);
    let bottom = guard(physical[2].y, physical[3].y);
    let width = (size.width - right) - left;
    let height = (size.height - bottom) - top;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || (left > 0.0 && width == size.width)
        || (top > 0.0 && height == size.height)
        || (right > 0.0 && size.width - right == size.width)
        || (bottom > 0.0 && size.height - bottom == size.height)
    {
        return Err(SurfaceOutlineError(
            "content bounds must have representable positive area".into(),
        ));
    }
    let result = SurfaceOutline {
        schema_version: "0.1.0",
        curvature,
        size,
        corners: PhysicalCorners {
            top_left: physical[0],
            top_right: physical[1],
            bottom_right: physical[2],
            bottom_left: physical[3],
        },
        content_bounds: PhysicalBounds {
            x: left,
            y: top,
            width,
            height,
        },
    };
    for x in [left, left + width] {
        for y in [top, top + height] {
            if !result.contains(PhysicalVector { x, y })? {
                return Err(SurfaceOutlineError(
                    "content containment is unrepresentable".into(),
                ));
            }
        }
    }
    Ok(result)
}

impl SurfaceOutline {
    pub fn size(&self) -> SurfaceSize {
        self.size
    }
    pub fn curvature(&self) -> OutlineCurvature {
        self.curvature
    }
    pub fn corners(&self) -> [CornerRadius; 4] {
        [
            self.corners.top_left,
            self.corners.top_right,
            self.corners.bottom_right,
            self.corners.bottom_left,
        ]
    }
    pub fn content_bounds(&self) -> PhysicalBounds {
        self.content_bounds
    }

    pub fn contains(&self, point: PhysicalVector) -> Result<bool, SurfaceOutlineError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(SurfaceOutlineError("point must be finite".into()));
        }
        if point.x < 0.0 || point.y < 0.0 || point.x > self.size.width || point.y > self.size.height
        {
            return Ok(false);
        }
        let coordinates = [
            (point.x, point.y),
            (self.size.width - point.x, point.y),
            (self.size.width - point.x, self.size.height - point.y),
            (point.x, self.size.height - point.y),
        ];
        for (r, (x, y)) in self.corners().into_iter().zip(coordinates) {
            if r.x == 0.0 || r.y == 0.0 || x >= r.x || y >= r.y {
                continue;
            }
            let u = (r.x - x) / r.x;
            let v = (r.y - y) / r.y;
            let squared = u * u;
            let other_squared = v * v;
            let value = match self.curvature {
                OutlineCurvature::Circular => squared + other_squared,
                OutlineCurvature::Quartic => squared * squared + other_squared * other_squared,
            };
            if value > 1.0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
