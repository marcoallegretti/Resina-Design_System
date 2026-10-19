use crate::{
    CommandContentPrepareError, LabelMeasureError, measure_command_label,
    prepare_command_content_at,
};
use guido::{prelude::FontFamily, renderer::DrawCommand};
use resina_environment::{EnvironmentSnapshot, SafeArea};
use resina_model::{
    ActivationState, CommandAnatomy, CommandAppearance, CommandEmphasis, MaterialLightChannels,
    OpaqueSurfaceAppearance, PhysicalBounds, PhysicalVector, SurfaceSize,
};
use resina_resolver::{
    CommandAccessibilityIr, CommandLabelError, CommandLabelInput, CommandLabelIr,
    CommandMotionChannels, CommandMotionInput, CommandMotionIr, CommandPaintError,
    CommandPaintInput, CommandPaintIr, CommandSnapshotError, CommandSnapshotInput, CompiledTheme,
    ControlRelief, HitRegionBody, HitRegionError, HitRegionIr, OpaqueSurfaceInput, SrgbFallback,
    SurfaceHitRegionInput, SurfacePaintInput, TypographyResolutionError, resolve_command_label,
    resolve_command_motion, resolve_command_paint, resolve_command_reservation_body,
    resolve_command_snapshot, resolve_command_states, resolve_surface_hit_region,
};
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub struct CommandInputs {
    pub theme: Arc<CompiledTheme>,
    pub environment: EnvironmentSnapshot,
    pub anatomy: CommandAnatomy,
    pub emphasis: CommandEmphasis,
    pub appearance: OpaqueSurfaceAppearance,
    pub command_appearance: CommandAppearance,
    /// Ambient and interaction light channels; the product supplies explicit null channels when
    /// it has no observation.
    pub light: MaterialLightChannels,
    pub relief: ControlRelief,
    pub font: FontFamily,
    pub text: String,
    pub description: Option<String>,
    pub minimum_size: SurfaceSize,
    pub maximum_size: SurfaceSize,
    pub padding: SafeArea,
    pub available_bounds: PhysicalBounds,
    pub component_minimum: SurfaceSize,
    pub occupied_regions: Vec<PhysicalBounds>,
    pub post_treatment_backdrop: Option<SrgbFallback>,
    pub adjacent_color: SrgbFallback,
    pub surrounding_color: Option<SrgbFallback>,
    pub minimum_content_contrast: f64,
    pub minimum_edge_contrast: f64,
    pub enabled: bool,
    pub focusable: bool,
    pub device_scale: f32,
    pub samples_per_axis: u8,
}

pub struct CommandPublication {
    label: CommandLabelIr,
    paint: CommandPaintIr,
    hit_region: HitRegionIr,
    accessibility: CommandAccessibilityIr,
    activation: ActivationState,
    hovered: bool,
    placement: PhysicalVector,
    commands: [DrawCommand; 2],
    motion: Option<CommandMotionIr>,
}

impl CommandPublication {
    pub fn label(&self) -> &CommandLabelIr {
        &self.label
    }
    pub fn paint(&self) -> &CommandPaintIr {
        &self.paint
    }
    pub fn hit_region(&self) -> &HitRegionIr {
        &self.hit_region
    }
    pub fn accessibility(&self) -> &CommandAccessibilityIr {
        &self.accessibility
    }
    pub fn activation(&self) -> &ActivationState {
        &self.activation
    }
    pub fn hovered(&self) -> bool {
        self.hovered
    }
    pub fn placement(&self) -> PhysicalVector {
        self.placement
    }
    pub fn commands(&self) -> &[DrawCommand; 2] {
        &self.commands
    }
    pub fn motion(&self) -> Option<&CommandMotionIr> {
        self.motion.as_ref()
    }
}

#[derive(Debug)]
pub enum CommandPrepareError {
    Availability {
        requested: bool,
        committed: bool,
    },
    ContrastRequirement {
        field: &'static str,
        requested: f64,
        minimum: f64,
    },
    Theme(Vec<TypographyResolutionError>),
    Label(CommandLabelError<LabelMeasureError>),
    Paint(CommandPaintError),
    HitRegion(HitRegionError),
    Snapshot(CommandSnapshotError),
    Content(CommandContentPrepareError),
}

impl fmt::Display for CommandPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Availability {
                requested,
                committed,
            } => write!(
                f,
                "requested Command availability {requested} disagrees with committed activation {committed}"
            ),
            Self::ContrastRequirement {
                field,
                requested,
                minimum,
            } => write!(
                f,
                "Command {field} contrast requirement {requested} must be finite and at least {minimum}:1"
            ),
            Self::Theme(errors) => {
                f.write_str("Command theme resolution failed")?;
                for error in errors {
                    write!(f, "; {error}")?;
                }
                Ok(())
            }
            Self::Label(error) => write!(f, "Command label: {error}"),
            Self::Paint(error) => write!(f, "Command paint: {error}"),
            Self::HitRegion(error) => write!(f, "Command reservation: {error}"),
            Self::Snapshot(error) => write!(f, "Command publication: {error}"),
            Self::Content(error) => write!(f, "Command native content: {error}"),
        }
    }
}

impl std::error::Error for CommandPrepareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Availability { .. } | Self::ContrastRequirement { .. } => None,
            Self::Theme(errors) => errors.first().map(|error| error as &dyn std::error::Error),
            Self::Label(error) => Some(error),
            Self::Paint(error) => Some(error),
            Self::HitRegion(error) => Some(error),
            Self::Snapshot(error) => Some(error),
            Self::Content(error) => Some(error),
        }
    }
}

impl CommandInputs {
    pub fn prepare(
        &self,
        activation: &ActivationState,
        hovered: bool,
        placement: PhysicalVector,
    ) -> Result<CommandPublication, CommandPrepareError> {
        self.prepare_sample(activation, hovered, placement, None)
    }

    pub fn prepare_motion(
        &self,
        activation: &ActivationState,
        hovered: bool,
        placement: PhysicalVector,
        channels: &CommandMotionChannels,
        time: f64,
    ) -> Result<CommandPublication, CommandPrepareError> {
        self.prepare_sample(activation, hovered, placement, Some((channels, time)))
    }

    fn prepare_sample(
        &self,
        activation: &ActivationState,
        hovered: bool,
        placement: PhysicalVector,
        motion_input: Option<(&CommandMotionChannels, f64)>,
    ) -> Result<CommandPublication, CommandPrepareError> {
        for (field, requested, minimum) in [
            ("content", self.minimum_content_contrast, 4.5),
            ("edge", self.minimum_edge_contrast, 3.0),
        ] {
            if !requested.is_finite() || requested < minimum {
                return Err(CommandPrepareError::ContrastRequirement {
                    field,
                    requested,
                    minimum,
                });
            }
        }
        if self.enabled != activation.enabled() {
            return Err(CommandPrepareError::Availability {
                requested: self.enabled,
                committed: activation.enabled(),
            });
        }
        let resolved = self
            .theme
            .resolve(&self.environment)
            .map_err(CommandPrepareError::Theme)?;
        let variant = self.anatomy.variant(self.emphasis);
        let label = resolve_command_label(
            CommandLabelInput {
                text: &self.text,
                typography: &resolved.typography()[&variant.label_typography()],
                minimum_size: self.minimum_size,
                maximum_size: self.maximum_size,
                padding: self.padding.clone(),
                direction: self.environment.layout_direction(),
            },
            |input| measure_command_label(self.font, input),
        )
        .map_err(CommandPrepareError::Label)?;
        let neutral =
            ActivationState::try_new(true, false, None).expect("neutral reservation state");
        let rest_surface = variant
            .body()
            .intent(resolve_command_states(&neutral, false));
        let surface = variant
            .body()
            .intent(resolve_command_states(activation, hovered));
        let input = |surface| CommandPaintInput {
            surface: SurfacePaintInput {
                body: OpaqueSurfaceInput {
                    surface,
                    size: label.size(),
                    appearance: &self.appearance,
                    foreground_role: variant.body().content_role(),
                    post_treatment_backdrop: self.post_treatment_backdrop.as_ref(),
                    adjacent_color: &self.adjacent_color,
                    minimum_content_contrast: self.minimum_content_contrast,
                    minimum_edge_contrast: self.minimum_edge_contrast,
                },
                surrounding_color: self.surrounding_color.as_ref(),
            },
            command_appearance: &self.command_appearance,
            light: &self.light,
            relief: &self.relief,
        };
        let deepest =
            resolve_command_reservation_body(&self.theme, &self.environment, input(&rest_surface))
                .map_err(CommandPrepareError::Paint)?;
        let target = resolve_surface_hit_region(SurfaceHitRegionInput {
            environment: &self.environment,
            body: HitRegionBody::Lit(&deepest),
            available_bounds: self.available_bounds,
            component_minimum: self.component_minimum,
            occupied_regions: &self.occupied_regions,
        })
        .map_err(CommandPrepareError::HitRegion)?;
        let motion = motion_input
            .map(|(channels, time)| {
                resolve_command_motion(
                    &self.theme,
                    &self.environment,
                    CommandMotionInput {
                        command: input(&surface),
                        channels,
                        time,
                    },
                )
            })
            .transpose()
            .map_err(CommandPrepareError::Paint)?;
        let paint = match &motion {
            Some(sample) => sample.command().clone(),
            None => resolve_command_paint(&self.theme, &self.environment, input(&surface))
                .map_err(CommandPrepareError::Paint)?,
        };
        let snapshot = resolve_command_snapshot(CommandSnapshotInput {
            label: &label,
            paint: &paint,
            hit_region: target,
            activation,
            hovered,
            description: self.description.as_deref(),
            focusable: self.focusable,
            environment: &self.environment,
            available_bounds: self.available_bounds,
            component_minimum: self.component_minimum,
            occupied_regions: &self.occupied_regions,
        })
        .map_err(CommandPrepareError::Snapshot)?;
        let commands = prepare_command_content_at(
            &snapshot,
            self.font,
            placement,
            self.device_scale,
            self.samples_per_axis,
        )
        .map_err(CommandPrepareError::Content)?;
        let hit_region = *snapshot.hit_region();
        let accessibility = snapshot.accessibility().clone();
        Ok(CommandPublication {
            label,
            paint,
            hit_region,
            accessibility,
            activation: activation.clone(),
            hovered,
            placement,
            commands,
            motion,
        })
    }
}
