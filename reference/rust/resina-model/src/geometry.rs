use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::fmt;

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SurfaceSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceSizeInput {
    width: f64,
    height: f64,
}

impl<'de> Deserialize<'de> for SurfaceSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SizeVisitor;
        impl<'de> Visitor<'de> for SizeVisitor {
            type Value = SurfaceSize;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a surface size object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let input = SurfaceSizeInput::deserialize(MapAccessDeserializer::new(map))?;
                Ok(SurfaceSize {
                    width: input.width,
                    height: input.height,
                })
            }
        }
        deserializer.deserialize_map(SizeVisitor)
    }
}
