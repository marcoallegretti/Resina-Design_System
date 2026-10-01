mod contrast;
pub use contrast::{ContrastError, opaque_contrast_ratio};

mod fallback;
pub use fallback::{ColorFallbackError, SrgbFallback, resolve_srgb_fallback};
