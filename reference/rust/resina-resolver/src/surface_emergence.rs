use crate::{
    CommandMotionPolicy, CommandProjection, SurfaceOutline,
    control_motion::project,
    resolve_surface_outline,
    surface_motion::{
        ContentBox, SurfaceMotionError, contained_in_face, cover, interpolate, interpolate_corner,
        motion_policy, parse_request, require_outline_version, require_time,
    },
    surface_outline::OutlineRequest,
};
use resina_model::{
    LogicalCornerRadii, MaterialFamily, PhysicalBounds, PhysicalVector, SpringDynamics,
    SpringState, SurfaceSize,
};
use resina_motion::{SpringTrajectorySample, sample_spring_trajectory};
use serde::{Deserialize, Serialize};

const OBJECTS: [&str; 11] = [
    "",
    "/from",
    "/from/origin",
    "/from/content",
    "/to",
    "/to/origin",
    "/to/content",
    "/sweepOffset",
    "/channels",
    "/channels/progress",
    "/channels/opacity",
];
const NESTED_OBJECTS: [&str; 4] = [
    "/channels/progress/dynamics",
    "/channels/progress/initial",
    "/channels/opacity/dynamics",
    "/channels/opacity/initial",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct End {
    identity: String,
    outline: OutlineRequest,
    origin: PhysicalVector,
    content: ContentBox,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Channel {
    dynamics: SpringDynamics,
    initial: SpringState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Channels {
    progress: Channel,
    opacity: Channel,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    family: MaterialFamily,
    reduced_motion: bool,
    time: f64,
    emerged: bool,
    from: End,
    to: End,
    sweep_offset: PhysicalVector,
    channels: Channels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EmergenceProjection {
    progress: CommandProjection,
    opacity: CommandProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ShownContent {
    bounds: PhysicalBounds,
    opacity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceEmergenceIr {
    schema_version: &'static str,
    policy: CommandMotionPolicy,
    identity: String,
    progress: SpringTrajectorySample,
    opacity: SpringTrajectorySample,
    projection: EmergenceProjection,
    origin: PhysicalVector,
    outline: SurfaceOutline,
    from_content: Option<ShownContent>,
    to_content: Option<ShownContent>,
    reservation: PhysicalBounds,
}

fn resolve_end(end: &End) -> Result<(PhysicalBounds, SurfaceOutline), SurfaceMotionError> {
    require_outline_version(&end.outline)?;
    let content = end.content.validated()?;
    let outline = resolve_surface_outline(
        end.outline.size,
        end.outline.radii,
        end.outline.layout_direction,
        end.outline.curvature,
        end.outline.minimum_inset,
    )?;
    if !contained_in_face(content, end.origin, &outline) {
        return Err(SurfaceMotionError::new(
            "content leaves the uniform face of its own outline",
        ));
    }
    Ok((content, outline))
}

fn shown(
    content: PhysicalBounds,
    visibility: f64,
    origin: PhysicalVector,
    outline: &SurfaceOutline,
) -> Result<Option<ShownContent>, SurfaceMotionError> {
    if visibility == 0.0 {
        return Ok(None);
    }
    if !contained_in_face(content, origin, outline) {
        return Err(SurfaceMotionError::new(
            "visible content leaves the uniform face",
        ));
    }
    Ok(Some(ShownContent {
        bounds: content,
        opacity: visibility,
    }))
}

pub fn resolve_surface_emergence_source(
    source: &str,
) -> Result<SurfaceEmergenceIr, SurfaceMotionError> {
    let objects: Vec<&str> = OBJECTS.iter().chain(&NESTED_OBJECTS).copied().collect();
    let request: Request = parse_request(source, &objects, &["/from/outline", "/to/outline"])?;
    if request.schema_version != "0.1.0" {
        return Err(SurfaceMotionError::new("schemaVersion must be 0.1.0"));
    }
    let policy = motion_policy(request.family, request.reduced_motion)?;
    require_time(request.time)?;
    if request.from.identity.is_empty() || request.from.identity != request.to.identity {
        return Err(SurfaceMotionError::new(
            "both states must name one nonempty identity",
        ));
    }
    let (from, to) = (&request.from.outline, &request.to.outline);
    if from.layout_direction != to.layout_direction {
        return Err(SurfaceMotionError::new(
            "outlines must share a layout direction",
        ));
    }
    if from.curvature != to.curvature {
        return Err(SurfaceMotionError::new("outlines must share a curvature"));
    }
    let fade = &request.channels.opacity.dynamics;
    if fade.damping() * fade.damping() < 4.0 * fade.mass() * fade.stiffness() {
        return Err(SurfaceMotionError::new(
            "opacity dynamics must not overshoot",
        ));
    }
    let (from_content, _) = resolve_end(&request.from)?;
    let (to_content, _) = resolve_end(&request.to)?;

    let immediate = policy != CommandMotionPolicy::Spring;
    let target = if request.emerged { 1.0 } else { 0.0 };
    let sample = |channel: &Channel| {
        sample_spring_trajectory(
            &channel.dynamics,
            channel.initial,
            target,
            request.time,
            immediate,
        )
    };
    let progress = sample(&request.channels.progress)?;
    let opacity = sample(&request.channels.opacity)?;
    let (weight, progress_projection) = project(progress.state().position(), 0.0, 1.0);
    let (fade_in, opacity_projection) = project(opacity.state().position(), 0.0, 1.0);

    let size = SurfaceSize {
        width: interpolate(from.size.width, to.size.width, weight),
        height: interpolate(from.size.height, to.size.height, weight),
    };
    let radii = LogicalCornerRadii {
        top_start: interpolate_corner(from.radii.top_start, to.radii.top_start, weight),
        top_end: interpolate_corner(from.radii.top_end, to.radii.top_end, weight),
        bottom_end: interpolate_corner(from.radii.bottom_end, to.radii.bottom_end, weight),
        bottom_start: interpolate_corner(from.radii.bottom_start, to.radii.bottom_start, weight),
    };
    let origin = PhysicalVector {
        x: interpolate(request.from.origin.x, request.to.origin.x, weight),
        y: interpolate(request.from.origin.y, request.to.origin.y, weight),
    };
    let outline = resolve_surface_outline(
        size,
        radii,
        from.layout_direction,
        from.curvature,
        interpolate(from.minimum_inset, to.minimum_inset, weight),
    )?;
    let from_shown = shown(from_content, 1.0 - fade_in, origin, &outline)?;
    let to_shown = shown(to_content, fade_in, origin, &outline)?;

    let covered = [&request.from, &request.to].map(|end| PhysicalBounds {
        x: end.origin.x,
        y: end.origin.y,
        width: end.outline.size.width,
        height: end.outline.size.height,
    });
    let reservation = cover(&covered, request.sweep_offset)?;
    Ok(SurfaceEmergenceIr {
        schema_version: "0.1.0",
        policy,
        identity: request.from.identity,
        progress,
        opacity,
        projection: EmergenceProjection {
            progress: progress_projection,
            opacity: opacity_projection,
        },
        origin,
        outline,
        from_content: from_shown,
        to_content: to_shown,
        reservation,
    })
}
