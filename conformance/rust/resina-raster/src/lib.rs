use resina_color::{ColorConversionError, SrgbFallback, linear_srgb_to_srgb, srgb_to_linear_srgb};
use resina_model::PhysicalVector;
use resina_resolver::{FocusIndicatorIr, OpaqueSurfaceIr, SurfacePaintError};
use std::{collections::TryReserveError, fmt, io::Write};

pub const MAX_PIXELS: u64 = 4_194_304;
pub const MAX_SAMPLES: u64 = 16_777_216;

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub origin: PhysicalVector,
    pub width: u32,
    pub height: u32,
    pub pixels_per_unit: f64,
}

#[derive(Debug)]
pub enum RasterError {
    InvalidViewport(&'static str),
    InvalidSampling,
    ResourceLimit,
    Allocation(TryReserveError),
    Paint(SurfacePaintError),
    Color(ColorConversionError),
}

impl fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidViewport(reason) => write!(formatter, "invalid raster viewport: {reason}"),
            Self::InvalidSampling => formatter.write_str("samples per axis must be in 1..=8"),
            Self::ResourceLimit => formatter.write_str("raster pixel or sample limit exceeded"),
            Self::Allocation(error) => write!(formatter, "raster allocation failed: {error}"),
            Self::Paint(error) => write!(formatter, "surface paint failed: {error}"),
            Self::Color(error) => write!(formatter, "raster color conversion failed: {error}"),
        }
    }
}

impl std::error::Error for RasterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Paint(error) => Some(error),
            Self::Color(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct RasterImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl RasterImage {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn write_png(&self, writer: impl Write) -> Result<(), png::EncodingError> {
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.rgba)?;
        writer.finish()
    }
}

pub fn render_surface(
    surface: &OpaqueSurfaceIr,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    render(viewport, samples_per_axis, |point| {
        surface.sample_paint(point)
    })
}

pub fn render_focus(
    indicator: &FocusIndicatorIr,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    render(viewport, samples_per_axis, |point| {
        indicator.sample_paint(point)
    })
}

fn render(
    viewport: Viewport,
    samples_per_axis: u8,
    mut sample: impl FnMut(PhysicalVector) -> Result<Option<SrgbFallback>, SurfacePaintError>,
) -> Result<RasterImage, RasterError> {
    let byte_count = validate(viewport, samples_per_axis)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(byte_count)
        .map_err(RasterError::Allocation)?;
    let grid = f64::from(samples_per_axis);
    let sample_count = grid * grid;
    for y in 0..viewport.height {
        for x in 0..viewport.width {
            let mut linear_sum = [0.0; 3];
            let mut covered = 0_u32;
            for sy in 0..samples_per_axis {
                for sx in 0..samples_per_axis {
                    let point = PhysicalVector {
                        x: viewport.origin.x
                            + (f64::from(x) + (f64::from(sx) + 0.5) / grid)
                                / viewport.pixels_per_unit,
                        y: viewport.origin.y
                            + (f64::from(y) + (f64::from(sy) + 0.5) / grid)
                                / viewport.pixels_per_unit,
                    };
                    if let Some(color) = sample(point).map_err(RasterError::Paint)? {
                        covered += 1;
                        let linear =
                            srgb_to_linear_srgb(color.components()).map_err(RasterError::Color)?;
                        for (sum, channel) in linear_sum.iter_mut().zip(linear) {
                            *sum += channel;
                        }
                    }
                }
            }
            if covered == 0 {
                rgba.extend_from_slice(&[0; 4]);
            } else {
                let rgb = linear_srgb_to_srgb(linear_sum.map(|sum| sum / f64::from(covered)))
                    .map_err(RasterError::Color)?;
                rgba.extend(rgb.map(quantize));
                rgba.push(quantize(f64::from(covered) / sample_count));
            }
        }
    }
    Ok(RasterImage {
        width: viewport.width,
        height: viewport.height,
        rgba,
    })
}

fn quantize(channel: f64) -> u8 {
    (channel * 255.0).round() as u8
}

fn validate(viewport: Viewport, samples_per_axis: u8) -> Result<usize, RasterError> {
    if !(1..=8).contains(&samples_per_axis) {
        return Err(RasterError::InvalidSampling);
    }
    if viewport.width == 0 || viewport.height == 0 {
        return Err(RasterError::InvalidViewport("dimensions must be positive"));
    }
    if !viewport.pixels_per_unit.is_finite() || viewport.pixels_per_unit <= 0.0 {
        return Err(RasterError::InvalidViewport(
            "scale must be finite and positive",
        ));
    }
    let pixels = u64::from(viewport.width) * u64::from(viewport.height);
    let samples = pixels.checked_mul(u64::from(samples_per_axis).pow(2));
    if pixels > MAX_PIXELS || samples.is_none_or(|samples| samples > MAX_SAMPLES) {
        return Err(RasterError::ResourceLimit);
    }
    let step = 1.0 / viewport.pixels_per_unit / f64::from(samples_per_axis);
    for (origin, length) in [
        (viewport.origin.x, viewport.width),
        (viewport.origin.y, viewport.height),
    ] {
        let end = origin + f64::from(length) / viewport.pixels_per_unit;
        let magnitude = origin.abs().max(end.abs());
        if !origin.is_finite()
            || !end.is_finite()
            || !step.is_finite()
            || step <= 0.0
            || end <= origin
            || step <= 4.0 * f64::EPSILON * magnitude
        {
            return Err(RasterError::InvalidViewport(
                "sample coordinates lose precision",
            ));
        }
    }
    usize::try_from(pixels * 4).map_err(|_| RasterError::ResourceLimit)
}
