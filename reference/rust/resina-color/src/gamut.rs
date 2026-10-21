use crate::{ColorConversionError, oklab_to_extended_srgb, srgb_to_oklab};

pub const GAMUT_MAPPING_JND: f64 = 0.02;
pub const GAMUT_MAPPING_EPSILON: f64 = 0.0001;

pub fn srgb_to_oklch(srgb: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    let [lightness, a, b] = srgb_to_oklab(srgb)?;
    Ok([
        lightness,
        a.hypot(b),
        normalize_hue(b.atan2(a).to_degrees()),
    ])
}

pub fn normalize_hue(degrees: f64) -> f64 {
    let hue = degrees % 360.0;
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    if hue >= 360.0 { 0.0 } else { hue }
}

pub fn gamut_map_oklch_to_srgb(origin: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    for (index, component) in origin.iter().enumerate() {
        if !component.is_finite() {
            return Err(ColorConversionError::NonFiniteOklchComponent { index });
        }
    }
    if origin[1] < 0.0 {
        return Err(ColorConversionError::OutOfRangeOklchComponent { index: 1 });
    }
    if !(0.0..360.0).contains(&origin[2]) {
        return Err(ColorConversionError::OutOfRangeOklchComponent { index: 2 });
    }
    if origin[0] >= 1.0 {
        return Ok([1.0; 3]);
    }
    if origin[0] <= 0.0 {
        return Ok([0.0; 3]);
    }
    let converted = oklch_to_srgb(origin)?;
    if in_gamut(converted) {
        return Ok(converted);
    }
    let mut current = origin;
    let mut clipped = clip(current)?;
    if delta_eok(clipped, current)? < GAMUT_MAPPING_JND {
        return Ok(clipped);
    }
    let mut minimum = 0.0;
    let mut maximum = origin[1];
    let mut minimum_in_gamut = true;
    while maximum - minimum > GAMUT_MAPPING_EPSILON {
        current[1] = (minimum + maximum) / 2.0;
        if minimum_in_gamut && in_gamut(oklch_to_srgb(current)?) {
            minimum = current[1];
            continue;
        }
        clipped = clip(current)?;
        let difference = delta_eok(clipped, current)?;
        if difference < GAMUT_MAPPING_JND {
            if GAMUT_MAPPING_JND - difference < GAMUT_MAPPING_EPSILON {
                return Ok(clipped);
            }
            minimum_in_gamut = false;
            minimum = current[1];
        } else {
            maximum = current[1];
        }
    }
    Ok(clipped)
}

fn oklch_to_oklab_unchecked([lightness, chroma, hue]: [f64; 3]) -> [f64; 3] {
    let angle = hue.to_radians();
    [lightness, chroma * angle.cos(), chroma * angle.sin()]
}

fn oklch_to_srgb(oklch: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    oklab_to_extended_srgb(oklch_to_oklab_unchecked(oklch))
}

fn in_gamut(srgb: [f64; 3]) -> bool {
    srgb.iter().all(|component| (0.0..=1.0).contains(component))
}

fn clip(oklch: [f64; 3]) -> Result<[f64; 3], ColorConversionError> {
    Ok(oklch_to_srgb(oklch)?.map(|component| component.clamp(0.0, 1.0)))
}

fn delta_eok(srgb: [f64; 3], oklch: [f64; 3]) -> Result<f64, ColorConversionError> {
    let clipped = srgb_to_oklab(srgb)?;
    let current = oklch_to_oklab_unchecked(oklch);
    Ok(((clipped[0] - current[0]).powi(2)
        + (clipped[1] - current[1]).powi(2)
        + (clipped[2] - current[2]).powi(2))
    .sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightness_bounds_return_white_and_black() {
        assert_eq!(
            gamut_map_oklch_to_srgb([1.0, 0.3, 120.0]).unwrap(),
            [1.0; 3]
        );
        assert_eq!(
            gamut_map_oklch_to_srgb([0.0, 0.3, 120.0]).unwrap(),
            [0.0; 3]
        );
    }

    #[test]
    fn in_gamut_colors_convert_without_mapping() {
        let srgb = [0.2, 0.4, 0.6];
        let mapped = gamut_map_oklch_to_srgb(srgb_to_oklch(srgb).unwrap()).unwrap();
        for (actual, expected) in mapped.into_iter().zip(srgb) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn mapping_reduces_chroma_instead_of_clipping() {
        let origin = [0.8888, 0.0791, 265.0];
        let clipped = clip(origin).unwrap();
        assert!(delta_eok(clipped, origin).unwrap() >= GAMUT_MAPPING_JND);
        let mapped = gamut_map_oklch_to_srgb(origin).unwrap();
        assert!(in_gamut(mapped));
        assert!(
            mapped
                .iter()
                .zip(clipped)
                .any(|(a, b)| (a - b).abs() > 1e-3)
        );
        let [lightness, chroma, _] = srgb_to_oklch(mapped).unwrap();
        assert!((lightness - origin[0]).abs() < GAMUT_MAPPING_JND);
        assert!(chroma < origin[1]);
    }

    #[test]
    fn every_search_exit_matches_independent_values() {
        for (origin, expected) in [
            (
                [0.878, 0.203, 101.9],
                [0.9521966026830196, 0.8519357687705986, 0.0],
            ),
            ([0.5, 0.3, 30.0], [0.7659544146267703, 0.0, 0.0]),
            (
                [0.8888, 0.0791, 265.0],
                [0.7619422285267825, 0.8563475125754105, 1.0],
            ),
        ] {
            let mapped = gamut_map_oklch_to_srgb(origin).unwrap();
            for (actual, expected) in mapped.into_iter().zip(expected) {
                assert!((actual - expected).abs() <= 1e-9, "{origin:?}");
            }
        }
    }

    #[test]
    fn invalid_origins_fail() {
        assert!(gamut_map_oklch_to_srgb([0.5, -0.1, 0.0]).is_err());
        assert!(gamut_map_oklch_to_srgb([0.5, 0.1, 360.0]).is_err());
        assert!(gamut_map_oklch_to_srgb([f64::NAN, 0.1, 0.0]).is_err());
    }

    #[test]
    fn hue_normalization_stays_in_half_open_interval() {
        assert_eq!(normalize_hue(-1e-20), 0.0);
        assert_eq!(normalize_hue(720.0), 0.0);
        assert_eq!(normalize_hue(-90.0), 270.0);
    }
}
