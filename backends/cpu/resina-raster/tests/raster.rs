use resina_model::PhysicalVector;
use resina_raster::{RasterError, Viewport, render_surface};
use resina_resolver::{OpaqueSurfaceIr, resolve_opaque_surface_source};
use serde_json::{Value, json};
#[cfg(feature = "png")]
use std::io::{self, Cursor, Write};

fn request() -> Value {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    request["surface"]["form"]["elevation"] = json!("base");
    request["appearance"]["bands"]["cast"]["highlightWidth"] = json!(0);
    request
}

fn surface() -> OpaqueSurfaceIr {
    resolve_opaque_surface_source(&request().to_string()).unwrap()
}

fn viewport(x: f64, y: f64, width: u32, height: u32, scale: f64) -> Viewport {
    Viewport {
        origin: PhysicalVector { x, y },
        width,
        height,
        pixels_per_unit: scale,
    }
}

#[test]
fn square_coverage_respects_origin_scale_and_row_order() {
    let image = render_surface(&surface(), viewport(-1.0, -1.0, 22, 14, 1.0), 2).unwrap();
    assert_eq!((image.width(), image.height()), (22, 14));
    assert_eq!(image.rgba().len(), 22 * 14 * 4);
    for y in 0..14 {
        for x in 0..22 {
            let pixel = &image.rgba()[(y * 22 + x) * 4..(y * 22 + x + 1) * 4];
            let outside = x == 0 || x == 21 || y == 0 || y == 13;
            let edge = x == 1 || x == 20 || y == 1 || y == 12;
            assert_eq!(
                pixel,
                if outside {
                    &[0, 0, 0, 0]
                } else if edge {
                    &[0, 0, 0, 255]
                } else {
                    &[255; 4]
                }
            );
        }
    }
    let scaled = render_surface(&surface(), viewport(0.0, 0.0, 40, 24, 2.0), 1).unwrap();
    assert!(
        scaled
            .rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
}

#[test]
fn pixel_ownership_transfer_preserves_straight_rgba_and_allocation() {
    let image = render_surface(&surface(), viewport(-0.5, 5.0, 1, 1, 1.0), 2).unwrap();
    let pointer = image.rgba().as_ptr();
    let rgba = image.into_rgba();
    assert_eq!(rgba.as_ptr(), pointer);
    assert_eq!(rgba, [0, 0, 0, 128]);
}

#[test]
fn color_filtering_averages_linear_intensity() {
    let image = render_surface(&surface(), viewport(0.0, 5.0, 11, 1, 0.5), 2).unwrap();
    let mut expected = vec![188, 188, 188, 255];
    expected.extend([255; 4].repeat(8));
    expected.extend([188, 188, 188, 255]);
    expected.extend([0; 4]);
    assert_eq!(image.rgba(), expected);
}

#[test]
fn partial_coverage_preserves_straight_color() {
    let mut request = request();
    let mut theme: Value =
        serde_json::from_str(request["theme"]["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["palette"]["black"]["$value"]["components"] = json!([0.22, 0.26, 0.32]);
    request["theme"]["themeSource"] = json!(theme.to_string());
    let ir = resolve_opaque_surface_source(&request.to_string()).unwrap();
    let image = render_surface(&ir, viewport(-0.5, 5.0, 1, 1, 1.0), 2).unwrap();
    assert_eq!(image.rgba(), &[56, 66, 82, 128]);
    let clear = render_surface(&ir, viewport(-2.0, -2.0, 1, 1, 1.0), 8).unwrap();
    assert_eq!(clear.rgba(), &[0; 4]);
}

#[test]
fn invalid_viewports_and_resource_limits_fail_before_rendering() {
    let ir = surface();
    for viewport in [
        viewport(0.0, 0.0, 0, 1, 1.0),
        viewport(0.0, 0.0, 1, 0, 1.0),
        viewport(f64::NAN, 0.0, 1, 1, 1.0),
        viewport(0.0, f64::INFINITY, 1, 1, 1.0),
        viewport(0.0, 0.0, 1, 1, 0.0),
        viewport(0.0, 0.0, 1, 1, -1.0),
        viewport(0.0, 0.0, 1, 1, f64::NAN),
        viewport(0.0, 0.0, 1, 1, f64::INFINITY),
        viewport(0.0, 0.0, 1, 1, f64::MIN_POSITIVE / 4.0),
        viewport(1e20, 0.0, 1, 1, 1.0),
        viewport(0.0, -1e20, 1, 1, 1.0),
    ] {
        assert!(
            matches!(
                render_surface(&ir, viewport, 2),
                Err(RasterError::InvalidViewport(_))
            ),
            "{viewport:?}"
        );
    }
    for (width, height, samples) in [(u32::MAX, u32::MAX, 8), (4_194_305, 1, 1), (2048, 2048, 3)] {
        assert!(matches!(
            render_surface(&ir, viewport(0.0, 0.0, width, height, 1.0), samples),
            Err(RasterError::ResourceLimit)
        ));
    }
    for samples in [0, 9, u8::MAX] {
        assert!(matches!(
            render_surface(&ir, viewport(0.0, 0.0, 1, 1, 1.0), samples),
            Err(RasterError::InvalidSampling)
        ));
    }
}

#[test]
fn representable_subnormal_coordinates_are_supported() {
    let image = render_surface(&surface(), viewport(0.0, 0.0, 1, 1, f64::MAX), 2).unwrap();
    assert_eq!(image.rgba(), &[0, 0, 0, 255]);
}

#[test]
fn all_material_families_preserve_circular_coverage() {
    for role in [
        "surface.base",
        "surface.chrome",
        "control.primary",
        "surface.transient",
    ] {
        let mut request = request();
        request["size"] = json!({"width": 8, "height": 8});
        request["surface"]["form"]["shape"] = json!("capsule");
        request["surface"]["materialRole"] = json!(role);
        let ir = resolve_opaque_surface_source(&request.to_string()).unwrap();
        let image = render_surface(&ir, viewport(0.0, 0.0, 8, 8, 1.0), 8).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                let mut covered = 0;
                for sy in 0..8 {
                    for sx in 0..8 {
                        let dx = x as f64 + (f64::from(sx) + 0.5) / 8.0 - 4.0;
                        let dy = y as f64 + (f64::from(sy) + 0.5) / 8.0 - 4.0;
                        covered += u32::from(dx * dx + dy * dy <= 16.0);
                    }
                }
                assert_eq!(
                    image.rgba()[(y * 8 + x) * 4 + 3],
                    (f64::from(covered) * 255.0 / 64.0).round() as u8,
                    "{role} ({x},{y})"
                );
            }
        }
    }
}

#[test]
#[cfg(feature = "png")]
fn png_preserves_pixels_dimensions_and_srgb_metadata() {
    let image = render_surface(&surface(), viewport(-0.5, 5.0, 2, 1, 1.0), 2).unwrap();
    let mut png = Vec::new();
    image.write_png(&mut png).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
    assert_eq!(
        reader.info().srgb,
        Some(png::SrgbRenderingIntent::Perceptual)
    );
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut decoded).unwrap();
    assert_eq!((frame.width, frame.height), (2, 1));
    assert_eq!(&decoded[..frame.buffer_size()], image.rgba());
}

#[cfg(feature = "png")]
struct FailedWriter;
#[cfg(feature = "png")]
impl Write for FailedWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("output unavailable"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
#[cfg(feature = "png")]
fn png_writer_failure_is_reported() {
    let image = render_surface(&surface(), viewport(2.0, 2.0, 1, 1, 1.0), 1).unwrap();
    assert!(
        image
            .write_png(FailedWriter)
            .unwrap_err()
            .to_string()
            .contains("output unavailable")
    );
}
