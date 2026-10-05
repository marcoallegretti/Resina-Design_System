use resina_model::PhysicalVector;
use resina_qml::{PaintQmlError, render_surface_paint};
use resina_resolver::resolve_surface_paint_source;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn request(focused: bool) -> String {
    let mut body: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/opaque-surface-request.json"
    ))
    .unwrap();
    body["surface"]["states"]["states"] = serde_json::json!(if focused {
        vec!["focused"]
    } else {
        vec!["rest"]
    });
    serde_json::json!({"schemaVersion":"0.1.0","body":body,"surroundingColor":{"colorSpace":"srgb","components":[0,0,0],"alpha":1}}).to_string()
}

fn image(qml: &str) -> (u32, u32, Vec<u8>) {
    let encoded = qml
        .split("data:image/png,")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(encoded.len() % 3, 0);
    let bytes: Vec<u8> = encoded
        .as_bytes()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|chunk| {
            assert_eq!(chunk[0], b'%');
            u8::from_str_radix(std::str::from_utf8(&chunk[1..]).unwrap(), 16).unwrap()
        })
        .collect();
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    assert!(reader.info().srgb.is_some());
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    pixels.truncate(frame.buffer_size());
    (frame.width, frame.height, pixels)
}

#[test]
fn complete_paint_reserves_focus_and_samples_after_fractional_placement() {
    for focused in [false, true] {
        let ir = resolve_surface_paint_source(&request(focused)).unwrap();
        for scale in [0.5, 1.0, 1.25, 2.0] {
            let placement = PhysicalVector { x: 13.3, y: 17.1 };
            let qml = render_surface_paint(&ir, placement, scale, 4).unwrap();
            let number = |name: &str| {
                qml.lines()
                    .find_map(|line| {
                        line.trim()
                            .strip_prefix(&format!("readonly property real {name}: "))
                    })
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            let origin = PhysicalVector {
                x: number("paintOriginX") - placement.x,
                y: number("paintOriginY") - placement.y,
            };
            let (width, height, pixels) = image(&qml);
            let expected = resina_raster::render_surface_paint(
                &ir,
                resina_raster::Viewport {
                    origin,
                    width,
                    height,
                    pixels_per_unit: scale,
                },
                4,
            )
            .unwrap();
            assert_eq!(pixels, expected.rgba());
            let bounds = if focused {
                ir.focus()
                    .unwrap()
                    .geometry()
                    .outer()
                    .contour()
                    .bounds()
                    .unwrap()
            } else {
                ir.body().geometry().silhouette().bounds().unwrap()
            };
            assert!(width as f64 / scale >= bounds.width);
            assert!(height as f64 / scale >= bounds.height);
            assert_eq!(number("preparedDeviceScale"), scale);
            assert_eq!(qml.matches("Accessible.ignored: true").count(), 2);
            assert!(qml.contains("smooth: false"));
            assert_eq!(qml, render_surface_paint(&ir, placement, scale, 4).unwrap());
        }
    }
}

#[test]
fn unplaced_focus_reserves_negative_origins_and_complete_navigation_pixels() {
    let ir = resolve_surface_paint_source(&request(true)).unwrap();
    let qml = std::thread::spawn(|| {
        let ir = resolve_surface_paint_source(&request(true)).unwrap();
        render_surface_paint(&ir, PhysicalVector { x: 0.0, y: 0.0 }, 1.25, 4)
    })
    .join()
    .unwrap()
    .unwrap();
    assert!(qml.contains("paintOriginX: -4"));
    assert!(qml.contains("paintOriginY: -4"));
    let (width, height, pixels) = image(&qml);
    let expected = resina_raster::render_surface_paint(
        &ir,
        resina_raster::Viewport {
            origin: PhysicalVector { x: -4.0, y: -4.0 },
            width,
            height,
            pixels_per_unit: 1.25,
        },
        4,
    )
    .unwrap();
    assert_eq!(pixels, expected.rgba());
    assert!(pixels.as_chunks::<4>().0.iter().any(|pixel| pixel[3] == 0));
    assert!(
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] == 255)
    );
}

#[test]
fn invalid_preparation_fails_without_a_component() {
    let ir = resolve_surface_paint_source(&request(true)).unwrap();
    let origin = PhysicalVector { x: 0.0, y: 0.0 };
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            render_surface_paint(&ir, origin, scale, 4),
            Err(PaintQmlError::InvalidScale)
        ));
    }
    assert!(matches!(
        render_surface_paint(
            &ir,
            PhysicalVector {
                x: f64::INFINITY,
                y: 0.0
            },
            1.0,
            4
        ),
        Err(PaintQmlError::UnrepresentableBounds)
    ));
    assert!(matches!(
        render_surface_paint(&ir, PhysicalVector { x: 1e20, y: 0.0 }, 1.0, 4),
        Err(PaintQmlError::UnrepresentableBounds | PaintQmlError::Coordinates(_))
    ));
    for samples in [0, 9] {
        assert!(matches!(
            render_surface_paint(&ir, origin, 1.0, samples),
            Err(PaintQmlError::Raster(
                resina_raster::RasterError::InvalidSampling
            ))
        ));
    }
    assert!(render_surface_paint(&ir, origin, 10_000.0, 4).is_err());
}

#[test]
fn command_publishes_only_a_complete_valid_component() {
    let valid = request(true);
    let oversized = vec![b' '; 1024 * 1024 + 1];
    for (input, accepted) in [
        (valid.as_bytes(), true),
        (b"{}".as_slice(), false),
        (b"\xff".as_slice(), false),
        (oversized.as_slice(), false),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-paint-qml"))
            .args(["-", "1.25", "4", "13.3", "17.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.success(), accepted);
        if accepted {
            assert!(result.stderr.is_empty());
            let expected = render_surface_paint(
                &resolve_surface_paint_source(&valid).unwrap(),
                PhysicalVector { x: 13.3, y: 17.1 },
                1.25,
                4,
            )
            .unwrap();
            assert_eq!(result.stdout, expected.as_bytes());
        } else {
            assert_eq!(result.status.code(), Some(1));
            assert!(result.stdout.is_empty());
            assert!(!result.stderr.is_empty());
        }
    }
    let usage = Command::new(env!("CARGO_BIN_EXE_resina-paint-qml"))
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
}
