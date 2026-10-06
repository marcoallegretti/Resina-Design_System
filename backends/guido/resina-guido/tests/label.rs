#![cfg(feature = "testing")]

use guido::widgets::{Color, font::FontFamily};
use resina_environment::{LayoutDirection, SafeArea};
use resina_guido::{
    LabelMeasureError, LabelPrepareError, measure_command_label, prepare_command_label,
};
use resina_model::SurfaceSize;
use resina_resolver::{CommandLabelInput, LabelMeasureInput, resolve_command_label};
#[path = "common/cases.rs"]
mod label_cases;
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
        for case in label_cases::cases() {
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
    for (text, lines) in [
        ("Save\rNow", 2.0),
        ("Save\r\nNow", 2.0),
        ("Save\n\nNow", 3.0),
        ("Save\r\n\r\nNow", 3.0),
        ("Save\r\n\rNow", 3.0),
        ("Save\n\r\nNow", 3.0),
    ] {
        assert_eq!(
            measure(text, None).unwrap().height,
            lines * 28.0,
            "{text:?}"
        );
    }
    for (text, break_char) in [
        ("Save\u{b}Now", '\u{b}'),
        ("Save\u{c}Now", '\u{c}'),
        ("Save\u{85}Now", '\u{85}'),
        ("Save\u{2028}Now", '\u{2028}'),
        ("Save\u{2029}Now", '\u{2029}'),
        ("Save\n\rNow", '\r'),
        ("Save\n\r\rNow", '\r'),
        ("Save\n\r\n\rNow", '\r'),
    ] {
        assert_eq!(
            measure(text, None).unwrap_err(),
            LabelMeasureError::UnsupportedLineBreak(break_char),
            "{text:?}"
        );
    }
    for (text, separator) in [
        ("\u{5e9}\u{1c}Save", '\u{1c}'),
        ("Save\u{1d}\u{5e9}", '\u{1d}'),
        ("\u{5e9}\u{1e}Save", '\u{1e}'),
        ("Save\u{1c}Now", '\u{1c}'),
    ] {
        assert_eq!(
            measure(text, None).unwrap_err(),
            LabelMeasureError::UnsupportedParagraphSeparator(separator),
            "{text:?}"
        );
    }
    let digits = "1234 5678 1234 5678 1234 5678";
    let nested = |text: &str| {
        measure_command_label(
            family,
            LabelMeasureInput {
                text,
                typography: &typography,
                maximum_width: Some(40.0),
            },
        )
    };
    for text in [
        // The highest resolved level, 122: 60 left-to-right embeddings over digits.
        format!(
            "{}\u{661}\u{662}\u{663}\u{664} \u{665}\u{666}\u{667}\u{668} \u{661}\u{662}\u{663}\u{664}",
            "\u{202a}".repeat(60)
        ),
        format!(
            "{}{digits}\n{}{digits}",
            "\u{202b}".repeat(40),
            "\u{2067}".repeat(40)
        ),
    ] {
        assert!(nested(&text).is_ok());
    }
    for text in [
        format!("{}{digits}", "\u{202b}".repeat(61)),
        format!("{}{digits}", "\u{2067}\u{202c}".repeat(63)),
    ] {
        assert_eq!(
            nested(&text).unwrap_err(),
            LabelMeasureError::BidiInitiators
        );
    }
    let deep = format!("{}{digits}", "\u{2067}\u{202c}".repeat(63));
    for (text, expected) in [
        (deep.as_str(), LabelMeasureError::BidiInitiators),
        (
            "Save\u{2028}Now",
            LabelMeasureError::UnsupportedLineBreak('\u{2028}'),
        ),
        (
            "\u{5e9}\u{1c}Save",
            LabelMeasureError::UnsupportedParagraphSeparator('\u{1c}'),
        ),
    ] {
        let foreign = resolve_command_label(
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
                direction: LayoutDirection::Ltr,
            },
            |_| {
                Ok::<_, LabelMeasureError>(SurfaceSize {
                    width: 40.0,
                    height: 56.0,
                })
            },
        )
        .unwrap();
        assert_eq!(
            prepare_command_label(&foreign, family, Color::BLACK, 1.0).unwrap_err(),
            LabelPrepareError::Measurement(expected)
        );
    }
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
    for scale in [1.0, 2.0] {
        let natural = |tracking| {
            measure_command_label(
                family,
                LabelMeasureInput {
                    text: "Save",
                    typography: &style(scale, tracking, 400.0, 1.4),
                    maximum_width: None,
                },
            )
            .unwrap()
            .width
        };
        let unspaced = natural(0.0);
        for tracking in [-0.4, 0.15, 1.0] {
            let expected = unspaced + 4.0 * tracking * scale;
            assert!((natural(tracking) - expected).abs() <= 1.0e-4);
        }
    }
    for (typography, expected) in [
        (
            style(1.0, 1.0e10 + 0.5, 400.0, 1.4),
            LabelMeasureError::Precision("letter spacing"),
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
