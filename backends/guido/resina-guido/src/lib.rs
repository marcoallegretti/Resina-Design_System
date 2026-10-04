#[cfg(not(target_os = "linux"))]
compile_error!("the GUIdo backend requires Linux");

mod text;
pub use text::{LabelMeasureError, measure_command_label};

mod focus;
pub use focus::{FocusBinding, FocusTransferError, RequestedFocus, request_focus};

use guido::{layout::Size, prelude::ImageSource};
use resina_model::{PhysicalBounds, PhysicalVector};
use resina_raster::{RasterError, RasterImage, Viewport};
use resina_resolver::{FocusIndicatorIr, OpaqueSurfaceIr, SurfacePaintIr};
use std::fmt;

const MAX_COORDINATE_ERROR: f64 = 1.0 / 1024.0;

#[derive(Debug)]
pub enum PrepareError {
    InvalidScale,
    UnrepresentableBounds,
    CoordinatePrecision(f64),
    Raster(RasterError),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScale => formatter.write_str("device scale must be finite and positive"),
            Self::UnrepresentableBounds => {
                formatter.write_str("paint bounds cannot be represented at the requested scale")
            }
            Self::CoordinatePrecision(value) => write!(
                formatter,
                "GUIdo coordinate {value} exceeds the 1/1024 logical px binary32 rounding budget"
            ),
            Self::Raster(error) => write!(formatter, "paint preparation failed: {error}"),
        }
    }
}

impl std::error::Error for PrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Raster(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedPaint {
    source: ImageSource,
    origin: (f32, f32),
    size: Size,
}

impl PreparedPaint {
    pub fn image_source(&self) -> ImageSource {
        self.source.clone()
    }
    pub fn origin(&self) -> (f32, f32) {
        self.origin
    }
    pub fn logical_size(&self) -> Size {
        self.size
    }
}

pub fn prepare_surface(
    ir: &OpaqueSurfaceIr,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<PreparedPaint, PrepareError> {
    let bounds = ir
        .geometry()
        .silhouette()
        .bounds()
        .expect("validated surface bounds");
    prepare(bounds, device_scale, |viewport| {
        resina_raster::render_surface(ir, viewport, samples_per_axis)
    })
}

pub fn prepare_focus(
    ir: &FocusIndicatorIr,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<PreparedPaint, PrepareError> {
    let outer = ir.geometry().outer();
    let mut bounds = outer.contour().bounds().expect("validated focus bounds");
    bounds.x += outer.offset().x;
    bounds.y += outer.offset().y;
    prepare(bounds, device_scale, |viewport| {
        resina_raster::render_focus(ir, viewport, samples_per_axis)
    })
}

fn coordinate(value: f64) -> Result<f32, PrepareError> {
    let rounded = value as f32;
    if !rounded.is_finite() || (value - f64::from(rounded)).abs() > MAX_COORDINATE_ERROR {
        return Err(PrepareError::CoordinatePrecision(value));
    }
    Ok(rounded)
}

pub fn prepare_surface_paint(
    ir: &SurfacePaintIr,
    device_scale: f32,
    samples_per_axis: u8,
) -> Result<PreparedPaint, PrepareError> {
    let Some(focus) = ir.focus() else {
        return prepare_surface(ir.body(), device_scale, samples_per_axis);
    };
    let outer = focus.geometry().outer();
    let mut bounds = outer.contour().bounds().expect("validated focus bounds");
    bounds.x += outer.offset().x;
    bounds.y += outer.offset().y;
    prepare(bounds, device_scale, |viewport| {
        resina_raster::render_surface_paint(ir, viewport, samples_per_axis)
    })
}

fn prepare(
    bounds: PhysicalBounds,
    device_scale: f32,
    render: impl FnOnce(Viewport) -> Result<RasterImage, RasterError>,
) -> Result<PreparedPaint, PrepareError> {
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(PrepareError::InvalidScale);
    }
    let scale = f64::from(device_scale);
    let left = (bounds.x * scale).floor();
    let top = (bounds.y * scale).floor();
    let right = ((bounds.x + bounds.width) * scale).ceil();
    let bottom = ((bounds.y + bounds.height) * scale).ceil();
    let width = right - left;
    let height = bottom - top;
    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return Err(PrepareError::UnrepresentableBounds);
    }
    let viewport = Viewport {
        origin: PhysicalVector {
            x: left / scale,
            y: top / scale,
        },
        width: width as u32,
        height: height as u32,
        pixels_per_unit: scale,
    };
    let origin = (
        coordinate(viewport.origin.x)?,
        coordinate(viewport.origin.y)?,
    );
    let size = Size::new(coordinate(width / scale)?, coordinate(height / scale)?);
    coordinate(right / scale)?;
    coordinate(bottom / scale)?;
    let image = render(viewport).map_err(PrepareError::Raster)?;
    let source = ImageSource::Rgba {
        width: image.width(),
        height: image.height(),
        pixels: image.into_rgba().into(),
    };
    Ok(PreparedPaint {
        source,
        origin,
        size,
    })
}
