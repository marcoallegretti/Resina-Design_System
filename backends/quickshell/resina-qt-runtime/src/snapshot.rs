use crate::label::{parse_label_request, prepare_content};
use cxx_qt_lib::QString;
use resina_model::{
    ActivationState, ColorRole, CommandAppearance, OpaqueSurfaceAppearance, PhysicalBounds,
    SurfaceIntent, SurfaceSize,
};
use resina_resolver::{
    CommandMotionChannels, CommandMotionInput, CommandPaintInput, CommandSnapshotInput,
    OpaqueSurfaceInput, SrgbFallback, SurfaceHitRegionInput, SurfacePaintInput,
    resolve_command_motion, resolve_command_paint, resolve_command_snapshot,
    resolve_command_states, resolve_srgb_fallback, resolve_surface_hit_region,
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
        fn compose_request(
            request: &QString,
            activation: &QString,
            hovered: bool,
            focusable: bool,
        ) -> QString;
        fn resolve_snapshot(request: &QString, producer: &NativeLabelMeasure) -> QString;
        fn qualify_snapshot(
            request: &QString,
            producer: &NativeLabelMeasure,
            options: &QString,
        ) -> QString;
        fn resolve_motion_snapshot(
            request: &QString,
            producer: &NativeLabelMeasure,
            options: &QString,
            motion: &QString,
            render: bool,
        ) -> QString;
        fn retarget_motion(source: &QString) -> QString;
        fn validate_motion_channels(source: &QString) -> QString;
        fn resolve_painted_snapshot(
            request: &QString,
            producer: &NativeLabelMeasure,
            options: &QString,
        ) -> QString;
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    content: Value,
    body: Body,
    activation: ActivationState,
    hovered: bool,
    description: Value,
    focusable: bool,
    available_bounds: Rectangle,
    component_minimum: SurfaceSize,
    occupied_regions: Vec<Rectangle>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rectangle {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rectangle {
    pub(crate) fn bounds(&self) -> PhysicalBounds {
        PhysicalBounds {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Body {
    surface: SurfaceIntent,
    #[serde(deserialize_with = "crate::appearance_source::deserialize")]
    appearance: OpaqueSurfaceAppearance,
    command_appearance: CommandAppearance,
    foreground_role: ColorRole,
    adjacent_color: Value,
    post_treatment_backdrop: Value,
    surrounding_color: Value,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

#[cfg(test)]
mod pigment_source_tests {
    use super::*;

    #[test]
    fn command_body_rejects_negative_pigment_sources_before_rounding() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/command-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let mut input = json!({
            "surface": body["surface"], "appearance": body["appearance"],
            "commandAppearance": request["commandAppearance"],
            "foregroundRole": body["foregroundRole"], "adjacentColor": body["adjacentColor"],
            "postTreatmentBackdrop": body["postTreatmentBackdrop"], "surroundingColor": request["surface"]["surroundingColor"],
            "minimumContentContrast": body["minimumContentContrast"], "minimumEdgeContrast": body["minimumEdgeContrast"],
        });
        assert!(serde_json::from_str::<Body>(&input.to_string()).is_ok());
        input["appearance"]["pigmentProfiles"]["profiles"]["gel"]["highlightLift"] =
            json!("coefficient-source");
        let source = input
            .to_string()
            .replace("\"coefficient-source\"", "-1e-400");
        assert!(serde_json::from_str::<Body>(&source).is_err());
    }
}

fn optional_color(value: &Value) -> Result<Option<SrgbFallback>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        resolve_srgb_fallback(value)
            .map(Some)
            .map_err(|error| error.to_string())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RenderOptions {
    pub x: f64,
    pub y: f64,
    pub device_scale: f64,
    pub samples_per_axis: u8,
}

fn prepare(
    source: &str,
    producer: &ffi::NativeLabelMeasure,
    rendering: Option<(RenderOptions, bool)>,
    motion: Option<&MotionRequest>,
) -> Result<Value, String> {
    if source.len() > 8 * 1024 * 1024 {
        return Err("Qt command snapshot request exceeds 8 MiB".to_owned());
    }
    let document = parse_token_document(source).map_err(|error| error.to_string())?;
    if !document.is_object() || !document.get("body").is_some_and(Value::is_object) {
        return Err("Qt command snapshot request and body must be objects".to_owned());
    }
    if !document
        .get("availableBounds")
        .is_some_and(Value::is_object)
        || !document
            .get("occupiedRegions")
            .and_then(Value::as_array)
            .is_some_and(|regions| regions.iter().all(Value::is_object))
    {
        return Err("Qt command target bounds must be named objects".to_owned());
    }
    let request: Request = serde_json::from_str(source).map_err(|error| error.to_string())?;
    let description: Option<String> =
        serde_json::from_value(request.description).map_err(|error| error.to_string())?;
    let rest = resolve_command_states(
        &ActivationState::try_new(true, false, None).expect("valid neutral command"),
        false,
    );
    if request.body.surface.states() != &rest {
        return Err("Qt command body template must have exactly the neutral rest state".to_owned());
    }
    let adjacent =
        resolve_srgb_fallback(&request.body.adjacent_color).map_err(|error| error.to_string())?;
    let backdrop = optional_color(&request.body.post_treatment_backdrop)?;
    let surrounding = optional_color(&request.body.surrounding_color)?;
    let content = prepare_content(parse_label_request(request.content)?, producer)?;
    let available_bounds = request.available_bounds.bounds();
    let occupied_regions: Vec<_> = request
        .occupied_regions
        .iter()
        .map(Rectangle::bounds)
        .collect();
    let paint_input = |surface| CommandPaintInput {
        surface: SurfacePaintInput {
            body: OpaqueSurfaceInput {
                surface,
                size: content.label.size(),
                appearance: &request.body.appearance,
                foreground_role: request.body.foreground_role,
                post_treatment_backdrop: backdrop.as_ref(),
                adjacent_color: &adjacent,
                minimum_content_contrast: request.body.minimum_content_contrast,
                minimum_edge_contrast: request.body.minimum_edge_contrast,
            },
            surrounding_color: surrounding.as_ref(),
        },
        command_appearance: &request.body.command_appearance,
    };
    let rest_paint = resolve_command_paint(
        &content.theme,
        &content.environment,
        paint_input(&request.body.surface),
    )
    .map_err(|error| error.to_string())?;
    let hit_region = resolve_surface_hit_region(SurfaceHitRegionInput {
        environment: &content.environment,
        body: rest_paint.paint().body(),
        available_bounds,
        component_minimum: request.component_minimum,
        occupied_regions: &occupied_regions,
    })
    .map_err(|error| error.to_string())?;
    let surface = request
        .body
        .surface
        .clone()
        .with_states(resolve_command_states(&request.activation, request.hovered));
    let sampled = motion
        .map(|motion| {
            resolve_command_motion(
                &content.theme,
                &content.environment,
                CommandMotionInput {
                    command: paint_input(&surface),
                    channels: &motion.channels,
                    time: motion.time,
                },
            )
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let paint = match &sampled {
        Some(sample) => sample.command().clone(),
        None => resolve_command_paint(&content.theme, &content.environment, paint_input(&surface))
            .map_err(|error| error.to_string())?,
    };
    let snapshot = resolve_command_snapshot(CommandSnapshotInput {
        label: &content.label,
        paint: &paint,
        hit_region,
        activation: &request.activation,
        hovered: request.hovered,
        description: description.as_deref(),
        focusable: request.focusable,
        environment: &content.environment,
        available_bounds,
        component_minimum: request.component_minimum,
        occupied_regions: &occupied_regions,
    })
    .map_err(|error| error.to_string())?;
    let mut result = json!({"snapshot": {"label": snapshot.label(), "paint": snapshot.paint(),
        "hitRegion": snapshot.hit_region(), "accessibility": snapshot.accessibility()}});
    if let Some(sample) = sampled {
        result["motion"] = serde_json::to_value(sample).map_err(|error| error.to_string())?;
    }
    if let Some((options, render)) = rendering {
        let placement = resina_model::PhysicalVector {
            x: options.x,
            y: options.y,
        };
        resina_qml::prepare_surface_paint(
            snapshot.paint().paint(),
            placement,
            options.device_scale,
            options.samples_per_axis,
        )
        .map_err(|error| error.to_string())?;
        if render {
            let source = resina_qml::render_surface_paint(
                snapshot.paint().paint(),
                resina_model::PhysicalVector {
                    x: options.x,
                    y: options.y,
                },
                options.device_scale,
                options.samples_per_axis,
            )
            .map_err(|error| error.to_string())?;
            result["paintSource"] = Value::String(source);
        }
    }
    Ok(result)
}

fn resolve_snapshot(request: &QString, producer: &ffi::NativeLabelMeasure) -> QString {
    QString::from(
        prepare(&request.to_string(), producer, None, None)
            .unwrap_or_else(|error| json!({"error": error}))
            .to_string(),
    )
}

fn qualify_snapshot(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
) -> QString {
    resolve_with_options(request, producer, options, false)
}

fn resolve_painted_snapshot(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
) -> QString {
    resolve_with_options(request, producer, options, true)
}

fn resolve_with_options(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
    render: bool,
) -> QString {
    let result = (|| {
        let options = options.to_string();
        if options.len() > 64 * 1024 {
            return Err("Qt command rendering options exceed 64 KiB".to_owned());
        }
        let document = parse_token_document(&options).map_err(|error| error.to_string())?;
        if !document.is_object() {
            return Err("Qt command rendering options must be an object".to_owned());
        }
        let rendering = serde_json::from_value(document).map_err(|error| error.to_string())?;
        prepare(
            &request.to_string(),
            producer,
            Some((rendering, render)),
            None,
        )
    })();
    QString::from(
        result
            .unwrap_or_else(|error| json!({"error": error}))
            .to_string(),
    )
}

fn compose_source(
    source: &str,
    activation: &str,
    hovered: bool,
    focusable: bool,
) -> Result<String, String> {
    if source.len() > 8 * 1024 * 1024 || activation.len() > 64 * 1024 {
        return Err("Qt command composition input exceeds its source bound".to_owned());
    }
    let mut document = parse_token_document(source).map_err(|error| error.to_string())?;
    if !document.is_object() {
        return Err("Qt command template must be an object".to_owned());
    }
    // Validate the complete original shape before replacing native-owned channels.
    // In particular, never round-trip untrusted source through QML JSON.parse.
    let _: Request = serde_json::from_str(source).map_err(|error| error.to_string())?;
    let state = parse_token_document(activation).map_err(|error| error.to_string())?;
    let _: ActivationState =
        serde_json::from_value(state.clone()).map_err(|error| error.to_string())?;
    document["activation"] = state;
    document["hovered"] = Value::Bool(hovered);
    document["focusable"] = Value::Bool(focusable);
    Ok(document.to_string())
}

fn compose_request(
    request: &QString,
    activation: &QString,
    hovered: bool,
    focusable: bool,
) -> QString {
    QString::from(
        match compose_source(
            &request.to_string(),
            &activation.to_string(),
            hovered,
            focusable,
        ) {
            Ok(request) => json!({"request": request}),
            Err(error) => json!({"error": error}),
        }
        .to_string(),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MotionRequest {
    schema_version: String,
    channels: CommandMotionChannels,
    time: f64,
}

fn parse_motion_source<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, String> {
    if source.len() > 64 * 1024 {
        return Err("Qt motion input exceeds 64 KiB".to_owned());
    }
    let document = parse_token_document(source).map_err(|error| error.to_string())?;
    if !document.is_object() {
        return Err("Qt motion input must be an object".to_owned());
    }
    serde_json::from_value(document).map_err(|error| error.to_string())
}

fn resolve_motion_snapshot(
    request: &QString,
    producer: &ffi::NativeLabelMeasure,
    options: &QString,
    motion: &QString,
    render: bool,
) -> QString {
    let result = (|| {
        let motion: MotionRequest = parse_motion_source(&motion.to_string())?;
        if motion.schema_version != "0.1.0" {
            return Err("Qt motion schemaVersion must be 0.1.0".to_owned());
        }
        let rendering: RenderOptions = parse_motion_source(&options.to_string())?;
        prepare(
            &request.to_string(),
            producer,
            Some((rendering, render)),
            Some(&motion),
        )
    })();
    QString::from(
        result
            .unwrap_or_else(|error| json!({"error":error}))
            .to_string(),
    )
}

fn serialized_channels(channels: &CommandMotionChannels) -> Value {
    json!({"bodyMix":{"dynamics":channels.body_mix.dynamics,"initial":channels.body_mix.initial},
        "depthScale":{"dynamics":channels.depth_scale.dynamics,"initial":channels.depth_scale.initial}})
}

fn validate_motion_channels(source: &QString) -> QString {
    let result = parse_motion_source::<CommandMotionChannels>(&source.to_string())
        .map(|channels| json!({"channels":serialized_channels(&channels)}))
        .unwrap_or_else(|error| json!({"error":error}));
    QString::from(result.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum PreviousPolicy {
    Spring,
    CastImmediate,
    ReducedMotion,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetargetRequest {
    channels: CommandMotionChannels,
    target: resina_model::CommandResponse,
    policy: PreviousPolicy,
    time: f64,
}

fn retarget_motion(source: &QString) -> QString {
    let result = (|| {
        let mut request: RetargetRequest = parse_motion_source(&source.to_string())?;
        let immediate = !matches!(request.policy, PreviousPolicy::Spring);
        let body = resina_motion::sample_spring_trajectory(
            &request.channels.body_mix.dynamics,
            request.channels.body_mix.initial,
            request.target.body_mix(),
            request.time,
            immediate,
        )
        .map_err(|error| error.to_string())?;
        let depth = resina_motion::sample_spring_trajectory(
            &request.channels.depth_scale.dynamics,
            request.channels.depth_scale.initial,
            request.target.depth_scale(),
            request.time,
            immediate,
        )
        .map_err(|error| error.to_string())?;
        request.channels.body_mix.initial = body.state();
        request.channels.depth_scale.initial = depth.state();
        Ok(
            json!({"channels":serialized_channels(&request.channels),"previous":{"bodyMix":body,"depthScale":depth}}),
        )
    })();
    QString::from(
        result
            .unwrap_or_else(|error: String| json!({"error":error}))
            .to_string(),
    )
}
