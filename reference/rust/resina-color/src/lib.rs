mod contrast;
pub use contrast::{
    ContrastError, ContrastRangeError, OpaqueSrgbRange, opaque_contrast_over_range,
    opaque_contrast_ratio,
};

mod fallback;
pub use fallback::{
    ColorFallbackError, CompositeError, SrgbFallback, composite_srgb_over_opaque,
    resolve_srgb_fallback,
};

mod opaque;
pub use opaque::{OpaqueFallbackError, resolve_opaque_srgb_fallback};

mod conversion;
pub use conversion::{
    ColorConversionError, a98_rgb_to_extended_srgb, display_p3_to_extended_srgb, hsl_to_srgb,
    hwb_to_srgb, lab_to_extended_srgb, lch_to_lab, linear_srgb_to_srgb, oklab_to_extended_srgb,
    oklch_to_oklab, prophoto_rgb_to_extended_srgb, rec2020_to_extended_srgb, srgb_to_linear_srgb,
    srgb_to_oklab, xyz_d50_to_extended_srgb, xyz_d65_to_extended_srgb,
};
