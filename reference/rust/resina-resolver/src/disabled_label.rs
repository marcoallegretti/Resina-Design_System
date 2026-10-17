use crate::{SrgbFallback, opaque_contrast_ratio, resolve_srgb_fallback};
use resina_color::{linear_srgb_to_srgb, srgb_to_linear_srgb};

/// The bounded minimum contrast a disabled label is dimmed to against its background.
pub const DISABLED_LABEL_CONTRAST: f64 = 3.0;

/// A label already this close above the bound is published unchanged.
const BOUND_TOLERANCE: f64 = 1e-9;

const LUMINANCE: [f64; 3] = [0.2126, 0.7152, 0.0722];

fn linear(color: &SrgbFallback) -> [f64; 3] {
    srgb_to_linear_srgb(color.components()).expect("opaque label colors are encoded sRGB")
}

fn luminance(linear: [f64; 3]) -> f64 {
    linear.iter().zip(LUMINANCE).map(|(c, w)| c * w).sum()
}

fn mixed(label: [f64; 3], background: [f64; 3], fraction: f64, alpha: f64) -> SrgbFallback {
    let linear: [f64; 3] =
        std::array::from_fn(|i| label[i] + (background[i] - label[i]) * fraction);
    let components = linear_srgb_to_srgb(linear.map(|c| c.clamp(0.0, 1.0)))
        .expect("a mix of two encoded sRGB colors stays in gamut");
    resolve_srgb_fallback(&serde_json::json!({
        "colorSpace": "srgb", "components": components, "alpha": alpha
    }))
    .expect("finite in-gamut components and the label alpha form a color")
}

/// Mixes the label toward the background in linear light by the largest fraction whose contrast
/// against the background is not below [`DISABLED_LABEL_CONTRAST`]. Relative luminance is linear
/// in that mix, so the fraction has a closed form; a label at or below the bound is unchanged.
pub(crate) fn dim_disabled_label(label: &SrgbFallback, background: &SrgbFallback) -> SrgbFallback {
    let contrast = |color: &SrgbFallback| {
        opaque_contrast_ratio(color, background).expect("disabled label colors are opaque")
    };
    if contrast(label) <= DISABLED_LABEL_CONTRAST + BOUND_TOLERANCE {
        return label.clone();
    }
    let (from, to) = (linear(label), linear(background));
    let (label_luminance, background_luminance) = (luminance(from), luminance(to));
    let target = if label_luminance > background_luminance {
        DISABLED_LABEL_CONTRAST * (background_luminance + 0.05) - 0.05
    } else {
        (background_luminance + 0.05) / DISABLED_LABEL_CONTRAST - 0.05
    };
    let mut fraction = (label_luminance - target) / (label_luminance - background_luminance);
    let mut dimmed = mixed(from, to, fraction, label.alpha());
    // Encoding rounding can leave the closed-form mix a few ulps below the bound.
    while contrast(&dimmed) < DISABLED_LABEL_CONTRAST {
        fraction -= 1e-12;
        if fraction <= 0.0 {
            return label.clone();
        }
        dimmed = mixed(from, to, fraction, label.alpha());
    }
    dimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(channel: f64) -> SrgbFallback {
        resolve_srgb_fallback(&serde_json::json!({
            "colorSpace": "srgb", "components": vec![channel; 3], "alpha": 1
        }))
        .unwrap()
    }

    #[test]
    fn labels_a_few_ulps_above_the_bound_terminate_unchanged_or_at_the_bound() {
        let white = gray(1.0);
        let at_bound = linear_srgb_to_srgb([(1.05 / DISABLED_LABEL_CONTRAST) - 0.05; 3]).unwrap()[0];
        let mut channel = at_bound;
        let mut checked = 0;
        for _ in 0..64 {
            channel = f64::from_bits(channel.to_bits() - 1);
            let label = gray(channel);
            let contrast = opaque_contrast_ratio(&label, &white).unwrap();
            if contrast <= DISABLED_LABEL_CONTRAST {
                continue;
            }
            let dimmed = dim_disabled_label(&label, &white);
            assert!(opaque_contrast_ratio(&dimmed, &white).unwrap() >= DISABLED_LABEL_CONTRAST);
            if contrast <= DISABLED_LABEL_CONTRAST + BOUND_TOLERANCE {
                assert_eq!(dimmed, label);
            }
            checked += 1;
        }
        assert!(checked > 32);
    }
}
