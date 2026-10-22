use crate::label::{PreparedContent, parse_label_request, prepare_content};
use crate::snapshot::{RELIEF, Rectangle, RenderOptions, light_source};
use cxx_qt_lib::QString;
use resina_environment::SafeArea;
use resina_model::{
    ActivationState, ColorRole, CommandAppearance, MaterialLightChannels, OpaqueSurfaceAppearance,
    PhysicalBounds, PhysicalVector, SurfaceIntent, SurfaceSize,
};
use resina_resolver::{
    CommandLabelIr, CommandMotionChannel, CommandMotionChannels, ControlRelief, HitRegionInput,
    HitRegionIr, OpaqueSurfaceInput, SrgbFallback, SurfacePaintInput, ToggleLayoutInput,
    ToggleLayoutIr, TogglePart, TogglePartMotionInput, TogglePartPaintInput, ToggleSnapshotInput,
    resolve_hit_region, resolve_srgb_fallback, resolve_toggle_layout, resolve_toggle_part_motion,
    resolve_toggle_part_paint, resolve_toggle_part_reservation_paint,
    resolve_toggle_snapshot as resolve_snapshot_ir, resolve_toggle_states, resolve_toggle_travel,
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
    #[serde(deserialize_with = "light_source")]
    light: MaterialLightChannels,
    relief: ControlRelief,
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
            "light": request["light"], "relief": request["relief"],
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

    #[test]
    fn toggle_part_rejects_negative_light_sources_even_when_suppressed() {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/toggle-part-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let input = json!({
            "surface": body["surface"], "appearance": body["appearance"],
            "checkedColorRole": request["checkedColorRole"], "interactionAppearance": request["interactionAppearance"],
            "light": request["light"], "relief": request["relief"],
            "foregroundRole": body["foregroundRole"],
            "minimumContentContrast": body["minimumContentContrast"], "minimumEdgeContrast": body["minimumEdgeContrast"],
        });
        for pointer in ["/light/ambient", "/light/interaction/strength"] {
            let mut changed = input.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("coefficient-source");
            let source = changed.to_string();
            for (literal, valid) in [("-1e-400", false), ("-0e-400", true), ("1e-400", true)] {
                let text = source.replace("\"coefficient-source\"", literal);
                assert_eq!(
                    serde_json::from_str::<Part>(&text).is_ok(),
                    valid,
                    "{pointer} {literal}"
                );
            }
        }
        for member in ["light", "relief"] {
            let mut missing = input.clone();
            missing.as_object_mut().unwrap().remove(member);
            assert!(serde_json::from_str::<Part>(&missing.to_string()).is_err());
        }
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

#[cfg(test)]
mod request_cache_tests {
    use super::*;

    fn source(checked: bool) -> String {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/toggle-part-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let part = json!({
            "surface": body["surface"], "appearance": body["appearance"],
            "checkedColorRole": request["checkedColorRole"],
            "interactionAppearance": request["interactionAppearance"],
            "light": request["light"], "relief": request["relief"],
            "foregroundRole": body["foregroundRole"],
            "minimumContentContrast": 4.5, "minimumEdgeContrast": 3,
        });
        let bounds = json!({"x": 0, "y": 0, "width": 200, "height": 100});
        json!({
            "content": {}, "track": part, "thumb": part,
            "layout": {"trackSize": {"width": 64, "height": 40}, "thumbSize": {"width": 16, "height": 16},
                "insets": {"start": 12, "end": 12, "top": 12, "bottom": 12}},
            "labelOrigin": {"x": 80, "y": 0}, "labelForegroundRole": "content.primary",
            "background": body["adjacentColor"], "minimumLabelContrast": 4.5, "minimumThumbContrast": 3,
            "targetBounds": bounds, "availableBounds": bounds,
            "activation": {"schemaVersion": "0.1.0", "enabled": true, "focused": false, "hold": null},
            "hovered": false, "checked": checked, "description": null, "focusable": true,
            "componentMinimum": {"width": 48, "height": 48}, "occupiedRegions": [],
        })
        .to_string()
    }

    #[test]
    fn one_source_is_parsed_once_and_a_changed_or_invalid_source_is_not_kept() {
        let first = parse_request(&source(false)).unwrap();
        assert!(Rc::ptr_eq(&first, &parse_request(&source(false)).unwrap()));
        let changed = parse_request(&source(true)).unwrap();
        assert!(!Rc::ptr_eq(&first, &changed));
        assert!(changed.checked && !first.checked);
        assert!(parse_request("{").is_err());
        assert!(parse_request("{}").is_err());
        assert!(Rc::ptr_eq(&changed, &parse_request(&source(true)).unwrap()));
    }
}

thread_local! {
    // Frames of one motion repeat one request source, so its parse and its validated reservation
    // are kept. Only successes are kept: a failure is diagnosed again exactly as before.
    static PARSED: RefCell<Option<(String, Rc<Request>)>> = const { RefCell::new(None) };
    static RESERVED: RefCell<Option<(String, CommandLabelIr)>> = const { RefCell::new(None) };
}

fn parse_request(source: &str) -> Result<Rc<Request>, String> {
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
    document(source)?;
    let request: Rc<Request> =
        Rc::new(serde_json::from_str(source).map_err(|error| error.to_string())?);
    let _ =
        PARSED.try_with(|entry| *entry.borrow_mut() = Some((source.to_owned(), request.clone())));
    Ok(request)
}

/// Resolves both parts at the projected depth bound and requires the complete snapshot checks to
/// pass with them, so that the reserved target covers every state the control can paint.
fn validate_reservation(
    request: &Request,
    content: &PreparedContent,
    label_foreground: &SrgbFallback,
    background: &SrgbFallback,
    (layout, target): (&ToggleLayoutIr, HitRegionIr),
    occupied: &[PhysicalBounds],
    description: Option<&str>,
) -> Result<(), String> {
    let neutral_focus =
        ActivationState::try_new(true, true, None).expect("valid focused neutral activation");
    let deepest_states = resolve_toggle_states(&neutral_focus, false, request.checked);
    let deepest_track_surface = request
        .track
        .surface
        .clone()
        .with_states(deepest_states.clone());
    let deepest_thumb_surface = request.thumb.surface.clone().with_states(deepest_states);
    let deepest_track = resolve_toggle_part_reservation_paint(
        &content.theme,
        &content.environment,
        TogglePartPaintInput {
            part: TogglePart::Track,
            checked_color_role: request.track.checked_color_role,
            interaction_appearance: &request.track.interaction_appearance,
            light: &request.track.light,
            relief: &request.track.relief,
            surface: SurfacePaintInput {
                surrounding_color: Some(background),
                body: OpaqueSurfaceInput {
                    surface: &deepest_track_surface,
                    size: request.layout.track_size,
                    appearance: &request.track.appearance,
                    foreground_role: request.track.foreground_role,
                    post_treatment_backdrop: Some(background),
                    adjacent_color: background,
                    minimum_content_contrast: request.track.minimum_content_contrast,
                    minimum_edge_contrast: request.track.minimum_edge_contrast,
                },
            },
        },
    )
    .map_err(|error| error.to_string())?;
    let deepest_track_pigment = deepest_track.paint().body().face().pigment().body();
    let deepest_thumb = resolve_toggle_part_reservation_paint(
        &content.theme,
        &content.environment,
        TogglePartPaintInput {
            part: TogglePart::Thumb,
            checked_color_role: request.thumb.checked_color_role,
            interaction_appearance: &request.thumb.interaction_appearance,
            light: &request.thumb.light,
            relief: &request.thumb.relief,
            surface: SurfacePaintInput {
                surrounding_color: None,
                body: OpaqueSurfaceInput {
                    surface: &deepest_thumb_surface,
                    size: request.layout.thumb_size,
                    appearance: &request.thumb.appearance,
                    foreground_role: request.thumb.foreground_role,
                    post_treatment_backdrop: Some(deepest_track_pigment),
                    adjacent_color: deepest_track_pigment,
                    minimum_content_contrast: request.thumb.minimum_content_contrast,
                    minimum_edge_contrast: request.thumb.minimum_edge_contrast,
                },
            },
        },
    )
    .map_err(|error| error.to_string())?;
    resolve_snapshot_ir(ToggleSnapshotInput {
        label: &content.label,
        label_origin: request.label_origin,
        label_foreground,
        label_background: background,
        minimum_label_contrast: request.minimum_label_contrast,
        layout,
        track: &deepest_track,
        thumb: &deepest_thumb,
        hit_region: target,
        activation: &neutral_focus,
        hovered: false,
        checked: request.checked,
        description,
        focusable: true,
        minimum_thumb_contrast: request.minimum_thumb_contrast,
        environment: &content.environment,
        available_bounds: request.available_bounds.bounds(),
        component_minimum: request.component_minimum,
        occupied_regions: occupied,
    })
    .map_err(|error| {
        format!(
            "Qt Toggle layout or reservation does not cover the deepest part silhouettes: {error}"
        )
    })?;
    Ok(())
}

fn prepare(
    source: &str,
    producer: &ffi::NativeLabelMeasure,
    options: &str,
    render: bool,
    motion: Option<&MotionRequest>,
) -> Result<Value, String> {
    let request = parse_request(source)?;
    if options.len() > 64 * 1024 {
        return Err("Qt Toggle rendering options exceed 64 KiB".to_owned());
    }
    let options = parse_token_document(options).map_err(|error| error.to_string())?;
    if !options.is_object() {
        return Err("Qt Toggle rendering options must be an object".to_owned());
    }
    let options: RenderOptions =
        serde_json::from_value(options).map_err(|error| error.to_string())?;
    let content = prepare_content(parse_label_request(request.content.clone())?, producer)?;
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
        light: &request.track.light,
        relief: &request.track.relief,
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
    let track_pigment = track.paint().body().face().pigment().body();
    let thumb_input = TogglePartPaintInput {
        part: TogglePart::Thumb,
        checked_color_role: request.thumb.checked_color_role,
        interaction_appearance: &request.thumb.interaction_appearance,
        light: &request.thumb.light,
        relief: &request.thumb.relief,
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
        serde_json::from_value(request.description.clone()).map_err(|error| error.to_string())?;
    let reserved = RESERVED
        .try_with(|entry| {
            entry
                .borrow()
                .as_ref()
                .is_some_and(|(cached, label)| cached == source && label == &content.label)
        })
        .unwrap_or(false);
    if !reserved {
        validate_reservation(
            &request,
            &content,
            label_foreground,
            &background,
            (&layout, target),
            &occupied,
            description.as_deref(),
        )?;
        let _ = RESERVED.try_with(|entry| {
            *entry.borrow_mut() = Some((source.to_owned(), content.label.clone()))
        });
    }
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
        resina_qml::prepare_lit_control_paint(
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
        for (name, paint, origin) in [
            ("trackSource", snapshot.track().paint(), track_origin),
            ("thumbSource", snapshot.thumb().paint(), thumb_origin),
        ] {
            let source = RELIEF
                .with(|reuse| {
                    resina_qml::render_lit_control_paint(
                        paint,
                        origin,
                        options.device_scale,
                        options.samples_per_axis,
                        reuse,
                    )
                })
                .map_err(|error| error.to_string())?;
            result[name] = Value::String(source);
        }
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
