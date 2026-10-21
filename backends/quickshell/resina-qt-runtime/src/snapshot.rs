use crate::label::{PreparedContent, parse_label_request, prepare_content};
use cxx_qt_lib::QString;
use resina_model::{
    ActivationState, ColorRole, CommandAppearance, MaterialLightChannels, OpaqueSurfaceAppearance,
    PhysicalBounds, SurfaceIntent, SurfaceSize,
};
use resina_resolver::{
    CommandLabelIr, CommandMotionChannels, CommandMotionInput, CommandPaintInput,
    CommandSnapshotInput, ControlRelief, HitRegionBody, HitRegionIr, OpaqueSurfaceInput,
    ReliefReuse, SrgbFallback, SurfaceHitRegionInput, SurfacePaintInput, resolve_command_motion,
    resolve_command_paint, resolve_command_reservation_body, resolve_command_snapshot,
    resolve_command_states, resolve_srgb_fallback, resolve_surface_hit_region,
};
use resina_tokens::parse_token_document;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

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
    #[serde(deserialize_with = "light_source")]
    light: MaterialLightChannels,
    relief: ControlRelief,
    foreground_role: ColorRole,
    adjacent_color: Value,
    post_treatment_backdrop: Value,
    surrounding_color: Value,
    minimum_content_contrast: f64,
    minimum_edge_contrast: f64,
}

fn light_source<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<MaterialLightChannels, D::Error> {
    let source = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
    resina_resolver::parse_light_channels_source(source.get()).map_err(serde::de::Error::custom)
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
            "light": request["light"], "relief": request["relief"],
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

    #[test]
    fn command_body_rejects_negative_light_sources_even_when_suppressed() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/command-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let input = json!({
            "surface": body["surface"], "appearance": body["appearance"],
            "commandAppearance": request["commandAppearance"],
            "light": request["light"], "relief": request["relief"],
            "foregroundRole": body["foregroundRole"], "adjacentColor": body["adjacentColor"],
            "postTreatmentBackdrop": body["postTreatmentBackdrop"], "surroundingColor": request["surface"]["surroundingColor"],
            "minimumContentContrast": body["minimumContentContrast"], "minimumEdgeContrast": body["minimumEdgeContrast"],
        });
        for pointer in ["/light/ambient", "/light/interaction/strength"] {
            let mut changed = input.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("coefficient-source");
            let source = changed.to_string();
            for (literal, valid) in [("-1e-400", false), ("-0e-400", true), ("1e-400", true)] {
                let text = source.replace("\"coefficient-source\"", literal);
                assert_eq!(
                    serde_json::from_str::<Body>(&text).is_ok(),
                    valid,
                    "{pointer} {literal}"
                );
            }
        }
        for member in ["light", "relief"] {
            let mut missing = input.clone();
            missing.as_object_mut().unwrap().remove(member);
            assert!(serde_json::from_str::<Body>(&missing.to_string()).is_err());
        }
    }
}

struct ParsedRequest {
    request: Request,
    description: Option<String>,
    adjacent: SrgbFallback,
    backdrop: Option<SrgbFallback>,
    surrounding: Option<SrgbFallback>,
}

thread_local! {
    // Motion frames resolve the same request bytes; only successful results are kept, so every
    // failure is diagnosed again exactly as before.
    static PARSED: RefCell<Option<(String, Rc<ParsedRequest>)>> = const { RefCell::new(None) };
    static RESERVATION: RefCell<Option<(String, CommandLabelIr, HitRegionIr)>> =
        const { RefCell::new(None) };
    // Motion frames paint the same lit faces; their relief terms are kept between them.
    static RELIEF: ReliefReuse = ReliefReuse::default();
}

#[cfg(test)]
thread_local! {
    static REQUEST_PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TARGET_RESOLUTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn parse_request(source: &str) -> Result<Rc<ParsedRequest>, String> {
    let cached = PARSED
        .try_with(|entry| {
            entry
                .borrow()
                .as_ref()
                .filter(|(cached, _)| cached == source)
                .map(|(_, parsed)| Rc::clone(parsed))
        })
        .ok()
        .flatten();
    if let Some(parsed) = cached {
        return Ok(parsed);
    }
    #[cfg(test)]
    REQUEST_PARSES.set(REQUEST_PARSES.get() + 1);
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
        serde_json::from_value(request.description.clone()).map_err(|error| error.to_string())?;
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
    let parsed = Rc::new(ParsedRequest {
        request,
        description,
        adjacent,
        backdrop,
        surrounding,
    });
    let _ = PARSED
        .try_with(|entry| *entry.borrow_mut() = Some((source.to_owned(), Rc::clone(&parsed))));
    Ok(parsed)
}

fn paint_input<'a>(
    parsed: &'a ParsedRequest,
    content: &'a PreparedContent,
    surface: &'a SurfaceIntent,
) -> CommandPaintInput<'a> {
    let body = &parsed.request.body;
    CommandPaintInput {
        surface: SurfacePaintInput {
            body: OpaqueSurfaceInput {
                surface,
                size: content.label.size(),
                appearance: &body.appearance,
                foreground_role: body.foreground_role,
                post_treatment_backdrop: parsed.backdrop.as_ref(),
                adjacent_color: &parsed.adjacent,
                minimum_content_contrast: body.minimum_content_contrast,
                minimum_edge_contrast: body.minimum_edge_contrast,
            },
            surrounding_color: parsed.surrounding.as_ref(),
        },
        command_appearance: &body.command_appearance,
        light: &body.light,
        relief: &body.relief,
    }
}

fn target_region(
    source: &str,
    parsed: &ParsedRequest,
    content: &PreparedContent,
) -> Result<HitRegionIr, String> {
    let cached = RESERVATION
        .try_with(|entry| {
            entry
                .borrow()
                .as_ref()
                .filter(|(cached, measured, _)| cached == source && measured == &content.label)
                .map(|(_, _, region)| *region)
        })
        .ok()
        .flatten();
    if let Some(region) = cached {
        return Ok(region);
    }
    #[cfg(test)]
    TARGET_RESOLUTIONS.set(TARGET_RESOLUTIONS.get() + 1);
    let request = &parsed.request;
    let deepest = resolve_command_reservation_body(
        &content.theme,
        &content.environment,
        paint_input(parsed, content, &request.body.surface),
    )
    .map_err(|error| error.to_string())?;
    let occupied_regions: Vec<_> = request
        .occupied_regions
        .iter()
        .map(Rectangle::bounds)
        .collect();
    let region = resolve_surface_hit_region(SurfaceHitRegionInput {
        environment: &content.environment,
        body: HitRegionBody::Lit(&deepest),
        available_bounds: request.available_bounds.bounds(),
        component_minimum: request.component_minimum,
        occupied_regions: &occupied_regions,
    })
    .map_err(|error| error.to_string())?;
    let _ = RESERVATION.try_with(|entry| {
        *entry.borrow_mut() = Some((source.to_owned(), content.label.clone(), region))
    });
    Ok(region)
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
    let parsed = parse_request(source)?;
    let request = &parsed.request;
    let content = prepare_content(parse_label_request(request.content.clone())?, producer)?;
    let available_bounds = request.available_bounds.bounds();
    let occupied_regions: Vec<_> = request
        .occupied_regions
        .iter()
        .map(Rectangle::bounds)
        .collect();
    let hit_region = target_region(source, &parsed, &content)?;
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
                    command: paint_input(&parsed, &content, &surface),
                    channels: &motion.channels,
                    time: motion.time,
                },
            )
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let paint = match &sampled {
        Some(sample) => sample.command().clone(),
        None => resolve_command_paint(
            &content.theme,
            &content.environment,
            paint_input(&parsed, &content, &surface),
        )
        .map_err(|error| error.to_string())?,
    };
    let snapshot = resolve_command_snapshot(CommandSnapshotInput {
        label: &content.label,
        paint: &paint,
        hit_region,
        activation: &request.activation,
        hovered: request.hovered,
        description: parsed.description.as_deref(),
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
        resina_qml::prepare_lit_control_paint(
            snapshot.paint().paint(),
            placement,
            options.device_scale,
            options.samples_per_axis,
        )
        .map_err(|error| error.to_string())?;
        if render {
            let source = RELIEF
                .with(|reuse| {
                    resina_qml::render_lit_control_paint(
                        snapshot.paint().paint(),
                        resina_model::PhysicalVector {
                            x: options.x,
                            y: options.y,
                        },
                        options.device_scale,
                        options.samples_per_axis,
                        reuse,
                    )
                })
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

#[cfg(test)]
mod input_cache_tests {
    use super::*;
    use resina_environment::{EnvironmentSnapshot, SafeArea};
    use resina_model::TypographyRole;
    use resina_resolver::{CommandLabelInput, resolve_command_label};

    fn request(text: &str) -> String {
        let headless: Value = serde_json::from_str(include_str!(
            "../../../../conformance/headless/valid-request.json"
        ))
        .unwrap();
        let definition: Value = serde_json::from_str(include_str!(
            "../../../../definitions/components/command.json"
        ))
        .unwrap();
        let body = &definition["variants"]["standard"]["body"];
        let white = json!({"colorSpace": "srgb", "components": [1, 1, 1], "alpha": 1});
        json!({
            "content": {
                "themeSource": include_str!("../../../../tokens/themes/light.json"),
                "externalSources": {"foundation.json": include_str!("../../../../tokens/foundation.json")},
                "environment": headless["environment"], "text": text,
                "fontFamily": {"role": "sans", "name": "DejaVu Sans"},
                "minimumSize": {"width": 160, "height": 80}, "maximumSize": {"width": 240, "height": 600},
                "padding": {"start": 32, "end": 32, "top": 26, "bottom": 26}
            },
            "body": {
                "surface": {"schemaVersion": "0.2.0", "materialRole": body["materialRole"],
                    "colorRole": body["colorRole"],
                    "form": {"schemaVersion": "0.1.0", "shape": body["shape"], "elevation": body["elevation"]},
                    "states": {"schemaVersion": "0.1.0", "states": ["rest"]},
                    "treatmentStack": {"schemaVersion": "0.1.0", "treatments": ["none"]}},
                "appearance": serde_json::from_str::<Value>(include_str!("../../../../definitions/tier0-surface-appearance.json")).unwrap(),
                "commandAppearance": serde_json::from_str::<Value>(include_str!("../../../../definitions/command-appearance-light.json")).unwrap(),
                "light": {"ambient": null, "interaction": null},
                "relief": serde_json::from_str::<Value>(include_str!("../../../../definitions/control-relief.json")).unwrap(),
                "foregroundRole": body["contentRole"], "adjacentColor": white,
                "postTreatmentBackdrop": white, "surroundingColor": white,
                "minimumContentContrast": 4.5, "minimumEdgeContrast": 3
            },
            "activation": {"schemaVersion": "0.1.0", "enabled": true, "focused": false, "hold": null},
            "hovered": false, "description": "Connect using saved settings.", "focusable": true,
            "availableBounds": {"x": -100, "y": -100, "width": 1000, "height": 1000},
            "componentMinimum": {"width": 48, "height": 48}, "occupiedRegions": []
        })
        .to_string()
    }

    fn content(source: &str, extent: SurfaceSize) -> PreparedContent {
        let document: Value = serde_json::from_str(source).unwrap();
        let content = &document["content"];
        let external = serde_json::from_value(content["externalSources"].clone()).unwrap();
        let theme =
            crate::label::compiled_theme(content["themeSource"].as_str().unwrap(), &external)
                .unwrap();
        let environment: EnvironmentSnapshot =
            serde_json::from_value(content["environment"].clone()).unwrap();
        let resolved = theme.resolve(&environment).unwrap();
        let label = resolve_command_label(
            CommandLabelInput {
                text: content["text"].as_str().unwrap(),
                typography: &resolved.typography()[&TypographyRole::Label],
                minimum_size: SurfaceSize {
                    width: 160.0,
                    height: 80.0,
                },
                maximum_size: SurfaceSize {
                    width: 240.0,
                    height: 600.0,
                },
                padding: SafeArea {
                    start: 32.0,
                    end: 32.0,
                    top: 26.0,
                    bottom: 26.0,
                },
                direction: environment.layout_direction(),
            },
            |_| Ok::<_, String>(extent),
        )
        .unwrap();
        PreparedContent {
            label,
            theme,
            environment,
        }
    }

    #[test]
    fn identical_request_bytes_reuse_the_validated_request() {
        let source = request("Connect");
        let parses = REQUEST_PARSES.get();
        let first = parse_request(&source).unwrap();
        assert!(Rc::ptr_eq(&first, &parse_request(&source).unwrap()));
        assert_eq!(REQUEST_PARSES.get(), parses + 1);
        let other = parse_request(&request("Disconnect")).unwrap();
        assert!(!Rc::ptr_eq(&first, &other));
        assert_eq!(REQUEST_PARSES.get(), parses + 2);
    }

    #[test]
    fn a_failing_request_is_diagnosed_again_and_keeps_the_entry() {
        let source = request("Connect");
        let kept = parse_request(&source).unwrap();
        let broken = source.replacen("\"hovered\":false", "\"hovered\":0", 1);
        assert_ne!(broken, source);
        let parses = REQUEST_PARSES.get();
        let first = parse_request(&broken).err().unwrap();
        assert_eq!(parse_request(&broken).err().unwrap(), first);
        assert_eq!(REQUEST_PARSES.get(), parses + 2);
        assert!(Rc::ptr_eq(&kept, &parse_request(&source).unwrap()));
    }

    #[test]
    fn the_target_is_reused_only_for_equal_request_bytes_and_label() {
        let source = request("Connect");
        let parsed = parse_request(&source).unwrap();
        let measured = content(
            &source,
            SurfaceSize {
                width: 58.0,
                height: 17.0,
            },
        );
        let resolutions = TARGET_RESOLUTIONS.get();
        let region = target_region(&source, &parsed, &measured).unwrap();
        assert_eq!(target_region(&source, &parsed, &measured).unwrap(), region);
        assert_eq!(TARGET_RESOLUTIONS.get(), resolutions + 1);
        let wider = content(
            &source,
            SurfaceSize {
                width: 150.0,
                height: 17.0,
            },
        );
        assert_ne!(wider.label, measured.label);
        target_region(&source, &parsed, &wider).unwrap();
        assert_eq!(TARGET_RESOLUTIONS.get(), resolutions + 2);
        let moved = source.replacen("\"x\":-100", "\"x\":-90", 1);
        assert_ne!(moved, source);
        let moved_parsed = parse_request(&moved).unwrap();
        target_region(&moved, &moved_parsed, &wider).unwrap();
        assert_eq!(TARGET_RESOLUTIONS.get(), resolutions + 3);
    }
}
