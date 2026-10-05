use crate::{QmlError, check_coordinates};
use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport};
use resina_resolver::SurfacePaintIr;
use std::{error::Error, fmt, fmt::Write};

#[derive(Debug)]
pub enum PaintQmlError {
    InvalidScale,
    UnrepresentableBounds,
    Coordinates(QmlError),
    Raster(RasterError),
    Encoding(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for PaintQmlError {
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

impl Error for PaintQmlError {
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
) -> Result<String, PaintQmlError> {
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(PaintQmlError::InvalidScale);
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
    let left = ((bounds.x + placement.x) * device_scale).floor();
    let top = ((bounds.y + placement.y) * device_scale).floor();
    let right = ((bounds.x + bounds.width + placement.x) * device_scale).ceil();
    let bottom = ((bounds.y + bounds.height + placement.y) * device_scale).ceil();
    let width = right - left;
    let height = bottom - top;
    if ![left, top, right, bottom, width, height]
        .iter()
        .all(|v| v.is_finite())
        || width < 1.0
        || height < 1.0
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return Err(PaintQmlError::UnrepresentableBounds);
    }
    let origin_x = left / device_scale;
    let origin_y = top / device_scale;
    let logical_width = width / device_scale;
    let logical_height = height / device_scale;
    check_coordinates([
        origin_x,
        origin_y,
        logical_width,
        logical_height,
        right / device_scale,
        bottom / device_scale,
    ])
    .map_err(PaintQmlError::Coordinates)?;
    for (expected, native) in [
        (right / device_scale, origin_x as f32 + logical_width as f32),
        (
            bottom / device_scale,
            origin_y as f32 + logical_height as f32,
        ),
    ] {
        if (expected - f64::from(native)).abs() > crate::MAX_COORDINATE_ERROR {
            return Err(PaintQmlError::Coordinates(QmlError::CoordinatePrecision(
                expected,
            )));
        }
    }
    let image = resina_raster::render_surface_paint(
        ir,
        Viewport {
            origin: PhysicalVector {
                x: origin_x - placement.x,
                y: origin_y - placement.y,
            },
            width: width as u32,
            height: height as u32,
            pixels_per_unit: device_scale,
        },
        samples_per_axis,
    )
    .map_err(PaintQmlError::Raster)?;
    let mut png = Vec::new();
    image
        .write_png(&mut png)
        .map_err(|error| PaintQmlError::Encoding(Box::new(error)))?;
    let mut output = format!(
        r#"import QtQuick

Item {{
    readonly property real paintOriginX: {origin_x}
    readonly property real paintOriginY: {origin_y}
    readonly property real preparedDeviceScale: {device_scale}
    implicitWidth: {logical_width}
    implicitHeight: {logical_height}
    focus: false
    activeFocusOnTab: false
    Accessible.ignored: true
    readonly property bool paintReady: paint.status === Image.Ready
    Image {{
        id: paint
        Accessible.ignored: true
        width: parent.implicitWidth
        height: parent.implicitHeight
        fillMode: Image.Stretch
        smooth: false
        mipmap: false
        asynchronous: false
        source: "data:image/png,"#
    );
    for byte in png {
        write!(output, "%{byte:02X}").expect("string formatting");
    }
    output.push_str("\"\n    }\n}\n");
    Ok(output)
}
