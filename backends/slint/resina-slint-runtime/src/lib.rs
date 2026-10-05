use resina_model::PhysicalVector;
use resina_raster::{RasterError, prepare_viewport};
use resina_resolver::SurfacePaintIr;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::{error::Error, fmt};

const MAX_COORDINATE_ERROR: f64 = 1.0 / 1024.0;

#[cfg(feature = "native-text")]
mod label;
#[cfg(feature = "native-text")]
pub use label::{LabelMeasureError, LabelMeasurer, NativeLabelMeasure};

#[derive(Debug)]
pub enum PrepareError {
    InvalidScale,
    UnrepresentableBounds,
    CoordinatePrecision(f64),
    Raster(RasterError),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => f.write_str("device scale must be finite and positive"),
            Self::UnrepresentableBounds => f.write_str(
                "paint bounds cannot be represented at the requested placement and scale",
            ),
            Self::CoordinatePrecision(value) => write!(
                f,
                "Slint coordinate {value} exceeds the 1/1024 logical px binary32 rounding budget"
            ),
            Self::Raster(error) => write!(f, "paint preparation failed: {error}"),
        }
    }
}

impl Error for PrepareError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Raster(error) => Some(error),
            _ => None,
        }
    }
}

/// Worker-transferable pixels and geometry sampled on the final device grid.
#[derive(Debug, Clone)]
pub struct PreparedPaint {
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    origin: [f32; 2],
    logical_size: [f32; 2],
    device_scale: f32,
}

/// UI-thread image and geometry to publish through one application property.
#[derive(Debug, Clone)]
pub struct PaintSnapshot {
    pub image: Image,
    /// Final x/y placement in logical pixels, including the preparation placement.
    pub origin: [f32; 2],
    /// Fixed width/height in logical pixels; do not stretch when the container changes.
    pub logical_size: [f32; 2],
    /// Actual target window scale used to prepare this image.
    pub device_scale: f32,
}

impl PreparedPaint {
    /// Construct on the UI thread, after transferring the prepared pixels there.
    pub fn into_snapshot(self) -> PaintSnapshot {
        PaintSnapshot {
            image: Image::from_rgba8_premultiplied(self.pixels),
            origin: self.origin,
            logical_size: self.logical_size,
            device_scale: self.device_scale,
        }
    }
}

fn check_coordinate(value: f64, native: f32) -> Result<(), PrepareError> {
    if !native.is_finite() || (value - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
        return Err(PrepareError::CoordinatePrecision(value));
    }
    Ok(())
}

/// Placement is applied before sampling and is already included in the returned origin.
/// Pass the target window's actual scale factor and reprepare after scale or placement changes.
pub fn prepare_surface_paint(
    ir: &SurfacePaintIr,
    placement: PhysicalVector,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<PreparedPaint, PrepareError> {
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(PrepareError::InvalidScale);
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
    let viewport = prepare_viewport(bounds, placement, f64::from(device_scale))
        .map_err(|_| PrepareError::UnrepresentableBounds)?;
    let bounds = viewport.bounds();
    let far_corner = viewport.far_corner();
    for value in [
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        far_corner.x,
        far_corner.y,
    ] {
        check_coordinate(value, value as f32)?;
    }
    let origin = [bounds.x as f32, bounds.y as f32];
    let logical_size = [bounds.width as f32, bounds.height as f32];
    check_coordinate(far_corner.x, origin[0] + logical_size[0])?;
    check_coordinate(far_corner.y, origin[1] + logical_size[1])?;
    let raster = resina_raster::render_surface_paint(ir, viewport.viewport(), samples_per_axis)
        .map_err(PrepareError::Raster)?;
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        raster.rgba(),
        raster.width(),
        raster.height(),
    );
    for pixel in pixels.make_mut_slice() {
        let alpha = u16::from(pixel.a);
        pixel.r = ((u16::from(pixel.r) * alpha + 127) / 255) as u8;
        pixel.g = ((u16::from(pixel.g) * alpha + 127) / 255) as u8;
        pixel.b = ((u16::from(pixel.b) * alpha + 127) / 255) as u8;
    }
    Ok(PreparedPaint {
        pixels,
        origin,
        logical_size,
        device_scale,
    })
}

#[cfg(test)]
mod tests;
