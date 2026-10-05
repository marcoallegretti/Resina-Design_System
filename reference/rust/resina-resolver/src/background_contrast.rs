use crate::{
    EdgeContrastError, EdgeContrastResult, SrgbFallback, resolve_edge_contrast,
    resolve_edge_contrast_over_ranges,
};
use resina_color::{OpaqueSrgbRange, opaque_contrast_over_range};

#[derive(Clone, Copy)]
pub(crate) enum EdgeBackground<'a> {
    Uniform(&'a SrgbFallback),
    Ranges(&'a [OpaqueSrgbRange]),
}

impl EdgeBackground<'_> {
    pub(crate) fn validate(self) -> Result<(), EdgeContrastError> {
        match self {
            Self::Uniform(color) if color.alpha() != 1.0 => {
                Err(EdgeContrastError::TranslucentAdjacentColor)
            }
            Self::Ranges([]) => Err(EdgeContrastError::EmptyAdjacentRanges),
            _ => Ok(()),
        }
    }

    pub(crate) fn resolve(
        self,
        outline: &SrgbFallback,
        strong: &SrgbFallback,
        minimum: f64,
    ) -> Result<EdgeContrastResult, EdgeContrastError> {
        match self {
            Self::Uniform(color) => resolve_edge_contrast(outline, strong, color, minimum),
            Self::Ranges(ranges) => {
                resolve_edge_contrast_over_ranges(outline, strong, ranges, minimum)
            }
        }
    }
}

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
