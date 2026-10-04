use crate::{
    CommandLabelIr, ContrastError, HitRegionError, HitRegionInput, HitRegionIr, SrgbFallback,
    SurfacePaintError, ToggleAccessibilityError, ToggleAccessibilityInput, ToggleAccessibilityIr,
    ToggleLayoutIr, TogglePart, TogglePartPaintIr, opaque_contrast_ratio,
    opaque_paint::placed_contains, resolve_hit_region, resolve_toggle_accessibility,
    resolve_toggle_states,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{ActivationState, PhysicalBounds, PhysicalVector, StateSet, SurfaceSize};
use std::fmt;

const CONTRAST_TOLERANCE: f64 = 1e-12;

pub struct ToggleSnapshotInput<'a, 'context> {
    pub label: &'a CommandLabelIr,
    pub label_origin: PhysicalVector,
    pub label_foreground: &'context SrgbFallback,
    pub label_background: &'context SrgbFallback,
    pub minimum_label_contrast: f64,
    pub layout: &'a ToggleLayoutIr,
    pub track: &'a TogglePartPaintIr,
    pub thumb: &'a TogglePartPaintIr,
    pub hit_region: HitRegionIr,
    pub activation: &'context ActivationState,
    pub hovered: bool,
    pub checked: bool,
    pub description: Option<&'context str>,
    pub focusable: bool,
    pub minimum_thumb_contrast: f64,
    pub environment: &'context EnvironmentSnapshot,
    pub available_bounds: PhysicalBounds,
    pub component_minimum: SurfaceSize,
    pub occupied_regions: &'context [PhysicalBounds],
}

#[derive(Debug)]
pub struct ToggleSnapshot<'a> {
    label: &'a CommandLabelIr,
    label_origin: PhysicalVector,
    label_foreground: SrgbFallback,
    label_background: SrgbFallback,
    label_contrast_ratio: f64,
    layout: &'a ToggleLayoutIr,
    track: &'a TogglePartPaintIr,
    thumb: &'a TogglePartPaintIr,
    hit_region: HitRegionIr,
    accessibility: ToggleAccessibilityIr,
    thumb_contrast_ratio: f64,
}
impl ToggleSnapshot<'_> {
    pub fn label(&self) -> &CommandLabelIr {
        self.label
    }
    pub fn label_origin(&self) -> PhysicalVector {
        self.label_origin
    }
    pub fn label_foreground(&self) -> &SrgbFallback {
        &self.label_foreground
    }
    pub fn label_background(&self) -> &SrgbFallback {
        &self.label_background
    }
    pub fn label_contrast_ratio(&self) -> f64 {
        self.label_contrast_ratio
    }
    pub fn layout(&self) -> &ToggleLayoutIr {
        self.layout
    }
    pub fn track(&self) -> &TogglePartPaintIr {
        self.track
    }
    pub fn thumb(&self) -> &TogglePartPaintIr {
        self.thumb
    }
    pub fn hit_region(&self) -> &HitRegionIr {
        &self.hit_region
    }
    pub fn accessibility(&self) -> &ToggleAccessibilityIr {
        &self.accessibility
    }
    pub fn thumb_contrast_ratio(&self) -> f64 {
        self.thumb_contrast_ratio
    }
}

#[derive(Debug)]
pub enum ToggleSnapshotError {
    PartMismatch,
    StatesMismatch {
        part: TogglePart,
        expected: StateSet,
        actual: StateSet,
    },
    SelectionMismatch,
    DirectionMismatch,
    SizeMismatch(TogglePart),
    NumericRange,
    ThumbOutsideContent(&'static str),
    InvalidContrast,
    InvalidLabelContrast,
    LabelColor(ContrastError),
    LabelContrast {
        actual: f64,
        minimum: f64,
    },
    ThumbContrast {
        actual: f64,
        minimum: f64,
    },
    ThumbAdjacencyMismatch {
        reported: f64,
        actual: f64,
    },
    TargetResize,
    TargetCoverage(&'static str),
    LabelOverlap,
    Geometry(SurfacePaintError),
    Target(HitRegionError),
    Accessibility(ToggleAccessibilityError),
}
impl fmt::Display for ToggleSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartMismatch => {
                f.write_str("toggle snapshot requires track and thumb in their declared slots")
            }
            Self::StatesMismatch {
                part,
                expected,
                actual,
            } => write!(
                f,
                "toggle {part:?} states {:?} disagree with current signals {:?}",
                actual.states(),
                expected.states()
            ),
            Self::SelectionMismatch => {
                f.write_str("toggle layout and part selection must match current checked state")
            }
            Self::DirectionMismatch => {
                f.write_str("toggle layout and label direction must match current environment")
            }
            Self::SizeMismatch(part) => write!(
                f,
                "toggle {part:?} front size must match part layout at local origin"
            ),
            Self::NumericRange => f.write_str("toggle placement exceeds representable arithmetic"),
            Self::ThumbOutsideContent(endpoint) => write!(
                f,
                "toggle {endpoint} thumb footprint exceeds the uniform track content region"
            ),
            Self::InvalidLabelContrast => {
                f.write_str("minimumLabelContrast must be finite and in [1, 21]")
            }
            Self::LabelColor(e) => write!(f, "toggle label contrast: {e}"),
            Self::LabelContrast { actual, minimum } => {
                write!(f, "toggle label contrast is {actual}, below {minimum}")
            }
            Self::InvalidContrast => {
                f.write_str("minimumThumbContrast must be finite and in [1, 21]")
            }
            Self::ThumbContrast { actual, minimum } => write!(
                f,
                "thumb edge contrast against painted track is {actual}, below {minimum}"
            ),
            Self::ThumbAdjacencyMismatch { reported, actual } => write!(
                f,
                "thumb edge contrast {reported} disagrees with painted track contrast {actual}"
            ),
            Self::TargetResize => {
                f.write_str("reserved toggle target does not meet current minimum without resizing")
            }
            Self::TargetCoverage(member) => {
                write!(f, "reserved toggle target does not contain {member}")
            }
            Self::LabelOverlap => {
                f.write_str("toggle label layout box overlaps part paint or navigation")
            }
            Self::Geometry(e) => write!(f, "toggle snapshot geometry: {e}"),
            Self::Target(e) => write!(f, "toggle snapshot target: {e}"),
            Self::Accessibility(e) => write!(f, "toggle snapshot accessibility: {e}"),
        }
    }
}
impl std::error::Error for ToggleSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LabelColor(e) => Some(e),
            Self::Geometry(e) => Some(e),
            Self::Target(e) => Some(e),
            Self::Accessibility(e) => Some(e),
            _ => None,
        }
    }
}
fn sum(origin: f64, offset: f64) -> Result<f64, ToggleSnapshotError> {
    let value = origin + offset;
    if !origin.is_finite()
        || !offset.is_finite()
        || !value.is_finite()
        || (offset != 0.0 && value == origin)
        || (origin != 0.0 && value == offset)
    {
        return Err(ToggleSnapshotError::NumericRange);
    }
    Ok(value)
}
fn placed(
    bounds: PhysicalBounds,
    offset: PhysicalVector,
) -> Result<PhysicalBounds, ToggleSnapshotError> {
    Ok(PhysicalBounds {
        x: sum(bounds.x, offset.x)?,
        y: sum(bounds.y, offset.y)?,
        ..bounds
    })
}
fn endpoints(bounds: PhysicalBounds) -> Result<(f64, f64), ToggleSnapshotError> {
    if !bounds.width.is_finite()
        || bounds.width <= 0.0
        || !bounds.height.is_finite()
        || bounds.height <= 0.0
    {
        return Err(ToggleSnapshotError::NumericRange);
    }
    Ok((sum(bounds.x, bounds.width)?, sum(bounds.y, bounds.height)?))
}
fn overlap(a: PhysicalBounds, b: PhysicalBounds) -> Result<bool, ToggleSnapshotError> {
    let (ar, ab) = endpoints(a)?;
    let (br, bb) = endpoints(b)?;
    Ok(a.x < br && b.x < ar && a.y < bb && b.y < ab)
}

pub fn resolve_toggle_snapshot<'a>(
    input: ToggleSnapshotInput<'a, '_>,
) -> Result<ToggleSnapshot<'a>, ToggleSnapshotError> {
    if input.track.part() != TogglePart::Track || input.thumb.part() != TogglePart::Thumb {
        return Err(ToggleSnapshotError::PartMismatch);
    }
    let expected = resolve_toggle_states(input.activation, input.hovered, input.checked);
    for part in [input.track, input.thumb] {
        if part.paint().body().states() != &expected {
            return Err(ToggleSnapshotError::StatesMismatch {
                part: part.part(),
                expected,
                actual: part.paint().body().states().clone(),
            });
        }
    }
    if input.layout.checked() != input.checked
        || input.track.checked() != input.checked
        || input.thumb.checked() != input.checked
    {
        return Err(ToggleSnapshotError::SelectionMismatch);
    }
    if input.layout.layout_direction() != input.environment.layout_direction()
        || input.label.layout_direction() != input.environment.layout_direction()
    {
        return Err(ToggleSnapshotError::DirectionMismatch);
    }
    for (part, bounds) in [
        (input.track, input.layout.track_bounds()),
        (input.thumb, input.layout.thumb_bounds()),
    ] {
        let front = part.paint().body().geometry().front().bounds().ok_or(
            ToggleSnapshotError::Geometry(SurfacePaintError::UnsupportedContour),
        )?;
        if front
            != (PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: bounds.width,
                height: bounds.height,
            })
        {
            return Err(ToggleSnapshotError::SizeMismatch(part.part()));
        }
    }
    let track = input.track.paint().body();
    let thumb = input.thumb.paint().body();
    let track_bounds =
        track
            .geometry()
            .silhouette()
            .bounds()
            .ok_or(ToggleSnapshotError::Geometry(
                SurfacePaintError::UnsupportedContour,
            ))?;
    let thumb_bounds =
        thumb
            .geometry()
            .silhouette()
            .bounds()
            .ok_or(ToggleSnapshotError::Geometry(
                SurfacePaintError::UnsupportedContour,
            ))?;
    let mut footprints = vec![("track paint", track_bounds)];
    for (name, endpoint) in [
        ("off", input.layout.off_thumb_bounds()),
        ("on", input.layout.on_thumb_bounds()),
    ] {
        let bounds = placed(
            thumb_bounds,
            PhysicalVector {
                x: endpoint.x,
                y: endpoint.y,
            },
        )?;
        let (right, bottom) = endpoints(bounds)?;
        // Convex content containing the whole footprint box contains every painted thumb point.
        for x in [bounds.x, right] {
            for y in [bounds.y, bottom] {
                if !placed_contains(track.geometry().content(), PhysicalVector { x, y })
                    .map_err(ToggleSnapshotError::Geometry)?
                {
                    return Err(ToggleSnapshotError::ThumbOutsideContent(name));
                }
            }
        }
        footprints.push((name, bounds));
    }
    if let Some(focus) = input.track.paint().focus() {
        let outer = focus.geometry().outer();
        footprints.push((
            "track navigation",
            placed(
                outer
                    .contour()
                    .bounds()
                    .ok_or(ToggleSnapshotError::Geometry(
                        SurfacePaintError::UnsupportedContour,
                    ))?,
                outer.offset(),
            )?,
        ));
    }
    let minimum = input.minimum_thumb_contrast;
    if !minimum.is_finite() || !(1.0..=21.0).contains(&minimum) {
        return Err(ToggleSnapshotError::InvalidContrast);
    }
    let ratio = opaque_contrast_ratio(thumb.edge().color(), track.pigment().body())
        .map_err(|_| ToggleSnapshotError::InvalidContrast)?;
    if ratio < minimum {
        return Err(ToggleSnapshotError::ThumbContrast {
            actual: ratio,
            minimum,
        });
    }
    if (ratio - thumb.edge().contrast_ratio()).abs() > CONTRAST_TOLERANCE {
        return Err(ToggleSnapshotError::ThumbAdjacencyMismatch {
            reported: thumb.edge().contrast_ratio(),
            actual: ratio,
        });
    }
    let hit_region = resolve_hit_region(HitRegionInput {
        environment: input.environment,
        visual_bounds: input.hit_region.bounds(),
        available_bounds: input.available_bounds,
        component_minimum: input.component_minimum,
        occupied_regions: input.occupied_regions,
    })
    .map_err(ToggleSnapshotError::Target)?;
    if hit_region.bounds() != input.hit_region.bounds() {
        return Err(ToggleSnapshotError::TargetResize);
    }
    let size = input.label.size();
    let label_bounds = PhysicalBounds {
        x: input.label_origin.x,
        y: input.label_origin.y,
        width: size.width,
        height: size.height,
    };
    endpoints(label_bounds)?;
    for (name, bounds) in footprints {
        if !hit_region
            .contains_bounds(bounds)
            .map_err(ToggleSnapshotError::Target)?
        {
            return Err(ToggleSnapshotError::TargetCoverage(name));
        }
        if overlap(label_bounds, bounds)? {
            return Err(ToggleSnapshotError::LabelOverlap);
        }
    }
    if !hit_region
        .contains_bounds(label_bounds)
        .map_err(ToggleSnapshotError::Target)?
    {
        return Err(ToggleSnapshotError::TargetCoverage("label layout box"));
    }
    let minimum = input.minimum_label_contrast;
    if !minimum.is_finite() || !(1.0..=21.0).contains(&minimum) {
        return Err(ToggleSnapshotError::InvalidLabelContrast);
    }
    let label_contrast_ratio =
        opaque_contrast_ratio(input.label_foreground, input.label_background)
            .map_err(ToggleSnapshotError::LabelColor)?;
    if label_contrast_ratio < minimum {
        return Err(ToggleSnapshotError::LabelContrast {
            actual: label_contrast_ratio,
            minimum,
        });
    }
    let accessibility = resolve_toggle_accessibility(ToggleAccessibilityInput {
        label: input.label,
        activation: input.activation,
        description: input.description,
        focusable: input.focusable,
        checked: input.checked,
    })
    .map_err(ToggleSnapshotError::Accessibility)?;
    Ok(ToggleSnapshot {
        label: input.label,
        label_origin: input.label_origin,
        label_foreground: input.label_foreground.clone(),
        label_background: input.label_background.clone(),
        label_contrast_ratio,
        layout: input.layout,
        track: input.track,
        thumb: input.thumb,
        hit_region,
        accessibility,
        thumb_contrast_ratio: ratio,
    })
}
