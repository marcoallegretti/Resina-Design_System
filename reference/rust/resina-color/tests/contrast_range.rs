use resina_color::{
    ContrastError, ContrastRangeError, OpaqueSrgbRange, SrgbFallback, opaque_contrast_over_range,
    opaque_contrast_ratio, resolve_srgb_fallback,
};
use serde_json::{Value, json};

fn color(components: [f64; 3]) -> SrgbFallback {
    resolve_srgb_fallback(&json!({"colorSpace": "srgb", "components": components, "alpha": 1}))
        .unwrap()
}

#[test]
fn public_cases_check_bounds_crossings_and_diagnostics() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/contrast-range-cases.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 19);
    for case in cases {
        let foreground = resolve_srgb_fallback(&case["foreground"]).unwrap();
        let range = OpaqueSrgbRange::try_new(
            resolve_srgb_fallback(&case["range"]["lower"]).unwrap(),
            resolve_srgb_fallback(&case["range"]["upper"]).unwrap(),
        );
        if let Some(expected) = case["expected"].as_f64() {
            let range = range.unwrap();
            assert_eq!(serde_json::to_value(&range).unwrap(), case["range"]);
            let actual = opaque_contrast_over_range(&foreground, &range).unwrap();
            assert!(
                (actual - expected).abs() < 1e-12,
                "{}: {actual}",
                case["name"]
            );
        } else {
            let error = match range {
                Err(error) => error.to_string(),
                Ok(range) => opaque_contrast_over_range(&foreground, &range)
                    .unwrap_err()
                    .to_string(),
            };
            assert_eq!(error, case["error"].as_str().unwrap(), "{}", case["name"]);
        }
    }
}

#[test]
fn separated_flat_regions_do_not_claim_the_colors_between_them() {
    let foreground = color([0.5; 3]);
    let lower = color([0.1; 3]);
    let upper = color([0.9; 3]);
    let dark = OpaqueSrgbRange::try_new(lower.clone(), lower.clone()).unwrap();
    let light = OpaqueSrgbRange::try_new(upper.clone(), upper.clone()).unwrap();
    assert!(opaque_contrast_over_range(&foreground, &dark).unwrap() > 3.0);
    assert!(opaque_contrast_over_range(&foreground, &light).unwrap() > 3.0);
    let continuous = OpaqueSrgbRange::try_new(lower, upper).unwrap();
    assert_eq!(
        opaque_contrast_over_range(&foreground, &continuous).unwrap(),
        1.0
    );
}

#[test]
fn bound_never_overstates_independent_dense_color_contrast() {
    fn luminance(channels: [f64; 3]) -> f64 {
        let linear = channels.map(|c| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }
    let lower = [0.02, 0.1, 0.4];
    let upper = [0.08, 0.6, 0.95];
    let range = OpaqueSrgbRange::try_new(color(lower), color(upper)).unwrap();
    for foreground in [[0.0; 3], [1.0; 3], [1.0, 0.0, 0.0], [0.03, 0.2, 0.5]] {
        let bound = opaque_contrast_over_range(&color(foreground), &range).unwrap();
        let f = luminance(foreground);
        for red in 0..=16 {
            for green in 0..=16 {
                for blue in 0..=16 {
                    let steps = [red, green, blue];
                    let channels = std::array::from_fn(|i| {
                        lower[i] + (upper[i] - lower[i]) * f64::from(steps[i]) / 16.0
                    });
                    let b = luminance(channels);
                    let actual = (f.max(b) + 0.05) / (f.min(b) + 0.05);
                    assert!(
                        bound <= actual + 1e-12,
                        "{foreground:?}/{channels:?}: {bound}>{actual}"
                    );
                }
            }
        }
    }
}

#[test]
fn uniform_ranges_preserve_existing_contrast_results() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/contrast-vectors.json"
    ))
    .unwrap();
    for case in cases {
        let foreground = resolve_srgb_fallback(&case["foreground"]).unwrap();
        let background = resolve_srgb_fallback(&case["background"]).unwrap();
        if background.alpha() == 1.0 {
            let range = OpaqueSrgbRange::try_new(background.clone(), background).unwrap();
            assert_eq!(
                opaque_contrast_over_range(&foreground, &range),
                opaque_contrast_ratio(&foreground, range.lower())
            );
        }
    }
}

#[test]
fn typed_bounds_reject_transparency_before_channel_order() {
    let low = color([0.9; 3]);
    let high = color([0.1; 3]);
    assert_eq!(
        OpaqueSrgbRange::try_new(low.clone().with_alpha(0.5).unwrap(), high.clone()).unwrap_err(),
        ContrastRangeError::TranslucentLower
    );
    assert_eq!(
        OpaqueSrgbRange::try_new(low.clone(), high.clone().with_alpha(0.5).unwrap()).unwrap_err(),
        ContrastRangeError::TranslucentUpper
    );
    assert_eq!(
        OpaqueSrgbRange::try_new(low, high).unwrap_err(),
        ContrastRangeError::InvertedChannel(0)
    );
    let range = OpaqueSrgbRange::try_new(color([0.0; 3]), color([1.0; 3])).unwrap();
    assert_eq!(
        opaque_contrast_over_range(&color([0.5; 3]).with_alpha(0.0).unwrap(), &range),
        Err(ContrastError::TranslucentForeground)
    );
}
