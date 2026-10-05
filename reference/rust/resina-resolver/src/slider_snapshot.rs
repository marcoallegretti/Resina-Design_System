use crate::{
    CommandLabelIr, HitRegionError, HitRegionInput, HitRegionIr, SliderAccessibilityError,
    SliderAccessibilityInput, SliderAccessibilityIr, SliderLayoutIr, SliderPartPaintIr,
    SliderPresentation, SliderStatesError, SliderStatesInput, SrgbFallback, SurfacePaintError,
    background_contrast::minimum_range_contrast, hit_region::bounds_overlap, resolve_hit_region,
    resolve_slider_accessibility, resolve_slider_states,
};
use resina_color::{ContrastError, OpaqueSrgbRange};
use resina_environment::EnvironmentSnapshot;
use resina_model::{PhysicalBounds, PhysicalVector, SliderPart, SurfaceSize};
use std::fmt;

const CONTRAST_REPORT_TOLERANCE: f64 = 1e-12;

pub struct SliderSnapshotInput<'a, 'context> {
    pub interaction: SliderStatesInput<'a>,
    pub layout: &'a SliderLayoutIr,
    pub track: &'a SliderPartPaintIr,
    pub thumb: &'a SliderPartPaintIr,
    pub label: &'a CommandLabelIr,
    pub label_origin: PhysicalVector,
    pub label_foreground: &'context SrgbFallback,
    pub label_background: &'context SrgbFallback,
    pub minimum_label_contrast: f64,
    pub description: Option<&'context str>,
    pub value_text: Option<&'context str>,
    pub focusable: bool,
    pub canvas_ranges: &'context [OpaqueSrgbRange],
    pub minimum_track_contrast: f64,
    pub minimum_thumb_contrast: f64,
    pub hit_region: HitRegionIr,
    pub environment: &'context EnvironmentSnapshot,
    pub available_bounds: PhysicalBounds,
    pub component_minimum: SurfaceSize,
    pub occupied_regions: &'context [PhysicalBounds],
}

#[derive(Debug)]
pub struct SliderSnapshot<'a> {
    presentation: &'a SliderPresentation,
    layout: &'a SliderLayoutIr,
    track: &'a SliderPartPaintIr,
    thumb: &'a SliderPartPaintIr,
    label: &'a CommandLabelIr,
    label_origin: PhysicalVector,
    label_foreground: SrgbFallback,
    label_background: SrgbFallback,
    label_contrast_ratio: f64,
    track_contrast_ratio: f64,
    thumb_contrast_ratio: f64,
    hit_region: HitRegionIr,
    accessibility: SliderAccessibilityIr,
    reduced_motion: bool,
}
impl SliderSnapshot<'_> {
    pub fn presentation(&self) -> &SliderPresentation {
        self.presentation
    }
    pub fn layout(&self) -> &SliderLayoutIr {
        self.layout
    }
    pub fn track(&self) -> &SliderPartPaintIr {
        self.track
    }
    pub fn thumb(&self) -> &SliderPartPaintIr {
        self.thumb
    }
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
    pub fn track_contrast_ratio(&self) -> f64 {
        self.track_contrast_ratio
    }
    pub fn thumb_contrast_ratio(&self) -> f64 {
        self.thumb_contrast_ratio
    }
    pub fn hit_region(&self) -> &HitRegionIr {
        &self.hit_region
    }
    pub fn accessibility(&self) -> &SliderAccessibilityIr {
        &self.accessibility
    }
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }
}

#[derive(Debug)]
pub enum SliderSnapshotError {
    Coherence(&'static str),
    InvalidThreshold(&'static str),
    States(SliderStatesError),
    Target(HitRegionError),
    Paint(SurfacePaintError),
    LabelColor(ContrastError),
    Accessibility(SliderAccessibilityError),
    InsufficientContrast {
        stage: &'static str,
        actual: f64,
        minimum: f64,
    },
    OverstatedContrast {
        stage: &'static str,
        reported: f64,
        verified: f64,
    },
}
impl fmt::Display for SliderSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coherence(error) => write!(f, "slider snapshot {error}"),
            Self::InvalidThreshold(stage) => write!(
                f,
                "slider snapshot {stage} threshold must be finite and in [1, 21]"
            ),
            Self::States(error) => write!(f, "slider snapshot {error}"),
            Self::Target(error) => write!(f, "slider snapshot target: {error}"),
            Self::Paint(error) => write!(f, "slider snapshot paint: {error}"),
            Self::LabelColor(error) => write!(f, "slider snapshot label: {error}"),
            Self::Accessibility(error) => write!(f, "slider snapshot accessibility: {error}"),
            Self::InsufficientContrast {
                stage,
                actual,
                minimum,
            } => write!(
                f,
                "slider snapshot {stage} contrast {actual} is below {minimum}"
            ),
            Self::OverstatedContrast {
                stage,
                reported,
                verified,
            } => write!(
                f,
                "slider snapshot {stage} reported contrast {reported} exceeds verified {verified}"
            ),
        }
    }
}
impl std::error::Error for SliderSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::States(e) => Some(e),
            Self::Target(e) => Some(e),
            Self::Paint(e) => Some(e),
            Self::LabelColor(e) => Some(e),
            Self::Accessibility(e) => Some(e),
            _ => None,
        }
    }
}
fn sum(a: f64, b: f64) -> Result<f64, SliderSnapshotError> {
    let result = a + b;
    if !a.is_finite()
        || !b.is_finite()
        || !result.is_finite()
        || (a != 0.0 && result == b)
        || (b != 0.0 && result == a)
    {
        return Err(SliderSnapshotError::Coherence(
            "placement exceeds representable arithmetic",
        ));
    }
    Ok(result)
}
fn placed(
    bounds: PhysicalBounds,
    offset: PhysicalVector,
) -> Result<PhysicalBounds, SliderSnapshotError> {
    Ok(PhysicalBounds {
        x: sum(bounds.x, offset.x)?,
        y: sum(bounds.y, offset.y)?,
        ..bounds
    })
}
fn cover(a: PhysicalBounds, b: PhysicalBounds) -> Result<PhysicalBounds, SliderSnapshotError> {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = sum(a.x, a.width)?.max(sum(b.x, b.width)?).next_up();
    let bottom = sum(a.y, a.height)?.max(sum(b.y, b.height)?).next_up();
    let bounds = PhysicalBounds {
        x,
        y,
        width: (right - x).next_up(),
        height: (bottom - y).next_up(),
    };
    for endpoint in [a, b] {
        if !crate::hit_region::bounds_contain_bounds(bounds, endpoint)
            .map_err(SliderSnapshotError::Target)?
        {
            return Err(SliderSnapshotError::Coherence(
                "swept bounds lose an endpoint",
            ));
        }
    }
    Ok(bounds)
}
fn contrast(
    stage: &'static str,
    actual: f64,
    minimum: f64,
    reported: Option<f64>,
) -> Result<(), SliderSnapshotError> {
    if !minimum.is_finite() || !(1.0..=21.0).contains(&minimum) {
        return Err(SliderSnapshotError::InvalidThreshold(stage));
    }
    if actual < minimum {
        return Err(SliderSnapshotError::InsufficientContrast {
            stage,
            actual,
            minimum,
        });
    }
    if let Some(reported) = reported
        && reported - actual > CONTRAST_REPORT_TOLERANCE
    {
        return Err(SliderSnapshotError::OverstatedContrast {
            stage,
            reported,
            verified: actual,
        });
    }
    Ok(())
}

fn validate_coherence(input: &SliderSnapshotInput<'_, '_>) -> Result<(), SliderSnapshotError> {
    if input.track.part() != SliderPart::Track || input.thumb.part() != SliderPart::Thumb {
        return Err(SliderSnapshotError::Coherence(
            "requires track and thumb in their declared slots",
        ));
    }
    let interaction = &input.interaction;
    let presentation = interaction.presentation;
    if input.layout.value() != presentation.visible() {
        return Err(SliderSnapshotError::Coherence(
            "layout must use the current visible value",
        ));
    }
    if input.layout.layout_direction() != input.environment.layout_direction()
        || input.label.layout_direction() != input.environment.layout_direction()
    {
        return Err(SliderSnapshotError::Coherence(
            "layout and label direction must match the environment",
        ));
    }
    if interaction
        .pointer
        .held_control_region()
        .is_some_and(|held| held.bounds() != input.hit_region.bounds())
    {
        return Err(SliderSnapshotError::Coherence(
            "pointer ownership must use the reserved target",
        ));
    }
    let expected = resolve_slider_states(SliderStatesInput { ..*interaction })
        .map_err(SliderSnapshotError::States)?;
    for (part, allocation) in [
        (input.track, input.layout.track_bounds()),
        (input.thumb, input.layout.thumb_bounds()),
    ] {
        if part.layout_direction() != input.environment.layout_direction() {
            return Err(SliderSnapshotError::Coherence(
                "part paint direction must match the environment",
            ));
        }
        if part.read_only() != interaction.read_only || part.paint().body().states() != &expected {
            return Err(SliderSnapshotError::Coherence(
                "part states and readOnly must match current interaction",
            ));
        }
        let front =
            part.paint()
                .body()
                .geometry()
                .front()
                .bounds()
                .ok_or(SliderSnapshotError::Paint(
                    SurfacePaintError::UnsupportedContour,
                ))?;
        if front
            != (PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: allocation.width,
                height: allocation.height,
            })
        {
            return Err(SliderSnapshotError::Coherence(
                "part front size must match current allocation",
            ));
        }
    }
    Ok(())
}

fn resolve_part_contrast(
    input: &SliderSnapshotInput<'_, '_>,
) -> Result<(f64, f64), SliderSnapshotError> {
    if input.canvas_ranges.is_empty() {
        return Err(SliderSnapshotError::Coherence(
            "canvas ranges must not be empty",
        ));
    }
    let track = input.track.paint().body();
    let thumb = input.thumb.paint().body();
    let track_contrast_ratio = minimum_range_contrast(track.edge().color(), input.canvas_ranges);
    contrast(
        "track edge",
        track_contrast_ratio,
        input.minimum_track_contrast,
        Some(track.edge().contrast_ratio()),
    )?;
    let mut surrounding = track.paint_color_ranges();
    surrounding.extend_from_slice(input.canvas_ranges);
    let thumb_contrast_ratio = minimum_range_contrast(thumb.edge().color(), &surrounding);
    contrast(
        "thumb edge",
        thumb_contrast_ratio,
        input.minimum_thumb_contrast,
        Some(thumb.edge().contrast_ratio()),
    )?;
    if let Some(focus) = input.thumb.paint().focus() {
        contrast(
            "thumb focus",
            minimum_range_contrast(focus.indicator().color(), &surrounding),
            3.0,
            Some(focus.indicator().contrast_ratio()),
        )?;
    }
    Ok((track_contrast_ratio, thumb_contrast_ratio))
}

fn validate_coverage(
    input: &SliderSnapshotInput<'_, '_>,
    hit_region: &HitRegionIr,
) -> Result<(), SliderSnapshotError> {
    let track = input.track.paint().body();
    let thumb = input.thumb.paint().body();
    let track_bounds = placed(
        track
            .geometry()
            .silhouette()
            .bounds()
            .ok_or(SliderSnapshotError::Paint(
                SurfacePaintError::UnsupportedContour,
            ))?,
        PhysicalVector {
            x: input.layout.track_bounds().x,
            y: input.layout.track_bounds().y,
        },
    )?;
    let thumb_bounds = thumb
        .geometry()
        .silhouette()
        .bounds()
        .ok_or(SliderSnapshotError::Paint(
            SurfacePaintError::UnsupportedContour,
        ))?;
    let endpoints = [
        input.layout.minimum_thumb_bounds(),
        input.layout.maximum_thumb_bounds(),
    ];
    let offset = |bounds: PhysicalBounds| PhysicalVector {
        x: bounds.x,
        y: bounds.y,
    };
    let sweep = cover(
        placed(thumb_bounds, offset(endpoints[0]))?,
        placed(thumb_bounds, offset(endpoints[1]))?,
    )?;
    let mut footprints = vec![
        track_bounds,
        sweep,
        placed(thumb_bounds, offset(input.layout.thumb_bounds()))?,
    ];
    if let Some(focus) = input.thumb.paint().focus() {
        let outer = focus.geometry().outer();
        let local = placed(
            outer.contour().bounds().ok_or(SliderSnapshotError::Paint(
                SurfacePaintError::UnsupportedContour,
            ))?,
            outer.offset(),
        )?;
        footprints.push(cover(
            placed(local, offset(endpoints[0]))?,
            placed(local, offset(endpoints[1]))?,
        )?);
        footprints.push(placed(local, offset(input.layout.thumb_bounds()))?);
    }
    let label_size = input.label.size();
    let label_bounds = PhysicalBounds {
        x: input.label_origin.x,
        y: input.label_origin.y,
        width: label_size.width,
        height: label_size.height,
    };
    if !hit_region
        .contains_bounds(label_bounds)
        .map_err(SliderSnapshotError::Target)?
    {
        return Err(SliderSnapshotError::Coherence(
            "target must cover the full label layout box",
        ));
    }
    for bounds in footprints {
        if !hit_region
            .contains_bounds(bounds)
            .map_err(SliderSnapshotError::Target)?
        {
            return Err(SliderSnapshotError::Coherence(
                "target must cover track, swept thumb and navigation paint",
            ));
        }
        if bounds_overlap(bounds, label_bounds).map_err(SliderSnapshotError::Target)? {
            return Err(SliderSnapshotError::Coherence(
                "label must not overlap track, swept thumb or navigation paint",
            ));
        }
    }
    Ok(())
}

pub fn resolve_slider_snapshot<'a>(
    input: SliderSnapshotInput<'a, '_>,
) -> Result<SliderSnapshot<'a>, SliderSnapshotError> {
    validate_coherence(&input)?;
    let (track_contrast_ratio, thumb_contrast_ratio) = resolve_part_contrast(&input)?;
    let interaction = &input.interaction;
    let presentation = interaction.presentation;
    let hit_region = resolve_hit_region(HitRegionInput {
        environment: input.environment,
        visual_bounds: input.hit_region.bounds(),
        available_bounds: input.available_bounds,
        component_minimum: input.component_minimum,
        occupied_regions: input.occupied_regions,
    })
    .map_err(SliderSnapshotError::Target)?;
    if hit_region.bounds() != input.hit_region.bounds() {
        return Err(SliderSnapshotError::Coherence(
            "reserved target must not resize",
        ));
    }
    validate_coverage(&input, &hit_region)?;
    let label_contrast_ratio =
        crate::opaque_contrast_ratio(input.label_foreground, input.label_background)
            .map_err(SliderSnapshotError::LabelColor)?;
    contrast(
        "label",
        label_contrast_ratio,
        input.minimum_label_contrast,
        None,
    )?;
    let accessibility = resolve_slider_accessibility(SliderAccessibilityInput {
        label: input.label,
        value: presentation.visible(),
        description: input.description,
        value_text: input.value_text,
        enabled: interaction.enabled,
        read_only: interaction.read_only,
        focused: interaction.focused,
        focusable: input.focusable,
        orientation: input.layout.orientation(),
    })
    .map_err(SliderSnapshotError::Accessibility)?;
    Ok(SliderSnapshot {
        presentation,
        layout: input.layout,
        track: input.track,
        thumb: input.thumb,
        label: input.label,
        label_origin: input.label_origin,
        label_foreground: input.label_foreground.clone(),
        label_background: input.label_background.clone(),
        label_contrast_ratio,
        track_contrast_ratio,
        thumb_contrast_ratio,
        hit_region,
        accessibility,
        reduced_motion: input.environment.accessibility_preferences().reduced_motion,
    })
}
