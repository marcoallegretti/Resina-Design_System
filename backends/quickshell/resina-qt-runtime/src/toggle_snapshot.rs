use crate::label::{parse_label_request, prepare_content};
use crate::snapshot::{Rectangle, RenderOptions};
use cxx_qt_lib::QString;
use resina_environment::SafeArea;
use resina_model::{
    ActivationState, ColorRole, CommandAppearance, OpaqueSurfaceAppearance, PhysicalVector,
    SurfaceIntent, SurfaceSize,
};
use resina_resolver::{
    CommandMotionChannel, CommandMotionChannels, HitRegionInput, OpaqueSurfaceInput,
    SurfacePaintInput, ToggleLayoutInput, TogglePart, TogglePartMotionInput, TogglePartPaintInput,
    ToggleSnapshotInput, resolve_hit_region, resolve_srgb_fallback, resolve_toggle_layout,
    resolve_toggle_part_motion, resolve_toggle_part_paint,
    resolve_toggle_snapshot as resolve_snapshot_ir, resolve_toggle_states, resolve_toggle_travel,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::{Value, json};

#[cxx::bridge(namespace = "resina::qt")]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        #[namespace = ""]
        type QString = cxx_qt_lib::QString;
        include!("label.h");
        type NativeLabelMeasure = crate::label::ffi::NativeLabelMeasure;
    }
    extern "Rust" {
        fn prepare_toggle_snapshot(
            request: &QString,
            producer: &NativeLabelMeasure,
            options: &QString,
            render: bool,
        ) -> QString;
        fn prepare_toggle_motion_snapshot(
            request: &QString,
            producer: &NativeLabelMeasure,
            options: &QString,
            motion: &QString,
            render: bool,
        ) -> QString;
        fn validate_toggle_motion(source: &QString) -> QString;
        fn retarget_toggle_motion(source: &QString) -> QString;
        fn compose_toggle_request(
            request: &QString,
            activation: &QString,
            hovered: bool,
            checked: bool,
            focusable: bool,
        ) -> QString;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Part {
    surface: SurfaceIntent,
    #[serde(deserialize_with = "crate::appearance_source::deserialize")]
    appearance: OpaqueSurfaceAppearance,
    checked_color_role: ColorRole,
    interaction_appearance: CommandAppearance,
    foreground_role: ColorRole,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

#[cfg(test)]
mod pigment_source_tests {
    use super::*;

    #[test]
    fn toggle_part_rejects_negative_pigment_sources_before_rounding() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/toggle-part-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let mut input = json!({
            "surface": body["surface"], "appearance": body["appearance"],
            "checkedColorRole": request["checkedColorRole"], "interactionAppearance": request["interactionAppearance"],
            "foregroundRole": body["foregroundRole"],
            "minimumContentContrast": body["minimumContentContrast"], "minimumEdgeContrast": body["minimumEdgeContrast"],
        });
        assert!(serde_json::from_str::<Part>(&input.to_string()).is_ok());
        input["appearance"]["pigmentProfiles"]["profiles"]["gel"]["highlightLift"] =
            json!("coefficient-source");
        let source = input
            .to_string()
            .replace("\"coefficient-source\"", "-1e-400");
        assert!(serde_json::from_str::<Part>(&source).is_err());
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Layout {
    track_size: SurfaceSize,
    thumb_size: SurfaceSize,
    insets: SafeArea,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    content: Value,
    track: Part,
    thumb: Part,
    layout: Layout,
    label_origin: PhysicalVector,
    label_foreground_role: ColorRole,
    background: Value,
    minimum_label_contrast: f64,
    minimum_thumb_contrast: f64,
    target_bounds: Rectangle,
    activation: ActivationState,
    hovered: bool,
    checked: bool,
    description: Value,
    focusable: bool,
    available_bounds: Rectangle,
    component_minimum: SurfaceSize,
    occupied_regions: Vec<Rectangle>,
}

fn document(source: &str) -> Result<Value, String> {
    if source.len() > 8 * 1024 * 1024 {
        return Err("Qt Toggle request exceeds 8 MiB".to_owned());
    }
    let value = parse_token_document(source).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("Qt Toggle request must be an object".to_owned());
    }
    for member in [
        "content",
        "track",
        "thumb",
        "layout",
        "labelOrigin",
        "targetBounds",
        "availableBounds",
        "componentMinimum",
    ] {
        if !value.get(member).is_some_and(Value::is_object) {
            return Err(format!("Qt Toggle {member} must be an object"));
        }
    }
    for member in ["trackSize", "thumbSize", "insets"] {
        if !value["layout"].get(member).is_some_and(Value::is_object) {
            return Err(format!("Qt Toggle layout {member} must be an object"));
        }
    }
    if !value
        .get("occupiedRegions")
        .and_then(Value::as_array)
        .is_some_and(|regions| regions.iter().all(Value::is_object))
    {
        return Err("Qt Toggle occupiedRegions must contain named bounds".to_owned());
    }
    let _: Request = serde_json::from_str(source).map_err(|error| error.to_string())?;
    Ok(value)
}

fn prepare(
    source: &str,
    producer: &ffi::NativeLabelMeasure,
    options: &str,
    render: bool,
    motion: Option<&MotionRequest>,
) -> Result<Value, String> {
    document(source)?;
    let request: Request = serde_json::from_str(source).map_err(|error| error.to_string())?;
    if options.len() > 64 * 1024 {
        return Err("Qt Toggle rendering options exceed 64 KiB".to_owned());
    }
    let options = parse_token_document(options).map_err(|error| error.to_string())?;
    if !options.is_object() {
        return Err("Qt Toggle rendering options must be an object".to_owned());
    }
    let options: RenderOptions =
        serde_json::from_value(options).map_err(|error| error.to_string())?;
    let content = prepare_content(parse_label_request(request.content)?, producer)?;
    let neutral = resolve_toggle_states(
        &ActivationState::try_new(true, false, None).expect("valid neutral activation"),
        false,
        false,
    );
    if request.track.surface.states() != &neutral || request.thumb.surface.states() != &neutral {
        return Err("Qt Toggle part templates must have exactly the neutral rest state".to_owned());
    }
    let background =
        resolve_srgb_fallback(&request.background).map_err(|error| error.to_string())?;
    let resolved = content
        .theme
        .resolve(&content.environment)
        .map_err(|error| format!("Qt Toggle label theme: {error:?}"))?;
    let label_foreground = resolved
        .color_fallbacks()
        .get(&request.label_foreground_role)
        .ok_or("Qt Toggle label foreground role is unresolved")?;
    let layout = resolve_toggle_layout(ToggleLayoutInput {
        track_size: request.layout.track_size,
        thumb_size: request.layout.thumb_size,
        insets: &request.layout.insets,
        layout_direction: content.environment.layout_direction(),
        checked: request.checked,
    })
    .map_err(|error| error.to_string())?;
    let states = resolve_toggle_states(&request.activation, request.hovered, request.checked);
    let track_surface = request.track.surface.clone().with_states(states.clone());
    let thumb_surface = request.thumb.surface.clone().with_states(states);
    let track_input = TogglePartPaintInput {
        part: TogglePart::Track,
        checked_color_role: request.track.checked_color_role,
        interaction_appearance: &request.track.interaction_appearance,
        surface: SurfacePaintInput {
            surrounding_color: Some(&background),
            body: OpaqueSurfaceInput {
                surface: &track_surface,
                size: request.layout.track_size,
                appearance: &request.track.appearance,
                foreground_role: request.track.foreground_role,
                post_treatment_backdrop: Some(&background),
                adjacent_color: &background,
                minimum_content_contrast: request.track.minimum_content_contrast,
                minimum_edge_contrast: request.track.minimum_edge_contrast,
            },
        },
    };
    let (track, track_motion) = if let Some(motion) = motion {
        let sampled = resolve_toggle_part_motion(
            &content.theme,
            &content.environment,
            TogglePartMotionInput {
                part: track_input,
                channels: &motion.channels.track,
                time: motion.time,
            },
        )
        .map_err(|error| error.to_string())?;
        (sampled.part_paint().clone(), Some(sampled))
    } else {
        (
            resolve_toggle_part_paint(&content.theme, &content.environment, track_input)
                .map_err(|error| error.to_string())?,
            None,
        )
    };
    let track_pigment = track.paint().body().pigment().body();
    let thumb_input = TogglePartPaintInput {
        part: TogglePart::Thumb,
        checked_color_role: request.thumb.checked_color_role,
        interaction_appearance: &request.thumb.interaction_appearance,
        surface: SurfacePaintInput {
            surrounding_color: None,
            body: OpaqueSurfaceInput {
                surface: &thumb_surface,
                size: request.layout.thumb_size,
                appearance: &request.thumb.appearance,
                foreground_role: request.thumb.foreground_role,
                post_treatment_backdrop: Some(track_pigment),
                adjacent_color: track_pigment,
                minimum_content_contrast: request.thumb.minimum_content_contrast,
                minimum_edge_contrast: request.thumb.minimum_edge_contrast,
            },
        },
    };
    let (thumb, thumb_motion) = if let Some(motion) = motion {
        let sampled = resolve_toggle_part_motion(
            &content.theme,
            &content.environment,
            TogglePartMotionInput {
                part: thumb_input,
                channels: &motion.channels.thumb,
                time: motion.time,
            },
        )
        .map_err(|error| error.to_string())?;
        (sampled.part_paint().clone(), Some(sampled))
    } else {
        (
            resolve_toggle_part_paint(&content.theme, &content.environment, thumb_input)
                .map_err(|error| error.to_string())?,
            None,
        )
    };

    let occupied: Vec<_> = request
        .occupied_regions
        .iter()
        .map(Rectangle::bounds)
        .collect();
    let target = resolve_hit_region(HitRegionInput {
        environment: &content.environment,
        visual_bounds: request.target_bounds.bounds(),
        available_bounds: request.available_bounds.bounds(),
        component_minimum: request.component_minimum,
        occupied_regions: &occupied,
    })
    .map_err(|error| error.to_string())?;
    if target.bounds() != request.target_bounds.bounds() {
        return Err("Qt Toggle requires an explicitly complete target reservation".to_owned());
    }
    let description: Option<String> =
        serde_json::from_value(request.description).map_err(|error| error.to_string())?;
    let snapshot = resolve_snapshot_ir(ToggleSnapshotInput {
        label: &content.label,
        label_origin: request.label_origin,
        label_foreground,
        label_background: &background,
        minimum_label_contrast: request.minimum_label_contrast,
        layout: &layout,
        track: &track,
        thumb: &thumb,
        hit_region: target,
        activation: &request.activation,
        hovered: request.hovered,
        checked: request.checked,
        description: description.as_deref(),
        focusable: request.focusable,
        minimum_thumb_contrast: request.minimum_thumb_contrast,
        environment: &content.environment,
        available_bounds: request.available_bounds.bounds(),
        component_minimum: request.component_minimum,
        occupied_regions: &occupied,
    })
    .map_err(|error| error.to_string())?;
    let track_origin = PhysicalVector {
        x: options.x,
        y: options.y,
    };
    let travel = motion
        .map(|motion| {
            resolve_toggle_travel(
                &snapshot,
                &motion.channels.travel.dynamics,
                motion.channels.travel.initial,
                motion.time,
            )
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let thumb_bounds = travel
        .as_ref()
        .map_or(snapshot.layout().thumb_bounds(), |travel| {
            travel.thumb_bounds()
        });
    let thumb_origin = PhysicalVector {
        x: options.x + thumb_bounds.x,
        y: options.y + thumb_bounds.y,
    };
    for (paint, origin) in [
        (snapshot.track().paint(), track_origin),
        (snapshot.thumb().paint(), thumb_origin),
    ] {
        resina_qml::prepare_surface_paint(
            paint,
            origin,
            options.device_scale,
            options.samples_per_axis,
        )
        .map_err(|error| error.to_string())?;
    }
    let mut result = json!({"snapshot":{"label":snapshot.label(),"labelOrigin":snapshot.label_origin(),
        "labelForeground":snapshot.label_foreground(),"labelBackground":snapshot.label_background(),"labelContrastRatio":snapshot.label_contrast_ratio(),
        "layout":snapshot.layout(),"track":snapshot.track(),"thumb":snapshot.thumb(),"hitRegion":snapshot.hit_region(),
        "accessibility":snapshot.accessibility(),"thumbContrastRatio":snapshot.thumb_contrast_ratio()}});
    if let Some(travel) = travel {
        result["motion"] = json!({"track":track_motion,"thumb":thumb_motion,
            "travel":{"policy":travel.policy(),"target":if request.checked {1.0} else {0.0},
                "trajectory":travel.trajectory(),"projection":travel.projection(),"thumbBounds":travel.thumb_bounds()}});
    }
    if render {
        result["trackSource"] = Value::String(
            resina_qml::render_surface_paint(
                snapshot.track().paint(),
                track_origin,
                options.device_scale,
                options.samples_per_axis,
            )
            .map_err(|error| error.to_string())?,
        );
        result["thumbSource"] = Value::String(
            resina_qml::render_surface_paint(
                snapshot.thumb().paint(),
                thumb_origin,
                options.device_scale,
                options.samples_per_axis,
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(result)
}

fn prepare_toggle_snapshot(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
    render: bool,
) -> QString {
    QString::from(
        prepare(
            &request.to_string(),
            producer,
            &options.to_string(),
            render,
            None,
        )
        .unwrap_or_else(|error| json!({"error":error}))
        .to_string(),
    )
}
fn compose_toggle_request(
    request: &QString,
    activation: &QString,
    hovered: bool,
    checked: bool,
    focusable: bool,
) -> QString {
    let result = (|| {
        let mut request = document(&request.to_string())?;
        let activation =
            parse_token_document(&activation.to_string()).map_err(|error| error.to_string())?;
        let _: ActivationState =
            serde_json::from_value(activation.clone()).map_err(|error| error.to_string())?;
        request["activation"] = activation;
        request["hovered"] = json!(hovered);
        request["checked"] = json!(checked);
        request["focusable"] = json!(focusable);
        Ok(json!({"request":request.to_string()}))
    })();
    QString::from(
        result
            .unwrap_or_else(|error: String| json!({"error":error}))
            .to_string(),
    )
}

// These are native adapter descriptors, not a new portable component protocol.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionChannels {
    track: CommandMotionChannels,
    thumb: CommandMotionChannels,
    travel: CommandMotionChannel,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionRequest {
    channels: MotionChannels,
    time: f64,
}
fn motion_document<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, String> {
    if source.len() > 64 * 1024 {
        return Err("Qt Toggle motion source exceeds 64 KiB".to_owned());
    }
    let value = parse_token_document(source).map_err(|error| error.to_string())?;
    if !value.is_object() {
        return Err("Qt Toggle motion source must be an object".to_owned());
    }
    if value
        .get("channels")
        .is_some_and(|channels| !channels.is_object())
    {
        return Err("Qt Toggle motion channels must be an object".to_owned());
    }
    if let Some(targets) = value.get("targets") {
        if !targets.is_object() {
            return Err("Qt Toggle motion targets must be named objects".to_owned());
        }
        for part in ["track", "thumb", "travel"] {
            if !targets.get(part).is_some_and(Value::is_object)
                || !targets[part].get("policy").is_some_and(Value::is_string)
            {
                return Err(
                    "Qt Toggle motion targets require named part objects and policy strings"
                        .to_owned(),
                );
            }
        }
        for part in ["track", "thumb"] {
            if !targets[part].get("target").is_some_and(Value::is_object) {
                return Err("Qt Toggle part target must be an object".to_owned());
            }
        }
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}
fn channel_value(channel: &CommandMotionChannel) -> Value {
    json!({"dynamics":channel.dynamics,"initial":channel.initial})
}
fn channels_value(channels: &MotionChannels) -> Value {
    json!({"track":{"bodyMix":channel_value(&channels.track.body_mix),"depthScale":channel_value(&channels.track.depth_scale)},
        "thumb":{"bodyMix":channel_value(&channels.thumb.body_mix),"depthScale":channel_value(&channels.thumb.depth_scale)},
        "travel":channel_value(&channels.travel)})
}
fn validate_toggle_motion(source: &QString) -> QString {
    QString::from(
        motion_document::<MotionChannels>(&source.to_string())
            .map(|channels| json!({"channels":channels_value(&channels)}))
            .unwrap_or_else(|error| json!({"error":error}))
            .to_string(),
    )
}
fn prepare_toggle_motion_snapshot(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
    motion: &QString,
    render: bool,
) -> QString {
    let result = (|| {
        let motion: MotionRequest = motion_document(&motion.to_string())?;
        prepare(
            &request.to_string(),
            producer,
            &options.to_string(),
            render,
            Some(&motion),
        )
    })();
    QString::from(
        result
            .unwrap_or_else(|error| json!({"error":error}))
            .to_string(),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum PreviousPolicy {
    Spring,
    CastImmediate,
    ReducedMotion,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartTarget {
    policy: PreviousPolicy,
    target: resina_model::CommandResponse,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TravelTarget {
    policy: PreviousPolicy,
    target: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Targets {
    track: PartTarget,
    thumb: PartTarget,
    travel: TravelTarget,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetargetRequest {
    channels: MotionChannels,
    targets: Targets,
    time: f64,
}
fn retarget_toggle_motion(source: &QString) -> QString {
    let result = (|| {
        let mut request: RetargetRequest = motion_document(&source.to_string())?;
        if request.targets.travel.target != 0.0 && request.targets.travel.target != 1.0 {
            return Err("Qt Toggle travel target must be a binary endpoint".to_owned());
        }
        for (channels, target) in [
            (&mut request.channels.track, &request.targets.track),
            (&mut request.channels.thumb, &request.targets.thumb),
        ] {
            for (channel, value) in [
                (&mut channels.body_mix, target.target.body_mix()),
                (&mut channels.depth_scale, target.target.depth_scale()),
            ] {
                channel.initial = resina_motion::sample_spring_trajectory(
                    &channel.dynamics,
                    channel.initial,
                    value,
                    request.time,
                    !matches!(target.policy, PreviousPolicy::Spring),
                )
                .map_err(|error| error.to_string())?
                .state();
            }
        }
        let travel = &mut request.channels.travel;
        travel.initial = resina_motion::sample_spring_trajectory(
            &travel.dynamics,
            travel.initial,
            request.targets.travel.target,
            request.time,
            !matches!(request.targets.travel.policy, PreviousPolicy::Spring),
        )
        .map_err(|error| error.to_string())?
        .state();
        Ok(json!({"channels":channels_value(&request.channels)}))
    })();
    QString::from(
        result
            .unwrap_or_else(|error| json!({"error":error}))
            .to_string(),
    )
}
