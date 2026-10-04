#![cfg(feature = "testing")]

use guido::widgets::font::FontFamily;
use resina_environment::SafeArea;
use resina_guido::{LabelMeasureError, measure_command_label};
use resina_model::SurfaceSize;
use resina_resolver::{CommandLabelInput, LabelMeasureInput, resolve_command_label};
#[path = "common/label.rs"]
mod label_style;
use label_style::style;

#[test]
fn actual_font_shapes_complete_scaled_and_expanded_labels() {
    let font = std::env::var("RESINA_LABEL_FONT").expect("RESINA_LABEL_FONT is required");
    guido::load_font(std::fs::read(font).expect("explicit label font must be readable"));
    let family = FontFamily::name("DejaVu Sans");
    for scale in [1.0, 1.5, 2.0] {
        let typography = style(scale, 0.0, 400.0, 1.4);
        assert_eq!(typography.font_size(), 20.0 * scale);
        for case in label_style::cases() {
            let text = case.text.as_str();
            let direction = case.direction;
            let mut measurements = Vec::new();
            let ir = resolve_command_label(
                CommandLabelInput {
                    text,
                    typography: &typography,
                    minimum_size: SurfaceSize {
                        width: 64.0,
                        height: 44.0,
                    },
                    maximum_size: SurfaceSize {
                        width: 220.0,
                        height: 300.0,
                    },
                    padding: SafeArea {
                        start: 16.0,
                        end: 12.0,
                        top: 8.0,
                        bottom: 8.0,
                    },
                    direction,
                },
                |input| {
                    let maximum_width = input.maximum_width;
                    let size = measure_command_label(family, input)?;
                    measurements.push((maximum_width, size));
                    Ok::<_, LabelMeasureError>(size)
                },
            )
            .unwrap();
            assert_eq!(measurements.len(), 2);
            assert_eq!(measurements[1].0, Some(ir.label_bounds().width));
            assert_eq!(ir.label_bounds().height, measurements[1].1.height);
            assert_eq!(ir.text(), text);
            assert_eq!(ir.typography(), &typography);
            assert_eq!(ir.layout_direction(), direction);
            assert!(ir.size().height >= ir.label_bounds().height + 16.0);
            if text == "Verbindung erneut herstellen" {
                assert!(measurements[0].1.width > 192.0);
                assert!(measurements[1].1.height > measurements[0].1.height);
                assert_eq!(ir.size().width, 220.0);
            }
        }
    }
    let typography = style(1.0, 0.0, 400.0, 1.4);
    let measure = |text, maximum_width| {
        measure_command_label(
            family,
            LabelMeasureInput {
                text,
                typography: &typography,
                maximum_width,
            },
        )
    };
    assert_eq!(measure("Save\nSave", None).unwrap().height, 56.0);
    assert_eq!(measure("Save\nSave", Some(192.0)).unwrap().height, 56.0);
    let fractional_width = 192.0000001;
    assert!(
        measure("Verbindung erneut herstellen", Some(fractional_width))
            .unwrap()
            .width
            <= fractional_width
    );
    for width in [0.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert_eq!(
            measure("Save", Some(width)).unwrap_err(),
            LabelMeasureError::Precision("wrap width")
        );
    }
    for (typography, expected) in [
        (
            style(1.0, 0.25, 400.0, 1.4),
            LabelMeasureError::LetterSpacing,
        ),
        (style(1.0, 0.0, 400.5, 1.4), LabelMeasureError::FontWeight),
        (
            style(1.0, 0.0, 400.0, 1.0e38),
            LabelMeasureError::Precision("line height"),
        ),
        (
            style(1.0, 0.0, 400.0, f64::from(f32::MAX)),
            LabelMeasureError::Precision("resolved line height"),
        ),
    ] {
        assert_eq!(
            measure_command_label(
                family,
                LabelMeasureInput {
                    text: "Save",
                    typography: &typography,
                    maximum_width: Some(192.0),
                }
            )
            .unwrap_err(),
            expected
        );
    }
}
