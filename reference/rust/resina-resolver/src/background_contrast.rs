use crate::SrgbFallback;
use resina_color::{OpaqueSrgbRange, opaque_contrast_over_range};

pub(crate) fn minimum_range_contrast(color: &SrgbFallback, ranges: &[OpaqueSrgbRange]) -> f64 {
    ranges
        .iter()
        .map(|range| {
            opaque_contrast_over_range(color, range)
                .expect("candidate and range bounds are validated opaque")
        })
        .reduce(f64::min)
        .expect("background ranges were checked nonempty")
}
