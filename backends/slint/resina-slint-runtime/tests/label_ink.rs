#![cfg(feature = "testing")]

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, PhysicalSize, fontique_011::fontique};
use std::rc::Rc;

slint::slint! {
    export component InkWindow inherits Window {
        width: 400px;
        height: 240px;
        background: white;
        in property <string> family;
        out property <length> advance: bounded.preferred-width;
        bounded := Text {
            x: 80px; y: 40px;
            width: self.preferred-width; height: self.preferred-height;
            text: "j"; font-family: root.family; font-size: 96px;
            horizontal-alignment: center; vertical-alignment: top;
            wrap: no-wrap; overflow: clip; color: black;
            accessible-role: none;
        }
        Text {
            x: 200px; y: 40px;
            width: bounded.preferred-width + 40px; height: bounded.preferred-height;
            text: "j"; font-family: root.family; font-size: 96px;
            horizontal-alignment: center; vertical-alignment: top;
            wrap: no-wrap; overflow: clip; color: black;
            accessible-role: none;
        }
    }
}

struct SoftwarePlatform;
impl Platform for SoftwarePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }
}

#[test]
fn native_advance_box_ink_diagnostic() {
    slint::platform::set_platform(Box::new(SoftwarePlatform)).unwrap();
    let ui = InkWindow::new().unwrap();
    let bytes =
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap();
    let mut collection = slint::fontique_011::shared_collection();
    let fonts = collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(bytes)), None);
    let (family, _) = fonts.first().expect("test font must be registered");
    ui.set_family(collection.family_name(*family).unwrap().into());
    ui.window()
        .dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    ui.window().set_size(PhysicalSize::new(400, 240));
    ui.show().unwrap();
    let image = ui.window().take_snapshot().unwrap();
    assert_eq!((image.width(), image.height()), (400, 240));
    assert_eq!(ui.window().scale_factor(), 1.0);
    let pixel = |x: usize, y: usize| &image.as_bytes()[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
    let mut differences = 0;
    let mut reference_overhang = 0;
    let advance = ui.get_advance();
    assert!(advance.is_finite() && advance > 0.0 && advance < 60.0);
    let bounded_left = 80;
    let reference_left = 220;
    let reference_offset = reference_left - bounded_left;
    for y in 0..240 {
        for x in bounded_left - 20..bounded_left + 60 {
            differences += usize::from(pixel(x, y) != pixel(x + reference_offset, y));
            if x >= bounded_left && ((x - bounded_left) as f32) < advance {
                assert_eq!(
                    pixel(x, y),
                    pixel(x + reference_offset, y),
                    "glyph position must stay identical inside its advance box"
                );
            }
            if x < bounded_left && pixel(x + reference_offset, y)[..3] != [255, 255, 255] {
                assert_eq!(
                    pixel(x, y),
                    [255, 255, 255, 255],
                    "bounded Text crops reference ink outside its advance"
                );
                reference_overhang += 1;
            }
        }
    }
    println!(
        "advance={}, differing pixels={differences}, reference left overhang={reference_overhang}",
        ui.get_advance()
    );
    if let Some(directory) = std::env::var_os("RESINA_CAPTURE_DIR") {
        use std::io::Write;
        std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(std::path::Path::new(&directory).join("label-ink-400x240.rgba"))
            .unwrap()
            .write_all(image.as_bytes())
            .unwrap();
    }
    ui.hide().unwrap();
    assert!(
        reference_overhang > 0,
        "fixture must have actual ink outside the advance box"
    );
    assert!(
        differences > 0,
        "advance-sized Text must expose the observed clipping limitation"
    );
    assert_eq!(
        differences, reference_overhang,
        "all differences must be cropped overhang"
    );
}
