#[cfg(not(target_os = "linux"))]
compile_error!("the GUIdo backend requires Linux");

mod command_content;
pub use command_content::{CommandContentPrepareError, prepare_command_content};

mod text;
pub use text::{
    LabelMeasureError, LabelPrepareError, measure_command_label, prepare_command_label,
};

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
    check_coordinate(value, rounded)?;
    Ok(rounded)
}

fn check_coordinate(value: f64, native: f32) -> Result<(), PrepareError> {
    if !native.is_finite() || (value - f64::from(native)).abs() > MAX_COORDINATE_ERROR {
        return Err(PrepareError::CoordinatePrecision(value));
    }
    Ok(())
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
    check_coordinate(right / scale, origin.0 + size.width)?;
    check_coordinate(bottom / scale, origin.1 + size.height)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_corner_addition_must_preserve_the_coordinate_budget_before_rasterizing() {
        for (scale, start, pixels) in [
            (1.25_f32, 40_001.0, 1.0),
            (1.5, 40_000.0, 1.0),
            (1.75, 40_000.0, 6.0),
            (3.0, 49_129.0, 24.0),
        ] {
            let scale64 = f64::from(scale);
            let origin = start / scale64;
            let extent = pixels / scale64;
            let edge = (start + pixels) / scale64;
            let native_origin = coordinate(origin).unwrap();
            let native_extent = coordinate(extent).unwrap();
            coordinate(edge).unwrap();
            assert!((f64::from(native_origin + native_extent) - edge).abs() > MAX_COORDINATE_ERROR);
            for horizontal in [true, false] {
                let bounds = PhysicalBounds {
                    x: if horizontal {
                        (start + 0.125) / scale64
                    } else {
                        0.0
                    },
                    y: if horizontal {
                        0.0
                    } else {
                        (start + 0.125) / scale64
                    },
                    width: (pixels - 0.25) / scale64,
                    height: (pixels - 0.25) / scale64,
                };
                assert!(matches!(
                    prepare(bounds, scale, |_| panic!("invalid native corners must fail before rasterization")),
                    Err(PrepareError::CoordinatePrecision(value)) if value == edge
                ));
            }
        }
    }

    #[test]
    fn representable_image_corners_reach_rasterization() {
        let bounds = PhysicalBounds {
            x: 32_000.1,
            y: -32_000.7,
            width: 0.2,
            height: 0.2,
        };
        assert!(matches!(
            prepare(bounds, 1.25, |viewport| {
                assert_eq!(viewport.width, 1);
                assert_eq!(viewport.height, 1);
                Err(RasterError::InvalidSampling)
            }),
            Err(PrepareError::Raster(RasterError::InvalidSampling))
        ));
    }
}
