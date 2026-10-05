use crate::{SlintError, check_coordinates};
use resina_model::PhysicalVector;
use resina_raster::{RasterError, prepare_viewport};
use resina_resolver::SurfacePaintIr;
use std::{error::Error, fmt, fmt::Write};

#[derive(Debug)]
pub enum PaintSlintError {
    InvalidScale,
    UnrepresentableBounds,
    Coordinates(SlintError),
    Raster(RasterError),
    Encoding(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for PaintSlintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => f.write_str("device scale must be finite and positive"),
            Self::UnrepresentableBounds => f.write_str(
                "paint bounds cannot be represented at the requested placement and scale",
            ),
            Self::Coordinates(error) => error.fmt(f),
            Self::Raster(error) => write!(f, "paint preparation failed: {error}"),
            Self::Encoding(error) => write!(f, "paint PNG encoding failed: {error}"),
        }
    }
}

impl Error for PaintSlintError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Coordinates(error) => Some(error),
            Self::Raster(error) => Some(error),
            Self::Encoding(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Samples after parent placement; emitted paint origins already include that placement.
pub fn render_surface_paint(
    ir: &SurfacePaintIr,
    placement: PhysicalVector,
    device_scale: f64,
    samples_per_axis: u8,
) -> Result<String, PaintSlintError> {
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(PaintSlintError::InvalidScale);
    }
    let bounds = if let Some(focus) = ir.focus() {
        let outer = focus.geometry().outer();
        let mut bounds = outer.contour().bounds().expect("validated focus bounds");
        bounds.x += outer.offset().x;
        bounds.y += outer.offset().y;
        bounds
    } else {
        ir.body()
            .geometry()
            .silhouette()
            .bounds()
            .expect("validated body bounds")
    };
    let prepared = prepare_viewport(bounds, placement, device_scale)
        .map_err(|_| PaintSlintError::UnrepresentableBounds)?;
    let bounds = prepared.bounds();
    let far_corner = prepared.far_corner();
    let origin_x = bounds.x;
    let origin_y = bounds.y;
    let logical_width = bounds.width;
    let logical_height = bounds.height;
    check_coordinates([
        origin_x,
        origin_y,
        logical_width,
        logical_height,
        far_corner.x,
        far_corner.y,
    ])
    .map_err(PaintSlintError::Coordinates)?;
    for (expected, native) in [
        (far_corner.x, origin_x as f32 + logical_width as f32),
        (far_corner.y, origin_y as f32 + logical_height as f32),
    ] {
        if (expected - f64::from(native)).abs() > crate::MAX_COORDINATE_ERROR {
            return Err(PaintSlintError::Coordinates(
                SlintError::CoordinatePrecision(expected),
            ));
        }
    }
    let image = resina_raster::render_surface_paint(ir, prepared.viewport(), samples_per_axis)
        .map_err(PaintSlintError::Raster)?;
    let mut png = Vec::new();
    image
        .write_png(&mut png)
        .map_err(|error| PaintSlintError::Encoding(Box::new(error)))?;
    let mut output = format!(
        r#"export component ResinaSurfacePaint inherits Rectangle {{
    out property <length> paint-origin-x: {origin_x}px;
    out property <length> paint-origin-y: {origin_y}px;
    out property <float> prepared-device-scale: {device_scale};
    width: {logical_width}px;
    height: {logical_height}px;
    background: transparent;
    clip: false;
    accessible-role: none;
    Image {{
        x: 0px;
        y: 0px;
        width: {logical_width}px;
        height: {logical_height}px;
        image-fit: fill;
        image-rendering: pixelated;
        accessible-role: none;
        source: @image-url("data:image/png,"#
    );
    for byte in png {
        write!(output, "%{byte:02X}").expect("string formatting");
    }
    output.push_str("\");\n    }\n}\n");
    Ok(output)
}
