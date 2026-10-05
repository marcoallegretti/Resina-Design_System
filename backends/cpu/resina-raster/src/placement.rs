use crate::{RasterError, Viewport};
use resina_model::{PhysicalBounds, PhysicalVector};

#[derive(Debug, Clone, Copy)]
pub struct PreparedViewport {
    viewport: Viewport,
    bounds: PhysicalBounds,
    far_corner: PhysicalVector,
}

impl PreparedViewport {
    /// Sampling coordinates relative to the unplaced paint.
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Logical bounds with parent placement already applied.
    pub fn bounds(&self) -> PhysicalBounds {
        self.bounds
    }

    /// Divided device-grid edges, retained independently of origin plus extent.
    pub fn far_corner(&self) -> PhysicalVector {
        self.far_corner
    }
}

/// Rounds placed bounds outward on the device grid; sampling coordinates remain local.
pub fn prepare_viewport(
    bounds: PhysicalBounds,
    placement: PhysicalVector,
    pixels_per_unit: f64,
) -> Result<PreparedViewport, RasterError> {
    if !pixels_per_unit.is_finite() || pixels_per_unit <= 0.0 {
        return Err(RasterError::InvalidViewport(
            "pixels per unit must be finite and positive",
        ));
    }
    if ![
        bounds.x,
        bounds.y,
        bounds.width,
        bounds.height,
        placement.x,
        placement.y,
    ]
    .iter()
    .all(|value| value.is_finite())
        || bounds.width <= 0.0
        || bounds.height <= 0.0
    {
        return Err(RasterError::InvalidViewport(
            "bounds must have finite positive area and placement must be finite",
        ));
    }
    let left = ((bounds.x + placement.x) * pixels_per_unit).floor();
    let top = ((bounds.y + placement.y) * pixels_per_unit).floor();
    let right = ((bounds.x + bounds.width + placement.x) * pixels_per_unit).ceil();
    let bottom = ((bounds.y + bounds.height + placement.y) * pixels_per_unit).ceil();
    let width = right - left;
    let height = bottom - top;
    if ![left, top, right, bottom, width, height]
        .iter()
        .all(|value| value.is_finite())
        || width < 1.0
        || height < 1.0
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return Err(RasterError::InvalidViewport(
            "placed bounds cannot be represented on the device grid",
        ));
    }
    let placed_bounds = PhysicalBounds {
        x: left / pixels_per_unit,
        y: top / pixels_per_unit,
        width: width / pixels_per_unit,
        height: height / pixels_per_unit,
    };
    let far_corner = PhysicalVector {
        x: right / pixels_per_unit,
        y: bottom / pixels_per_unit,
    };
    let origin = PhysicalVector {
        x: placed_bounds.x - placement.x,
        y: placed_bounds.y - placement.y,
    };
    if ![
        placed_bounds.x,
        placed_bounds.y,
        placed_bounds.width,
        placed_bounds.height,
        far_corner.x,
        far_corner.y,
        origin.x,
        origin.y,
    ]
    .iter()
    .all(|value| value.is_finite())
    {
        return Err(RasterError::InvalidViewport(
            "placed logical coordinates exceed finite range",
        ));
    }
    Ok(PreparedViewport {
        viewport: Viewport {
            origin,
            width: width as u32,
            height: height as u32,
            pixels_per_unit,
        },
        bounds: placed_bounds,
        far_corner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_placement_preserves_local_sampling_and_global_edges() {
        let prepared = prepare_viewport(
            PhysicalBounds {
                x: -2.5,
                y: -3.25,
                width: 12.75,
                height: 9.5,
            },
            PhysicalVector { x: 7.125, y: 11.75 },
            1.25,
        )
        .unwrap();
        let viewport = prepared.viewport();
        assert_eq!((viewport.width, viewport.height), (17, 13));
        assert_eq!(
            viewport.origin,
            PhysicalVector {
                x: -3.125,
                y: -3.75
            }
        );
        assert_eq!(
            prepared.bounds(),
            PhysicalBounds {
                x: 4.0,
                y: 8.0,
                width: 13.6,
                height: 10.4
            }
        );
        assert_eq!(prepared.far_corner(), PhysicalVector { x: 17.6, y: 18.4 });
        assert_eq!(viewport.pixels_per_unit, 1.25);
    }

    #[test]
    fn negative_focus_extent_is_not_clipped_or_translated_twice() {
        let prepared = prepare_viewport(
            PhysicalBounds {
                x: -4.0,
                y: -4.0,
                width: 28.0,
                height: 22.0,
            },
            PhysicalVector { x: 0.0, y: 0.0 },
            0.5,
        )
        .unwrap();
        assert_eq!(
            prepared.viewport().origin,
            PhysicalVector { x: -4.0, y: -4.0 }
        );
        assert_eq!(
            (prepared.viewport().width, prepared.viewport().height),
            (14, 11)
        );
        assert_eq!(prepared.far_corner(), PhysicalVector { x: 24.0, y: 18.0 });
    }

    #[test]
    fn original_pixel_edge_division_is_retained_for_native_precision_checks() {
        let prepared = prepare_viewport(
            PhysicalBounds {
                x: 40_001.125 / 1.25,
                y: 0.0,
                width: 0.25 / 1.25,
                height: 1.0,
            },
            PhysicalVector { x: 0.0, y: 0.0 },
            1.25,
        )
        .unwrap();
        assert_eq!(prepared.far_corner().x, 40_002.0 / 1.25);
        let bounds = prepared.bounds();
        assert!(
            (f64::from(bounds.x as f32 + bounds.width as f32) - prepared.far_corner().x).abs()
                > 1.0 / 1024.0
        );
    }

    #[test]
    fn invalid_inputs_and_unrepresentable_results_fail_before_rendering() {
        let bounds = PhysicalBounds {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 14.0,
        };
        let origin = PhysicalVector { x: 0.0, y: 0.0 };
        for scale in [
            0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::MAX,
            f64::from_bits(1),
        ] {
            assert!(prepare_viewport(bounds, origin, scale).is_err());
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(prepare_viewport(bounds, PhysicalVector { x: value, y: 0.0 }, 1.0).is_err());
            assert!(prepare_viewport(PhysicalBounds { x: value, ..bounds }, origin, 1.0).is_err());
        }
        for width in [0.0, -1.0, f64::INFINITY, f64::from(u32::MAX) + 1.0] {
            assert!(prepare_viewport(PhysicalBounds { width, ..bounds }, origin, 1.0).is_err());
        }
        assert!(prepare_viewport(PhysicalBounds { x: 1e20, ..bounds }, origin, 1.0).is_err());
    }
}
