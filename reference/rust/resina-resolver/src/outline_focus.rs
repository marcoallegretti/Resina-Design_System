mod arithmetic;
mod certificate;

use crate::{
    FocusIndicatorResult, OutlineCurvature, SurfaceOutline, resolve_focus_indicator_source,
    resolve_surface_outline_source,
};
use resina_model::{OpticalTreatment, PhysicalBounds, PhysicalVector};
use resina_tokens::parse_token_document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineFocusError(String);
impl fmt::Display for OutlineFocusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "outline focus: {}", self.0)
    }
}
impl std::error::Error for OutlineFocusError {}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineFocusGeometry {
    outline: SurfaceOutline,
    side_offset: PhysicalVector,
    inner_outset: f64,
    outer_outset: f64,
    silhouette_bounds: PhysicalBounds,
    inner_bounds: PhysicalBounds,
    outer_bounds: PhysicalBounds,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineFocusIr {
    schema_version: &'static str,
    indicator: FocusIndicatorResult,
    geometry: OutlineFocusGeometry,
}
impl OutlineFocusIr {
    pub fn indicator(&self) -> &FocusIndicatorResult {
        &self.indicator
    }
    pub fn geometry(&self) -> &OutlineFocusGeometry {
        &self.geometry
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct OutlineFocusBoundary {
    pub point: PhysicalVector,
    pub support: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutlineFocusSupport {
    pub normal: PhysicalVector,
    pub silhouette: OutlineFocusBoundary,
    pub inner: OutlineFocusBoundary,
    pub outer: OutlineFocusBoundary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineFocusResult {
    schema_version: &'static str,
    focus: OutlineFocusIr,
    supports: Vec<OutlineFocusSupport>,
}
impl OutlineFocusResult {
    pub fn focus(&self) -> &OutlineFocusIr {
        &self.focus
    }
    pub fn supports(&self) -> &[OutlineFocusSupport] {
        &self.supports
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    focus: Value,
    outline: Value,
    side_offset: PhysicalVector,
    directions: Vec<PhysicalVector>,
}

#[derive(Deserialize)]
struct SourceVector<'a> {
    #[serde(borrow)]
    x: &'a RawValue,
    #[serde(borrow)]
    y: &'a RawValue,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceVectors<'a> {
    #[serde(borrow)]
    side_offset: SourceVector<'a>,
    #[serde(borrow)]
    directions: Vec<SourceVector<'a>>,
}

fn retain_source_components(
    source: &SourceVector<'_>,
    represented: PhysicalVector,
    location: &str,
) -> Result<(), OutlineFocusError> {
    for (component, original, value) in [
        ("x", source.x, represented.x),
        ("y", source.y, represented.y),
    ] {
        let nonzero = original
            .get()
            .bytes()
            .take_while(|byte| !matches!(byte, b'e' | b'E'))
            .any(|byte| matches!(byte, b'1'..=b'9'));
        if value == 0.0 && nonzero {
            return Err(OutlineFocusError(format!(
                "{location}/{component}: numeric precision cannot retain nonzero source component"
            )));
        }
    }
    Ok(())
}

pub fn resolve_outline_focus_source(source: &str) -> Result<OutlineFocusResult, OutlineFocusError> {
    let document = parse_token_document(source).map_err(|e| OutlineFocusError(e.to_string()))?;
    if !document.is_object() {
        return Err(OutlineFocusError("request must be an object".into()));
    }
    let request: Request =
        serde_json::from_value(document).map_err(|e| OutlineFocusError(e.to_string()))?;
    if request.schema_version != "0.1.0" {
        return Err(OutlineFocusError("schemaVersion must be 0.1.0".into()));
    }
    let originals: SourceVectors<'_> =
        serde_json::from_str(source).map_err(|error| OutlineFocusError(error.to_string()))?;
    retain_source_components(&originals.side_offset, request.side_offset, "sideOffset")?;
    for (index, (original, represented)) in originals
        .directions
        .iter()
        .zip(&request.directions)
        .enumerate()
    {
        retain_source_components(original, *represented, &format!("directions/{index}"))?;
    }
    let indicator = resolve_focus_indicator_source(&request.focus.to_string())
        .map_err(|e| OutlineFocusError(e.to_string()))?;
    let outline = resolve_surface_outline_source(&request.outline.to_string())
        .map_err(|e| OutlineFocusError(e.to_string()))?;
    let focus = resolve_outline_focus(indicator, outline, request.side_offset)?;
    let supports = request
        .directions
        .into_iter()
        .map(|direction| focus.geometry.support_at(direction))
        .collect::<Result<_, _>>()?;
    Ok(OutlineFocusResult {
        schema_version: "0.1.0",
        focus,
        supports,
    })
}

pub fn resolve_outline_focus(
    indicator: FocusIndicatorResult,
    outline: SurfaceOutline,
    side_offset: PhysicalVector,
) -> Result<OutlineFocusIr, OutlineFocusError> {
    if indicator.binding().treatment_stack().treatments().last() != Some(&OpticalTreatment::None) {
        return Err(OutlineFocusError(
            "requires effective current treatment none".into(),
        ));
    }
    finite_vector(side_offset)?;
    let size = outline.size();
    let silhouette_bounds = PhysicalBounds {
        x: side_offset.x.min(0.0),
        y: side_offset.y.min(0.0),
        width: size.width + side_offset.x.abs(),
        height: size.height + side_offset.y.abs(),
    };
    if (side_offset.x != 0.0 && silhouette_bounds.width == size.width)
        || (side_offset.y != 0.0 && silhouette_bounds.height == size.height)
    {
        return Err(numeric_error());
    }
    let inner_outset = indicator.gap();
    let outer_outset = inner_outset + indicator.stroke_width();
    let inner_bounds = expand(silhouette_bounds, inner_outset)?;
    let outer_bounds = expand(silhouette_bounds, outer_outset)?;
    if !close(
        outer_bounds.width - inner_bounds.width,
        2.0 * indicator.stroke_width(),
    ) || !close(
        outer_bounds.height - inner_bounds.height,
        2.0 * indicator.stroke_width(),
    ) || silhouette_bounds.width - side_offset.x.abs() <= 0.0
        || silhouette_bounds.height - side_offset.y.abs() <= 0.0
    {
        return Err(numeric_error());
    }
    Ok(OutlineFocusIr {
        schema_version: "0.1.0",
        indicator,
        geometry: OutlineFocusGeometry {
            outline,
            side_offset,
            inner_outset,
            outer_outset,
            silhouette_bounds,
            inner_bounds,
            outer_bounds,
        },
    })
}

impl OutlineFocusGeometry {
    pub fn outline(&self) -> &SurfaceOutline {
        &self.outline
    }
    pub fn side_offset(&self) -> PhysicalVector {
        self.side_offset
    }
    pub fn inner_outset(&self) -> f64 {
        self.inner_outset
    }
    pub fn outer_outset(&self) -> f64 {
        self.outer_outset
    }
    pub fn silhouette_bounds(&self) -> PhysicalBounds {
        self.silhouette_bounds
    }
    pub fn inner_bounds(&self) -> PhysicalBounds {
        self.inner_bounds
    }
    pub fn outer_bounds(&self) -> PhysicalBounds {
        self.outer_bounds
    }

    pub fn support_at(
        &self,
        direction: PhysicalVector,
    ) -> Result<OutlineFocusSupport, OutlineFocusError> {
        let n = normalize(direction)?;
        let right = n.x >= 0.0;
        let bottom = n.y >= 0.0;
        let corner = self.outline.corners()[match (right, bottom) {
            (false, false) => 0,
            (true, false) => 1,
            (true, true) => 2,
            (false, true) => 3,
        }];
        let size = self.outline.size();
        let c = PhysicalVector {
            x: if right {
                size.width - corner.x
            } else {
                corner.x
            },
            y: if bottom {
                size.height - corner.y
            } else {
                corner.y
            },
        };
        let wx = corner.x * n.x.abs();
        let wy = corner.y * n.y.abs();
        if (corner.x > 0.0 && n.x != 0.0 && wx == 0.0)
            || (corner.y > 0.0 && n.y != 0.0 && wy == 0.0)
        {
            return Err(numeric_error());
        }
        let maximum = wx.max(wy);
        let (length, u, v) = if maximum == 0.0 {
            (0.0, 0.0, 0.0)
        } else {
            let x = wx / maximum;
            let y = wy / maximum;
            if (wx > 0.0 && x == 0.0) || (wy > 0.0 && y == 0.0) {
                return Err(numeric_error());
            }
            match self.outline.curvature() {
                OutlineCurvature::Circular => {
                    let factor = (x * x + y * y).sqrt();
                    (maximum * factor, x / factor, y / factor)
                }
                OutlineCurvature::Quartic => {
                    let sum = x * arithmetic::cube_root(x) + y * arithmetic::cube_root(y);
                    let factor = sum.sqrt() * sum.sqrt().sqrt();
                    (
                        maximum * factor,
                        arithmetic::cube_root(x / factor),
                        arithmetic::cube_root(y / factor),
                    )
                }
            }
        };
        let mut point = PhysicalVector {
            x: c.x + if right { corner.x * u } else { -corner.x * u },
            y: c.y + if bottom { corner.y * v } else { -corner.y * v },
        };
        let mut support = dot(c, n) + length;
        if arithmetic::positive_dot(direction, self.side_offset) {
            if dot(self.side_offset, n) < 0.0 {
                return Err(numeric_error());
            }
            point.x += self.side_offset.x;
            point.y += self.side_offset.y;
            support += dot(self.side_offset, n);
        }
        let silhouette = boundary(point, support, n)?;
        let offset = |amount: f64| {
            boundary(
                PhysicalVector {
                    x: point.x + amount * n.x,
                    y: point.y + amount * n.y,
                },
                support + amount,
                n,
            )
        };
        let inner = offset(self.inner_outset)?;
        let outer = offset(self.outer_outset)?;
        if inner.support <= silhouette.support
            || outer.support <= inner.support
            || !close(inner.support - silhouette.support, self.inner_outset)
            || !close(
                outer.support - inner.support,
                self.outer_outset - self.inner_outset,
            )
        {
            return Err(numeric_error());
        }
        let result = OutlineFocusSupport {
            normal: n,
            silhouette,
            inner,
            outer,
        };
        if !certificate::certifies(self, direction, &result) {
            return Err(OutlineFocusError(
                "support precision cannot certify geometry within 1e-12".into(),
            ));
        }
        Ok(result)
    }
}

fn normalize(direction: PhysicalVector) -> Result<PhysicalVector, OutlineFocusError> {
    finite_vector(direction)?;
    let maximum = direction.x.abs().max(direction.y.abs());
    if maximum == 0.0 {
        return Err(OutlineFocusError("direction must be nonzero".into()));
    }
    let x = direction.x / maximum;
    let y = direction.y / maximum;
    let length = (x * x + y * y).sqrt();
    let result = PhysicalVector {
        x: x / length,
        y: y / length,
    };
    if (direction.x != 0.0 && result.x == 0.0) || (direction.y != 0.0 && result.y == 0.0) {
        return Err(numeric_error());
    }
    Ok(result)
}

fn dot(a: PhysicalVector, b: PhysicalVector) -> f64 {
    a.x * b.x + a.y * b.y
}
fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
}
fn finite_vector(value: PhysicalVector) -> Result<(), OutlineFocusError> {
    if value.x.is_finite() && value.y.is_finite() {
        Ok(())
    } else {
        Err(numeric_error())
    }
}
fn boundary(
    point: PhysicalVector,
    support: f64,
    normal: PhysicalVector,
) -> Result<OutlineFocusBoundary, OutlineFocusError> {
    finite_vector(point)?;
    if !close(dot(point, normal), support) {
        return Err(numeric_error());
    }
    Ok(OutlineFocusBoundary { point, support })
}
fn numeric_error() -> OutlineFocusError {
    OutlineFocusError("geometry cannot be represented at these numeric bounds".into())
}
fn expand(bounds: PhysicalBounds, amount: f64) -> Result<PhysicalBounds, OutlineFocusError> {
    let result = PhysicalBounds {
        x: bounds.x - amount,
        y: bounds.y - amount,
        width: bounds.width + 2.0 * amount,
        height: bounds.height + 2.0 * amount,
    };
    if ![result.x, result.y, result.width, result.height]
        .into_iter()
        .all(f64::is_finite)
        || !close(bounds.x - result.x, amount)
        || !close(bounds.y - result.y, amount)
        || !close(result.width - bounds.width, 2.0 * amount)
        || !close(result.height - bounds.height, 2.0 * amount)
    {
        return Err(numeric_error());
    }
    Ok(result)
}
