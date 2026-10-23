use crate::{
    CommandMotionPolicy, CommandProjection, SurfaceOutline,
    control_motion::project,
    resolve_surface_outline,
    surface_motion::{
        ContentBox, SurfaceMotionError, contained_in_face, cover, motion_policy, parse_request,
        require_outline_version, require_time,
    },
    surface_outline::OutlineRequest,
};
use resina_model::{
    MaterialFamily, PhysicalBounds, PhysicalVector, SpringDynamics, SpringState, SurfaceSize,
};
use resina_motion::{SpringTrajectorySample, sample_spring_trajectory};
use serde::{Deserialize, Serialize};

const OBJECTS: [&str; 13] = [
    "",
    "/content",
    "/sweepOffset",
    "/bounds",
    "/bounds/widthScale",
    "/bounds/heightScale",
    "/channels",
    "/channels/widthScale",
    "/channels/heightScale",
    "/channels/widthScale/dynamics",
    "/channels/widthScale/initial",
    "/channels/heightScale/dynamics",
    "/channels/heightScale/initial",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounds {
    lower: f64,
    upper: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScaleBounds {
    #[serde(rename = "widthScale")]
    width: Bounds,
    #[serde(rename = "heightScale")]
    height: Bounds,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScaleChannel {
    dynamics: SpringDynamics,
    initial: SpringState,
    target: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScaleChannels {
    #[serde(rename = "widthScale")]
    width: ScaleChannel,
    #[serde(rename = "heightScale")]
    height: ScaleChannel,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema_version: String,
    family: MaterialFamily,
    reduced_motion: bool,
    time: f64,
    outline: OutlineRequest,
    content: ContentBox,
    sweep_offset: PhysicalVector,
    bounds: ScaleBounds,
    channels: ScaleChannels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DeformationProjection {
    #[serde(rename = "widthScale")]
    width_scale: CommandProjection,
    #[serde(rename = "heightScale")]
    height_scale: CommandProjection,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceDeformationIr {
    schema_version: &'static str,
    policy: CommandMotionPolicy,
    #[serde(rename = "widthScale")]
    width_scale: SpringTrajectorySample,
    #[serde(rename = "heightScale")]
    height_scale: SpringTrajectorySample,
    projection: DeformationProjection,
    origin: PhysicalVector,
    outline: SurfaceOutline,
    reservation: PhysicalBounds,
}

fn deformation_limit(family: MaterialFamily) -> f64 {
    match family {
        MaterialFamily::Cast | MaterialFamily::Gel => 0.0,
        MaterialFamily::Frost => 0.02,
        MaterialFamily::Elastomer => 0.06,
    }
}

fn validate_domain(
    name: &str,
    bounds: &Bounds,
    target: f64,
    limit: f64,
) -> Result<(), SurfaceMotionError> {
    let contains_rest = bounds.lower > 0.0 && bounds.lower <= 1.0 && bounds.upper >= 1.0;
    if !contains_rest || !bounds.upper.is_finite() {
        return Err(SurfaceMotionError::new(format!(
            "{name} bounds must be finite and contain rest"
        )));
    }
    if bounds.lower < 1.0 - limit || bounds.upper > 1.0 + limit {
        return Err(SurfaceMotionError::new(format!(
            "{name} bounds exceed the family deformation limit"
        )));
    }
    if !(target >= bounds.lower && target <= bounds.upper) {
        return Err(SurfaceMotionError::new(format!(
            "{name} target is outside its bounds"
        )));
    }
    Ok(())
}

fn place(
    outline: &OutlineRequest,
    scale_width: f64,
    scale_height: f64,
) -> Result<(PhysicalVector, SurfaceOutline), SurfaceMotionError> {
    let rest = outline.size;
    let size = SurfaceSize {
        width: rest.width * scale_width,
        height: rest.height * scale_height,
    };
    let origin = PhysicalVector {
        x: (rest.width - size.width) / 2.0,
        y: (rest.height - size.height) / 2.0,
    };
    let resolved = resolve_surface_outline(
        size,
        outline.radii,
        outline.layout_direction,
        outline.curvature,
        outline.minimum_inset,
    )?;
    Ok((origin, resolved))
}

pub fn resolve_surface_deformation_source(
    source: &str,
) -> Result<SurfaceDeformationIr, SurfaceMotionError> {
    let request: Request = parse_request(source, &OBJECTS, &["/outline"])?;
    if request.schema_version != "0.1.0" {
        return Err(SurfaceMotionError::new("schemaVersion must be 0.1.0"));
    }
    require_outline_version(&request.outline)?;
    let policy = motion_policy(request.family, request.reduced_motion)?;
    require_time(request.time)?;
    let content = request.content.validated()?;
    let limit = deformation_limit(request.family);
    let (width, height) = (&request.bounds.width, &request.bounds.height);
    validate_domain("widthScale", width, request.channels.width.target, limit)?;
    validate_domain("heightScale", height, request.channels.height.target, limit)?;

    for scale_width in [width.lower, width.upper] {
        for scale_height in [height.lower, height.upper] {
            let (origin, resolved) = place(&request.outline, scale_width, scale_height)?;
            if !contained_in_face(content, origin, &resolved) {
                return Err(SurfaceMotionError::new(
                    "content leaves the uniform face inside the deformation domain",
                ));
            }
        }
    }

    let immediate = policy != CommandMotionPolicy::Spring;
    let sample = |channel: &ScaleChannel| {
        sample_spring_trajectory(
            &channel.dynamics,
            channel.initial,
            channel.target,
            request.time,
            immediate,
        )
    };
    let width_scale = sample(&request.channels.width)?;
    let height_scale = sample(&request.channels.height)?;
    let (scale_width, width_projection) =
        project(width_scale.state().position(), width.lower, width.upper);
    let (scale_height, height_projection) =
        project(height_scale.state().position(), height.lower, height.upper);
    let (origin, outline) = place(&request.outline, scale_width, scale_height)?;
    let (largest_origin, largest) = place(&request.outline, width.upper, height.upper)?;
    let largest_size = largest.size();
    let reservation = cover(
        &[PhysicalBounds {
            x: largest_origin.x,
            y: largest_origin.y,
            width: largest_size.width,
            height: largest_size.height,
        }],
        request.sweep_offset,
    )?;
    Ok(SurfaceDeformationIr {
        schema_version: "0.1.0",
        policy,
        width_scale,
        height_scale,
        projection: DeformationProjection {
            width_scale: width_projection,
            height_scale: height_projection,
        },
        origin,
        outline,
        reservation,
    })
}
