use resina_model::PhysicalVector;
use resina_raster::{RasterImage, Viewport, render_focus, render_surface};
use resina_resolver::{resolve_focus_ir_source, resolve_opaque_surface_source};
use serde::Deserialize;
use std::{error::Error, hint::black_box, process::ExitCode, time::Instant};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bundle {
    schema_version: String,
    capture: Capture,
    scenarios: Vec<Scene>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Capture {
    origin: PhysicalVector,
    width: u32,
    height: u32,
    pixels_per_unit: f64,
    samples_per_axis: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scene {
    name: String,
    kind: Kind,
    #[serde(rename = "expectedMaterialFamily")]
    _material_family: String,
    request: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    OpaqueSurface,
    FocusRing,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("paint benchmark: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments
        .next()
        .ok_or("usage: paint <scene-bundle> <scene-name>")?;
    let name = arguments
        .next()
        .ok_or("usage: paint <scene-bundle> <scene-name>")?;
    // Cargo appends this flag when running a benchmark without the test harness.
    if let Some(flag) = arguments.next()
        && (flag != "--bench" || arguments.next().is_some())
    {
        return Err("usage: paint <scene-bundle> <scene-name>".into());
    }
    let name = name.to_str().ok_or("scene name must be UTF-8")?;
    let bundle: Bundle = serde_json::from_slice(&std::fs::read(path)?)?;
    if bundle.schema_version != "0.1.0" {
        return Err("scene bundle schemaVersion must be 0.1.0".into());
    }
    if !bundle.capture.pixels_per_unit.is_finite() || bundle.capture.pixels_per_unit <= 0.0 {
        return Err("capture pixelsPerUnit must be finite and positive".into());
    }
    let mut matches = bundle.scenarios.iter().filter(|scene| scene.name == name);
    let scene = matches.next().ok_or("scene name not found")?;
    if matches.next().is_some() {
        return Err("scene name is ambiguous".into());
    }
    let source = scene.request.to_string();
    match scene.kind {
        Kind::OpaqueSurface => {
            let ir = resolve_opaque_surface_source(&source)?;
            measure(name, &bundle.capture, |viewport| {
                render_surface(&ir, viewport, bundle.capture.samples_per_axis)
            })
        }
        Kind::FocusRing => {
            let ir = resolve_focus_ir_source(&source)?;
            measure(name, &bundle.capture, |viewport| {
                render_focus(&ir, viewport, bundle.capture.samples_per_axis)
            })
        }
    }
}

fn dimension(pixels: u32, original_scale: f64, scale: f64) -> Result<u32, Box<dyn Error>> {
    let scaled = (f64::from(pixels) / original_scale * scale).ceil();
    if !scaled.is_finite() || scaled < 1.0 || scaled > f64::from(u32::MAX) {
        return Err("scaled capture dimension exceeds positive u32 range".into());
    }
    Ok(scaled as u32)
}

fn measure(
    name: &str,
    capture: &Capture,
    mut render: impl FnMut(Viewport) -> Result<RasterImage, resina_raster::RasterError>,
) -> Result<(), Box<dyn Error>> {
    for scale in [1.0, 1.25, 2.0, 3.0] {
        let viewport = Viewport {
            origin: capture.origin,
            width: dimension(capture.width, capture.pixels_per_unit, scale)?,
            height: dimension(capture.height, capture.pixels_per_unit, scale)?,
            pixels_per_unit: scale,
        };
        black_box(render(viewport)?);
        let mut milliseconds = [0.0; 9];
        for elapsed in &mut milliseconds {
            let start = Instant::now();
            let image = render(black_box(viewport))?;
            *elapsed = start.elapsed().as_secs_f64() * 1000.0;
            black_box(image);
        }
        milliseconds.sort_by(f64::total_cmp);
        println!(
            "{name} scale={scale} {}x{} samples={} runs=9 min_ms={:.3} median_ms={:.3} max_ms={:.3}",
            viewport.width,
            viewport.height,
            capture.samples_per_axis,
            milliseconds[0],
            milliseconds[4],
            milliseconds[8],
        );
    }
    Ok(())
}
