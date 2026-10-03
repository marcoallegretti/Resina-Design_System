use resina_color::{ColorConversionError, SrgbFallback, linear_srgb_to_srgb, srgb_to_linear_srgb};
use resina_model::PhysicalVector;
use resina_resolver::{FocusIndicatorIr, OpaqueSurfaceIr, SurfacePaintError, SurfacePaintIr};
use std::{collections::TryReserveError, fmt};
mod uniform;
use uniform::SampleBox;
#[cfg(test)]
mod tests;

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

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    #[cfg(feature = "png")]
    pub fn write_png(&self, writer: impl std::io::Write) -> Result<(), png::EncodingError> {
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
    let geometry = surface.geometry();
    let supported = uniform::supported(geometry.front())
        && uniform::supported(geometry.silhouette())
        && uniform::placed_supported(geometry.edge_interior())
        && uniform::placed_supported(geometry.highlight_outer())
        && uniform::placed_supported(geometry.content());
    render(
        viewport,
        samples_per_axis,
        surface.pigment().body(),
        |region| {
            if !supported {
                UniformRegion::Sample
            } else if uniform::disjoint(region, geometry.silhouette()) {
                UniformRegion::Clear
            } else if uniform::placed_inside(region, geometry.content()) {
                UniformRegion::Solid
            } else {
                UniformRegion::Sample
            }
        },
        |point| surface.sample_paint(point),
    )
}

pub fn render_focus(
    indicator: &FocusIndicatorIr,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    let geometry = indicator.geometry();
    let supported =
        uniform::placed_supported(geometry.outer()) && uniform::placed_supported(geometry.inner());
    render(
        viewport,
        samples_per_axis,
        indicator.indicator().color(),
        |region| {
            if !supported {
                UniformRegion::Sample
            } else if uniform::placed_disjoint(region, geometry.outer())
                || uniform::placed_inside(region, geometry.inner())
            {
                UniformRegion::Clear
            } else {
                UniformRegion::Sample
            }
        },
        |point| indicator.sample_paint(point),
    )
}

enum UniformRegion {
    Clear,
    Solid,
    Sample,
}

pub fn render_surface_paint(
    paint: &SurfacePaintIr,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    let body = paint.body();
    let Some(focus) = paint.focus() else {
        return render_surface(body, viewport, samples_per_axis);
    };
    let geometry = body.geometry();
    let ring = focus.geometry();
    let supported = uniform::supported(geometry.front())
        && uniform::supported(geometry.silhouette())
        && uniform::placed_supported(geometry.edge_interior())
        && uniform::placed_supported(geometry.highlight_outer())
        && uniform::placed_supported(geometry.content())
        && uniform::placed_supported(ring.inner())
        && uniform::placed_supported(ring.outer());
    render(
        viewport,
        samples_per_axis,
        body.pigment().body(),
        |region| {
            if !supported {
                UniformRegion::Sample
            } else if uniform::placed_inside(region, geometry.content()) {
                UniformRegion::Solid
            } else if uniform::disjoint(region, geometry.silhouette())
                && (uniform::placed_disjoint(region, ring.outer())
                    || uniform::placed_inside(region, ring.inner()))
            {
                UniformRegion::Clear
            } else {
                UniformRegion::Sample
            }
        },
        |point| match body.sample_paint(point)? {
            Some(color) => Ok(Some(color)),
            None => focus.sample_paint(point),
        },
    )
}

fn render(
    viewport: Viewport,
    samples_per_axis: u8,
    solid_color: &SrgbFallback,
    uniform: impl Fn(SampleBox) -> UniformRegion,
    mut sample: impl FnMut(PhysicalVector) -> Result<Option<SrgbFallback>, SurfacePaintError>,
) -> Result<RasterImage, RasterError> {
    let byte_count = validate(viewport, samples_per_axis)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(byte_count)
        .map_err(RasterError::Allocation)?;
    let grid = f64::from(samples_per_axis);
    let sample_count = grid * grid;
    let mut decoded_color = None;
    let mut solid_pixel = None;
    for y in 0..viewport.height {
        for x in 0..viewport.width {
            if samples_per_axis >= 3 {
                let at = |x: u32, y: u32, sample: f64| PhysicalVector {
                    x: viewport.origin.x
                        + (f64::from(x) + (sample + 0.5) / grid) / viewport.pixels_per_unit,
                    y: viewport.origin.y
                        + (f64::from(y) + (sample + 0.5) / grid) / viewport.pixels_per_unit,
                };
                let region = SampleBox {
                    min: at(x, y, 0.0),
                    max: at(x, y, grid - 1.0),
                };
                match uniform(region) {
                    UniformRegion::Clear => {
                        rgba.extend_from_slice(&[0; 4]);
                        continue;
                    }
                    UniformRegion::Solid => {
                        let pixel = match solid_pixel {
                            Some(pixel) => pixel,
                            None => {
                                let pixel = constant_pixel(solid_color, samples_per_axis)?;
                                solid_pixel = Some(pixel);
                                pixel
                            }
                        };
                        rgba.extend_from_slice(&pixel);
                        continue;
                    }
                    UniformRegion::Sample => {}
                }
            }
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
                        let components = color.components();
                        let linear = match decoded_color {
                            Some((previous, linear)) if previous == components => linear,
                            _ => {
                                let linear =
                                    srgb_to_linear_srgb(components).map_err(RasterError::Color)?;
                                decoded_color = Some((components, linear));
                                linear
                            }
                        };
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

fn constant_pixel(color: &SrgbFallback, samples_per_axis: u8) -> Result<[u8; 4], RasterError> {
    let linear = srgb_to_linear_srgb(color.components()).map_err(RasterError::Color)?;
    let count = u32::from(samples_per_axis).pow(2);
    let mut sum = [0.0; 3];
    // Repeated addition preserves the sampled path's rounding at RGBA8 thresholds.
    for _ in 0..count {
        for (sum, value) in sum.iter_mut().zip(linear) {
            *sum += value;
        }
    }
    let rgb = linear_srgb_to_srgb(sum.map(|value| value / f64::from(count)))
        .map_err(RasterError::Color)?;
    let [r, g, b] = rgb.map(quantize);
    Ok([r, g, b, 255])
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
