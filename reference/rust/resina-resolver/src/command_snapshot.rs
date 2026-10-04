use crate::{
    CommandAccessibilityError, CommandAccessibilityInput, CommandAccessibilityIr,
    CommandContentError, CommandLabelIr, CommandPaintIr, HitRegionError, HitRegionIr,
    SurfaceHitRegionInput, resolve_command_accessibility, resolve_command_states,
    resolve_surface_hit_region,
};
use resina_environment::EnvironmentSnapshot;
use resina_model::{ActivationState, PhysicalBounds, StateSet, SurfaceSize};
use std::fmt;

pub struct CommandSnapshotInput<'a, 'context> {
    pub label: &'a CommandLabelIr,
    pub paint: &'a CommandPaintIr,
    pub activation: &'context ActivationState,
    pub hovered: bool,
    pub description: Option<&'context str>,
    pub focusable: bool,
    pub environment: &'context EnvironmentSnapshot,
    pub available_bounds: PhysicalBounds,
    pub component_minimum: SurfaceSize,
    pub occupied_regions: &'context [PhysicalBounds],
}

#[derive(Debug)]
pub struct CommandSnapshot<'a> {
    label: &'a CommandLabelIr,
    paint: &'a CommandPaintIr,
    hit_region: HitRegionIr,
    accessibility: CommandAccessibilityIr,
}

impl CommandSnapshot<'_> {
    pub fn label(&self) -> &CommandLabelIr {
        self.label
    }
    pub fn paint(&self) -> &CommandPaintIr {
        self.paint
    }
    pub fn hit_region(&self) -> &HitRegionIr {
        &self.hit_region
    }
    pub fn accessibility(&self) -> &CommandAccessibilityIr {
        &self.accessibility
    }
}

#[derive(Debug)]
pub enum CommandSnapshotError {
    StatesMismatch {
        expected: StateSet,
        actual: StateSet,
    },
    DirectionMismatch,
    Content(CommandContentError),
    HitRegion(HitRegionError),
    Accessibility(CommandAccessibilityError),
}

impl fmt::Display for CommandSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StatesMismatch { expected, actual } => write!(
                f,
                "command paint states {:?} disagree with current activation and hover {:?}",
                actual.states(),
                expected.states()
            ),
            Self::DirectionMismatch => {
                f.write_str("command label direction disagrees with the current environment")
            }
            Self::Content(error) => write!(f, "command snapshot content: {error}"),
            Self::HitRegion(error) => write!(f, "command snapshot target: {error}"),
            Self::Accessibility(error) => write!(f, "command snapshot accessibility: {error}"),
        }
    }
}

impl std::error::Error for CommandSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Content(error) => Some(error),
            Self::HitRegion(error) => Some(error),
            Self::Accessibility(error) => Some(error),
            _ => None,
        }
    }
}

pub fn resolve_command_snapshot<'a>(
    input: CommandSnapshotInput<'a, '_>,
) -> Result<CommandSnapshot<'a>, CommandSnapshotError> {
    let body = input.paint.paint().body();
    let expected = resolve_command_states(input.activation, input.hovered);
    if body.states() != &expected {
        return Err(CommandSnapshotError::StatesMismatch {
            expected,
            actual: body.states().clone(),
        });
    }
    if input.label.layout_direction() != input.environment.layout_direction() {
        return Err(CommandSnapshotError::DirectionMismatch);
    }
    input
        .label
        .validate_content(body)
        .map_err(CommandSnapshotError::Content)?;
    let hit_region = resolve_surface_hit_region(SurfaceHitRegionInput {
        environment: input.environment,
        body,
        available_bounds: input.available_bounds,
        component_minimum: input.component_minimum,
        occupied_regions: input.occupied_regions,
    })
    .map_err(CommandSnapshotError::HitRegion)?;
    let accessibility = resolve_command_accessibility(CommandAccessibilityInput {
        label: input.label,
        activation: input.activation,
        description: input.description,
        focusable: input.focusable,
    })
    .map_err(CommandSnapshotError::Accessibility)?;
    Ok(CommandSnapshot {
        label: input.label,
        paint: input.paint,
        hit_region,
        accessibility,
    })
}
