use resina_model::PhysicalVector;
use resina_resolver::resolve_surface_paint_source;
use resina_slint::{PaintSlintError, render_surface_paint};
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

fn image(slint: &str) -> (u32, u32, Vec<u8>) {
    let encoded = slint
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
            let slint = render_surface_paint(&ir, placement, scale, 4).unwrap();
            let number = |name: &str| {
                slint
                    .lines()
                    .find_map(|line| {
                        line.trim()
                            .strip_prefix(&format!("out property <length> {name}: "))
                    })
                    .unwrap()
                    .trim_end_matches("px;")
                    .trim_end_matches(';')
                    .parse::<f64>()
                    .unwrap()
            };
            let origin = PhysicalVector {
                x: number("paint-origin-x") - placement.x,
                y: number("paint-origin-y") - placement.y,
            };
            let (width, height, pixels) = image(&slint);
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
            assert!(slint.contains(&format!(
                "out property <float> prepared-device-scale: {scale};"
            )));
            assert_eq!(slint.matches("accessible-role: none").count(), 2);
            assert!(slint.contains("image-rendering: pixelated"));
            assert!(slint.contains("x: 0px;"));
            assert_eq!(
                slint,
                render_surface_paint(&ir, placement, scale, 4).unwrap()
            );
        }
    }
}

#[test]
fn unplaced_focus_reserves_negative_origins_and_complete_navigation_pixels() {
    let ir = resolve_surface_paint_source(&request(true)).unwrap();
    let slint = std::thread::spawn(|| {
        let ir = resolve_surface_paint_source(&request(true)).unwrap();
        render_surface_paint(&ir, PhysicalVector { x: 0.0, y: 0.0 }, 1.25, 4)
    })
    .join()
    .unwrap()
    .unwrap();
    assert!(slint.contains("paint-origin-x: -4"));
    assert!(slint.contains("paint-origin-y: -4"));
    let (width, height, pixels) = image(&slint);
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
            Err(PaintSlintError::InvalidScale)
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
        Err(PaintSlintError::UnrepresentableBounds)
    ));
    assert!(matches!(
        render_surface_paint(&ir, PhysicalVector { x: 1e20, y: 0.0 }, 1.0, 4),
        Err(PaintSlintError::UnrepresentableBounds | PaintSlintError::Coordinates(_))
    ));
    for samples in [0, 9] {
        assert!(matches!(
            render_surface_paint(&ir, origin, 1.0, samples),
            Err(PaintSlintError::Raster(
                resina_raster::RasterError::InvalidSampling
            ))
        ));
    }
    assert!(render_surface_paint(&ir, origin, 10_000.0, 4).is_err());
}

#[test]
fn cumulative_native_corner_precision_is_checked_before_publishing() {
    let ir = resolve_surface_paint_source(&request(false)).unwrap();
    let bounds = ir.body().geometry().silhouette().bounds().unwrap();
    let origin = 40_001.0 / 1.25;
    let extent = 26.0 / 1.25;
    let edge = 40_027.0 / 1.25;
    for value in [origin, extent, edge] {
        assert!((value - f64::from(value as f32)).abs() <= 1.0 / 1024.0);
    }
    assert!((edge - f64::from(origin as f32 + extent as f32)).abs() > 1.0 / 1024.0);
    assert!(matches!(
        render_surface_paint(
            &ir,
            PhysicalVector { x: 40_001.125 / 1.25 - bounds.x, y: 0.0 },
            1.25,
            4,
        ),
        Err(PaintSlintError::Coordinates(resina_slint::SlintError::CoordinatePrecision(value))) if value == edge
    ));
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_resina-paint-slint"))
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
    let path = std::env::temp_dir().join(format!(
        "resina-slint-paint-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(valid.as_bytes()).unwrap();
    drop(file);
    let result = Command::new(env!("CARGO_BIN_EXE_resina-paint-slint"))
        .arg(&path)
        .args(["1.25", "4", "13.3", "17.1"])
        .output();
    std::fs::remove_file(&path).unwrap();
    let result = result.unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    assert_eq!(
        result.stdout,
        render_surface_paint(
            &resolve_surface_paint_source(&valid).unwrap(),
            PhysicalVector { x: 13.3, y: 17.1 },
            1.25,
            4,
        )
        .unwrap()
        .as_bytes()
    );
    let directory = Command::new(env!("CARGO_BIN_EXE_resina-paint-slint"))
        .args([
            concat!(env!("CARGO_MANIFEST_DIR"), "/src"),
            "1.25",
            "4",
            "13.3",
            "17.1",
        ])
        .output()
        .unwrap();
    assert_eq!(directory.status.code(), Some(1));
    assert!(directory.stdout.is_empty());
    let diagnostic = String::from_utf8(directory.stderr).unwrap();
    assert!(diagnostic.contains("cannot read request") && diagnostic.contains("/src"));
    for (index, name) in [
        (1, "device scale"),
        (2, "samples per axis"),
        (3, "origin x"),
        (4, "origin y"),
    ] {
        let mut args = ["-", "1.25", "4", "13.3", "17.1"];
        args[index] = "invalid";
        let result = Command::new(env!("CARGO_BIN_EXE_resina-paint-slint"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8(result.stderr).unwrap().contains(name));
    }
    let usage = Command::new(env!("CARGO_BIN_EXE_resina-paint-slint"))
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());
}
