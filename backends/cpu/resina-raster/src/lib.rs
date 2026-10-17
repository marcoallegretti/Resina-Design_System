use resina_color::{ColorConversionError, SrgbFallback, linear_srgb_to_srgb, srgb_to_linear_srgb};
use resina_model::{PhysicalBounds, PhysicalVector};
use resina_resolver::{
    FocusIndicatorIr, MaterialReliefPaint, OpaqueSurfaceIr, ReliefPaint, ReliefPaintError,
    SurfacePaintError, SurfacePaintIr,
};
use std::{
    collections::TryReserveError,
    fmt,
    num::NonZeroUsize,
    panic,
    sync::{Mutex, OnceLock, PoisonError},
    thread,
    time::{Duration, Instant},
};
mod uniform;
use uniform::SampleBox;
mod placement;
pub use placement::{PreparedViewport, prepare_viewport};
mod outline_focus;
pub use outline_focus::render_outline_focus;
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
    ReliefPaint(ReliefPaintError),
    OutlineFocus(resina_resolver::OutlineFocusError),
    FocusPrecision(&'static str),
    Color(ColorConversionError),
}

impl fmt::Display for RasterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidViewport(reason) => write!(formatter, "invalid raster viewport: {reason}"),
            Self::InvalidSampling => formatter.write_str("samples per axis must be in 1..=8"),
            Self::ResourceLimit => {
                formatter.write_str("raster pixel, sample or work limit exceeded")
            }
            Self::Allocation(error) => write!(formatter, "raster allocation failed: {error}"),
            Self::Paint(error) => write!(formatter, "surface paint failed: {error}"),
            Self::ReliefPaint(error) => write!(formatter, "relief paint failed: {error}"),
            Self::OutlineFocus(error) => write!(formatter, "focus support failed: {error}"),
            Self::FocusPrecision(reason) => {
                write!(formatter, "focus coverage cannot be certified: {reason}")
            }
            Self::Color(error) => write!(formatter, "raster color conversion failed: {error}"),
        }
    }
}

impl std::error::Error for RasterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Allocation(error) => Some(error),
            Self::Paint(error) => Some(error),
            Self::ReliefPaint(error) => Some(error),
            Self::OutlineFocus(error) => Some(error),
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
        |point| surface.sample_paint(point).map_err(RasterError::Paint),
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
        |point| indicator.sample_paint(point).map_err(RasterError::Paint),
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
        |point| match body.sample_paint(point).map_err(RasterError::Paint)? {
            Some(color) => Ok(Some(color)),
            None => focus.sample_paint(point).map_err(RasterError::Paint),
        },
    )
}

pub fn render_relief_paint(
    paint: &ReliefPaint,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    let size = paint.surface().outline().size();
    render_relief_face(
        (size.width, size.height),
        paint.content_bounds(),
        paint.pigment().body(),
        viewport,
        samples_per_axis,
        |point| paint.sample_paint(point),
    )
}

pub fn render_material_relief_paint(
    paint: &MaterialReliefPaint,
    viewport: Viewport,
    samples_per_axis: u8,
) -> Result<RasterImage, RasterError> {
    let size = paint.surface().outline().size();
    render_relief_face(
        (size.width, size.height),
        paint.content_bounds(),
        paint.pigment().body(),
        viewport,
        samples_per_axis,
        |point| paint.sample_paint(point),
    )
}

fn render_relief_face(
    size: (f64, f64),
    content: PhysicalBounds,
    body: &SrgbFallback,
    viewport: Viewport,
    samples_per_axis: u8,
    sample: impl Fn(PhysicalVector) -> Result<Option<SrgbFallback>, ReliefPaintError> + Sync,
) -> Result<RasterImage, RasterError> {
    render(
        viewport,
        samples_per_axis,
        body,
        |region| {
            if region.max.x < 0.0
                || region.max.y < 0.0
                || region.min.x > size.0
                || region.min.y > size.1
            {
                UniformRegion::Clear
            } else if region.min.x >= content.x
                && region.min.y >= content.y
                && region.max.x <= content.x + content.width
                && region.max.y <= content.y + content.height
            {
                UniformRegion::Solid
            } else {
                UniformRegion::Sample
            }
        },
        |point| sample(point).map_err(RasterError::ReliefPaint),
    )
}

fn render(
    viewport: Viewport,
    samples_per_axis: u8,
    solid_color: &SrgbFallback,
    uniform: impl Fn(SampleBox) -> UniformRegion + Sync,
    sample: impl Fn(PhysicalVector) -> Result<Option<SrgbFallback>, RasterError> + Sync,
) -> Result<RasterImage, RasterError> {
    let byte_count = validate(viewport, samples_per_axis)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(byte_count)
        .map_err(RasterError::Allocation)?;
    rgba.resize(byte_count, 0);
    let queue = Mutex::new(RowQueue {
        rows: rgba.chunks_exact_mut(viewport.width as usize * 4).zip(0..),
        failure: None,
    });
    let paint_rows = |budget: Option<Duration>| {
        let started = Instant::now();
        let mut painter = RowPainter {
            viewport,
            samples_per_axis,
            solid_color,
            uniform: &uniform,
            sample: &sample,
            decoded_color: None,
            solid_pixel: None,
        };
        while let Some((row, y)) = RowQueue::next(&queue) {
            if let Err(error) = painter.paint(row, y) {
                RowQueue::fail(&queue, y, error);
            }
            if budget.is_some_and(|budget| started.elapsed() >= budget) {
                return false;
            }
        }
        true
    };
    let (workers, serial_budget) = schedule(viewport.height);
    if workers <= 1 {
        paint_rows(None);
    } else if !paint_rows(Some(serial_budget)) {
        thread::scope(|scope| {
            // A worker that cannot be spawned leaves its rows to the remaining workers.
            let helpers: Vec<_> = (1..workers)
                .filter_map(|_| {
                    thread::Builder::new()
                        .spawn_scoped(scope, move || paint_rows(None))
                        .ok()
                })
                .collect();
            paint_rows(None);
            for helper in helpers {
                if let Err(payload) = helper.join() {
                    panic::resume_unwind(payload);
                }
            }
        });
    }
    let failure = queue
        .into_inner()
        .unwrap_or_else(PoisonError::into_inner)
        .failure;
    match failure {
        Some((_, error)) => Err(error),
        None => Ok(RasterImage {
            width: viewport.width,
            height: viewport.height,
            rgba,
        }),
    }
}

const MAX_RASTER_WORKERS: usize = 8;
const SERIAL_RASTER_BUDGET: Duration = Duration::from_micros(500);

#[cfg(test)]
thread_local! {
    static FORCED_WORKERS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Workers for a raster, and the time the calling thread paints alone before
/// helpers are worth their start-up cost.
fn schedule(rows: u32) -> (usize, Duration) {
    #[cfg(test)]
    if let Some(workers) = FORCED_WORKERS.with(std::cell::Cell::get) {
        return (workers, Duration::ZERO);
    }
    static AVAILABLE: OnceLock<usize> = OnceLock::new();
    let available = *AVAILABLE.get_or_init(|| {
        thread::available_parallelism()
            .map_or(1, NonZeroUsize::get)
            .min(MAX_RASTER_WORKERS)
    });
    (
        available.min(usize::try_from(rows).unwrap_or(usize::MAX)),
        SERIAL_RASTER_BUDGET,
    )
}

struct RowQueue<I> {
    rows: I,
    failure: Option<(u32, RasterError)>,
}

impl<'a, I: Iterator<Item = (&'a mut [u8], u32)>> RowQueue<I> {
    fn next(queue: &Mutex<Self>) -> Option<(&'a mut [u8], u32)> {
        let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.failure.is_some() {
            return None;
        }
        queue.rows.next()
    }

    // Rows are handed out in ascending order, so every row above a failure is
    // already claimed and the lowest failing row is the one a row-major pass
    // would report.
    fn fail(queue: &Mutex<Self>, y: u32, error: RasterError) {
        let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.failure.as_ref().is_none_or(|(first, _)| y < *first) {
            queue.failure = Some((y, error));
        }
    }
}

struct RowPainter<'a, U, S> {
    viewport: Viewport,
    samples_per_axis: u8,
    solid_color: &'a SrgbFallback,
    uniform: &'a U,
    sample: &'a S,
    decoded_color: Option<([f64; 3], [f64; 3])>,
    solid_pixel: Option<[u8; 4]>,
}

impl<U, S> RowPainter<'_, U, S>
where
    U: Fn(SampleBox) -> UniformRegion,
    S: Fn(PhysicalVector) -> Result<Option<SrgbFallback>, RasterError>,
{
    fn paint(&mut self, row: &mut [u8], y: u32) -> Result<(), RasterError> {
        for (pixel, x) in row.chunks_exact_mut(4).zip(0..) {
            pixel.copy_from_slice(&self.pixel(x, y)?);
        }
        Ok(())
    }

    fn pixel(&mut self, x: u32, y: u32) -> Result<[u8; 4], RasterError> {
        let viewport = self.viewport;
        let samples_per_axis = self.samples_per_axis;
        let grid = f64::from(samples_per_axis);
        let sample_count = grid * grid;
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
            match (self.uniform)(region) {
                UniformRegion::Clear => return Ok([0; 4]),
                UniformRegion::Solid => {
                    if let Some(pixel) = self.solid_pixel {
                        return Ok(pixel);
                    }
                    let pixel = constant_pixel(self.solid_color, samples_per_axis)?;
                    self.solid_pixel = Some(pixel);
                    return Ok(pixel);
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
                        + (f64::from(x) + (f64::from(sx) + 0.5) / grid) / viewport.pixels_per_unit,
                    y: viewport.origin.y
                        + (f64::from(y) + (f64::from(sy) + 0.5) / grid) / viewport.pixels_per_unit,
                };
                if let Some(color) = (self.sample)(point)? {
                    covered += 1;
                    let components = color.components();
                    let linear = match self.decoded_color {
                        Some((previous, linear)) if previous == components => linear,
                        _ => {
                            let linear =
                                srgb_to_linear_srgb(components).map_err(RasterError::Color)?;
                            self.decoded_color = Some((components, linear));
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
            return Ok([0; 4]);
        }
        let [r, g, b] = linear_srgb_to_srgb(linear_sum.map(|sum| sum / f64::from(covered)))
            .map_err(RasterError::Color)?
            .map(quantize);
        Ok([r, g, b, quantize(f64::from(covered) / sample_count)])
    }
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

/// Check the same sampling, precision and resource guards used by rendering,
/// without allocating or sampling a frame.
pub fn validate_viewport(viewport: Viewport, samples_per_axis: u8) -> Result<(), RasterError> {
    validate(viewport, samples_per_axis).map(|_| ())
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
