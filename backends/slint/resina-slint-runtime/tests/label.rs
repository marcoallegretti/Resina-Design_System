#![cfg(feature = "testing")]

use resina_environment::{LayoutDirection, SafeArea};
use resina_model::{SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelInput, LabelMeasureInput, ResolvedTypography, compile_theme_source,
    resolve_command_label,
};
use resina_slint_runtime::{LabelMeasureError, LabelMeasurer};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, PhysicalSize, fontique_011::fontique};
use std::rc::Rc;

slint::slint! {
    import { ResinaLabelMeasure, ResinaLabelRequest } from "../resina-label-measure.slint";
    export component MeasureWindow inherits Window {
        width: 320px;
        height: 240px;
        background: white;
        in property <ResinaLabelRequest> request;
        out property <length> measured-width: measurement.measured-width;
        out property <length> measured-height: measurement.measured-height;
        out property <length> minimum-required-width: measurement.minimum-required-width;
        measurement := ResinaLabelMeasure { request: root.request; }
    }

}

struct SoftwarePlatform;
impl Platform for SoftwarePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }
}

fn style(scale: f64, tracking: f64, weight: f64, line: f64) -> ResolvedTypography {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/command-motion-request.json"
    ))
    .unwrap();
    let source = &request["surface"]["body"]["theme"];
    let mut theme: serde_json::Value =
        serde_json::from_str(source["themeSource"].as_str().unwrap()).unwrap();
    theme["tokens"]["type"]["tracking"]["$value"]["value"] = serde_json::json!(tracking);
    theme["tokens"]["type"]["weight"]["$value"] = serde_json::json!(weight);
    theme["tokens"]["type"]["line"]["$value"] = serde_json::json!(line);
    let mut environment = source["environment"].clone();
    environment["textScale"] = serde_json::json!(scale);
    compile_theme_source(&theme.to_string())
        .unwrap()
        .resolve(&serde_json::from_value(environment).unwrap())
        .unwrap()
        .typography()[&TypographyRole::Label]
        .clone()
}

fn measure(
    ui: &MeasureWindow,
    measurer: &mut LabelMeasurer<'_>,
    family: &str,
    input: LabelMeasureInput<'_>,
) -> Result<SurfaceSize, LabelMeasureError> {
    let text = input.text;
    let native = measurer.prepare(family, input)?;
    assert_eq!(native.device_scale, ui.window().scale_factor());
    assert_eq!(native.text.as_str(), text);
    ui.set_request(ResinaLabelRequest {
        text: native.text.clone(),
        font_family: native.font_family.clone(),
        font_size: native.font_size,
        font_weight: native.font_weight,
        letter_spacing: native.letter_spacing,
        line_height_factor: native.line_height_factor,
        constrained: native.maximum_width.is_some(),
        wrapping_width: native.maximum_width.unwrap_or(0.0),
    });
    native.complete(
        ui.get_measured_width(),
        ui.get_measured_height(),
        ui.get_minimum_required_width(),
    )
}

#[test]
fn actual_font_measurement_preserves_complete_text_and_typography() {
    slint::platform::set_platform(Box::new(SoftwarePlatform)).unwrap();
    let ui = MeasureWindow::new().unwrap();
    let bytes =
        std::fs::read(std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required"))
            .unwrap();
    let mut collection = slint::fontique_011::shared_collection();
    let fonts = collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(bytes)), None);
    let (id, infos) = fonts.first().expect("test font must be registered");
    assert!(!infos.is_empty());
    let family = collection.family_name(*id).unwrap().to_owned();
    let mut measurer = LabelMeasurer::new(ui.window());
    let expansion: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/content/command-label-expansion.json"
    ))
    .unwrap();
    let mut texts: Vec<_> = expansion["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["text"].as_str().unwrap().to_owned(),
                LayoutDirection::Ltr,
            )
        })
        .collect();
    texts.extend([
        ("Save".to_owned(), LayoutDirection::Ltr),
        (
            "Verbindung erneut herstellen".to_owned(),
            LayoutDirection::Ltr,
        ),
        ("إعادة الاتصال بالشبكة".to_owned(), LayoutDirection::Rtl),
        ("Save\nall\nchanges".to_owned(), LayoutDirection::Ltr),
    ]);
    let mut cases = 0;
    for device_scale in [0.5_f32, 1.0, 1.25, 2.0, 1.3] {
        ui.window().dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: device_scale,
        });
        ui.window().set_size(PhysicalSize::new(
            (320.0 * device_scale).round() as u32,
            (240.0 * device_scale).round() as u32,
        ));
        for text_scale in [1.0, 1.5, 2.0] {
            for tracking in [-0.5, 0.0, 1.0] {
                let typography = style(text_scale, tracking, 400.0, 1.4);
                for (text, direction) in &texts {
                    let label = resolve_command_label(
                        CommandLabelInput {
                            text,
                            typography: &typography,
                            minimum_size: SurfaceSize {
                                width: 64.0,
                                height: 44.0,
                            },
                            maximum_size: SurfaceSize {
                                width: 320.0,
                                height: 600.0,
                            },
                            padding: SafeArea {
                                start: 16.0,
                                end: 12.0,
                                top: 8.0,
                                bottom: 8.0,
                            },
                            direction: *direction,
                        },
                        |input| measure(&ui, &mut measurer, &family, input),
                    )
                    .unwrap();
                    assert_eq!(label.text(), text);
                    assert_eq!(label.layout_direction(), *direction);
                    let fitted = measure(
                        &ui,
                        &mut measurer,
                        &family,
                        LabelMeasureInput {
                            text,
                            typography: &typography,
                            maximum_width: Some(label.label_bounds().width),
                        },
                    )
                    .unwrap();
                    assert_eq!(fitted.height, label.label_bounds().height);
                    assert!(fitted.width <= label.label_bounds().width);
                    assert_eq!(
                        label.label_bounds().x,
                        if *direction == LayoutDirection::Ltr {
                            16.0
                        } else {
                            12.0
                        }
                    );
                    if text == "Save\nall\nchanges" {
                        let expected =
                            (typography.font_size() * typography.line_height() * 3.0).ceil();
                        assert!(
                            (fitted.height - expected).abs() <= 1.0,
                            "three complete line boxes: {fitted:?}, expected {expected}"
                        );
                    }
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 315);
    let natural = |measurer: &mut LabelMeasurer<'_>, typography: &ResolvedTypography| {
        measure(
            &ui,
            measurer,
            &family,
            LabelMeasureInput {
                text: "MMMMMMMM",
                typography,
                maximum_width: None,
            },
        )
        .unwrap()
    };
    let tight = natural(&mut measurer, &style(1.0, -0.5, 400.0, 1.4));
    let loose = natural(&mut measurer, &style(1.0, 1.5, 400.0, 1.4));
    assert!(
        loose.width - tight.width >= 10.0,
        "tracking must reach native layout"
    );
    let base = natural(&mut measurer, &style(1.0, 0.0, 400.0, 1.4));
    let enlarged = natural(&mut measurer, &style(2.0, 0.0, 400.0, 1.4));
    assert!((enlarged.width - 2.0 * base.width).abs() <= 2.0);
    assert!((enlarged.height - 2.0 * base.height).abs() <= 2.0);
    let paragraph_style = style(1.0, 0.0, 400.0, 1.4);
    for (text, lines) in [
        ("\nSave", 2.0),
        ("Save\n", 2.0),
        ("Save\n\nchanges", 3.0),
        ("Save\r\nall\r\nchanges", 3.0),
    ] {
        let extent = measure(
            &ui,
            &mut measurer,
            &family,
            LabelMeasureInput {
                text,
                typography: &paragraph_style,
                maximum_width: Some(200.0),
            },
        )
        .unwrap();
        let expected = (paragraph_style.font_size() * paragraph_style.line_height() * lines).ceil();
        assert!(
            (extent.height - expected).abs() <= 1.0,
            "{text:?}: {extent:?}, expected {expected}"
        );
    }
    for weight in [100.0, 600.0, 900.0] {
        let typography = style(1.5, 0.5, weight, 1.4);
        let measured = measure(
            &ui,
            &mut measurer,
            &family,
            LabelMeasureInput {
                text: "Save\nall\nchanges",
                typography: &typography,
                maximum_width: Some(200.0),
            },
        )
        .unwrap();
        assert!(
            (measured.height - (typography.font_size() * typography.line_height() * 3.0).ceil())
                .abs()
                <= 1.0
        );
    }
    let typography = style(1.0, 0.0, 400.0, 1.4);
    let input = || LabelMeasureInput {
        text: "Save",
        typography: &typography,
        maximum_width: None,
    };
    assert!(matches!(
        measurer.prepare("Missing Font Family 87324", input()),
        Err(LabelMeasureError::UnknownFamily(_))
    ));
    assert!(matches!(
        measurer.prepare(
            &family,
            LabelMeasureInput {
                text: " ",
                ..input()
            }
        ),
        Err(LabelMeasureError::BlankText)
    ));
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            measurer.prepare(
                &family,
                LabelMeasureInput {
                    maximum_width: Some(width),
                    ..input()
                }
            ),
            Err(LabelMeasureError::Precision("wrap width"))
        ));
    }
    let fractional = style(1.0, 0.0, 400.5, 1.4);
    assert!(matches!(
        measurer.prepare(
            &family,
            LabelMeasureInput {
                typography: &fractional,
                ..input()
            }
        ),
        Err(LabelMeasureError::FontWeight)
    ));
    let rounded = measurer
        .prepare(
            &family,
            LabelMeasureInput {
                maximum_width: Some(50.123456789),
                ..input()
            },
        )
        .unwrap();
    assert!(f64::from(rounded.maximum_width.unwrap()) <= 50.123456789);
    assert!(matches!(
        measure(
            &ui,
            &mut measurer,
            &family,
            LabelMeasureInput {
                maximum_width: Some(1.0),
                ..input()
            }
        ),
        Err(LabelMeasureError::Wrapping { .. })
    ));
    assert!(matches!(
        measure(
            &ui,
            &mut measurer,
            &family,
            LabelMeasureInput {
                text: "Supercalifragilisticexpialidocious",
                maximum_width: Some(20.0),
                ..input()
            }
        ),
        Err(LabelMeasureError::Wrapping { .. })
    ));
    assert!(matches!(
        rounded.complete(0.0, 10.0, 1.0),
        Err(LabelMeasureError::Extent)
    ));
    assert!(matches!(
        rounded.complete(51.0, 10.0, 1.0),
        Err(LabelMeasureError::Extent)
    ));
    assert!(matches!(
        rounded.complete(10.0, f32::NAN, 1.0),
        Err(LabelMeasureError::Extent)
    ));
    assert!(matches!(
        rounded.complete(10.0, 10.0, 20.0),
        Err(LabelMeasureError::Extent)
    ));
    for offered in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut changed = rounded.clone();
        changed.maximum_width = Some(offered);
        assert!(matches!(
            changed.complete(10.0, 10.0, 1.0),
            Err(LabelMeasureError::Extent)
        ));
    }
    assert!(
        measure(
            &ui,
            &mut measurer,
            &family,
            LabelMeasureInput {
                text: "Save\nall\nchanges",
                maximum_width: None,
                ..input()
            }
        )
        .unwrap()
        .height
            > 2.0 * typography.font_size()
    );
    ui.show().unwrap();
    let capture = ui.window().take_snapshot().unwrap();
    assert!(
        capture
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [255, 255, 255, 255]),
        "measurement component must not paint or promise label rendering"
    );
    ui.hide().unwrap();
}
