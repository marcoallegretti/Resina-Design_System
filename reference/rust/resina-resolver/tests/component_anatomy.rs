use resina_color::OpaqueSrgbRange;
use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAnatomy, CommandAppearance, CommandBody, CommandEmphasis,
    MaterialLightChannels, OpaqueSurfaceAppearance, PhysicalVector, SliderAnatomy,
    SliderAppearance, SliderPart, StateSet, SurfaceIntent, SurfaceSize, ToggleAnatomy,
};
use resina_resolver::{
    CommandPaintError, CommandPaintInput, CommandPaintIr, CompiledTheme, ControlRelief,
    LitControlBodyIr, OpaqueSurfaceInput, SliderPartPaintInput, SliderPartPaintIr, SrgbFallback,
    SurfacePaintInput, TogglePart, TogglePartPaintInput, TogglePartPaintIr,
    compile_theme_source_with_sources, opaque_contrast_ratio, resolve_command_paint,
    resolve_slider_part_paint, resolve_toggle_part_paint,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const FAMILIES: [&str; 3] = ["cast", "frost", "elastomer"];
const PHASES: [&str; 4] = ["rest", "hover", "pressed", "disabled"];
const COMMAND_SIZE: SurfaceSize = SurfaceSize {
    width: 96.0,
    height: 48.0,
};
const TRACK_SIZE: SurfaceSize = SurfaceSize {
    width: 64.5,
    height: 40.5,
};
const THUMB_SIZE: SurfaceSize = SurfaceSize {
    width: 16.0,
    height: 16.0,
};
const SLIDER_TRACK_SIZE: SurfaceSize = SurfaceSize {
    width: 160.0,
    height: 8.0,
};
const SLIDER_THUMB_SIZE: SurfaceSize = SurfaceSize {
    width: 20.0,
    height: 20.0,
};

struct Theme {
    name: &'static str,
    assignment: Option<[&'static str; 3]>,
    theme: CompiledTheme,
    canvas: SrgbFallback,
    colors: BTreeMap<ColorRole, SrgbFallback>,
}

fn environment() -> EnvironmentSnapshot {
    serde_json::from_str(include_str!(
        "../../../../conformance/environment/valid-minimal-capabilities.json"
    ))
    .unwrap()
}

/// The minimal-capability snapshot with every condition that admits interaction light.
fn admitting_environment() -> EnvironmentSnapshot {
    let mut source: Value = serde_json::from_str(include_str!(
        "../../../../conformance/environment/valid-minimal-capabilities.json"
    ))
    .unwrap();
    source["accessibilityPreferences"]["highContrast"] = json!(false);
    source["accessibilityPreferences"]["reducedMotion"] = json!(false);
    source["rendererCapabilities"]["dynamicLighting"] = json!(true);
    source["qualityPolicy"] = json!("balanced");
    serde_json::from_value(source).unwrap()
}

/// The authored themes, then every Cast/Frost/Elastomer choice for the three control roles.
fn themes(environment: &EnvironmentSnapshot) -> Vec<Theme> {
    let sources = BTreeMap::from([(
        "foundation.json".to_owned(),
        include_str!("../../../../tokens/foundation.json").to_owned(),
    )]);
    let mut assignments = vec![None];
    for passive in FAMILIES {
        for interactive in FAMILIES {
            for primary in FAMILIES {
                assignments.push(Some([passive, interactive, primary]));
            }
        }
    }
    let mut themes = Vec::new();
    for (name, source) in [
        (
            "light",
            include_str!("../../../../tokens/themes/light.json"),
        ),
        ("dark", include_str!("../../../../tokens/themes/dark.json")),
    ] {
        for assignment in &assignments {
            let mut source: Value = serde_json::from_str(source).unwrap();
            if let Some([passive, interactive, primary]) = assignment {
                source["materialAssignments"]["control"] =
                    json!({"passive": passive, "interactive": interactive, "primary": primary});
            }
            let theme = compile_theme_source_with_sources(&source.to_string(), &sources).unwrap();
            let snapshot = theme.resolve(environment).unwrap();
            let colors = snapshot.opaque_color_fallbacks();
            themes.push(Theme {
                name,
                assignment: *assignment,
                canvas: colors[&ColorRole::SurfaceBase].clone(),
                colors: colors.clone(),
                theme,
            });
        }
    }
    themes
}

fn reshaped(intent: SurfaceIntent, shape: Option<&str>) -> SurfaceIntent {
    let Some(shape) = shape else {
        return intent;
    };
    let mut source = serde_json::to_value(&intent).unwrap();
    source["form"]["shape"] = json!(shape);
    let reshaped: SurfaceIntent = serde_json::from_value(source).unwrap();
    assert_eq!(
        serde_json::to_value(&reshaped).unwrap()["form"]["shape"],
        shape
    );
    reshaped
}

fn states(names: &[&str]) -> StateSet {
    serde_json::from_value(json!({"schemaVersion": "0.1.0", "states": names})).unwrap()
}

fn appearance(theme: &str) -> CommandAppearance {
    serde_json::from_str(if theme == "dark" {
        include_str!("../../../../definitions/command-appearance-dark.json")
    } else {
        include_str!("../../../../definitions/command-appearance-light.json")
    })
    .unwrap()
}

fn surface_appearance() -> OpaqueSurfaceAppearance {
    serde_json::from_str(include_str!(
        "../../../../definitions/tier0-surface-appearance.json"
    ))
    .unwrap()
}

fn command_anatomy() -> CommandAnatomy {
    serde_json::from_str(include_str!(
        "../../../../definitions/components/command.json"
    ))
    .unwrap()
}

fn toggle_anatomy() -> ToggleAnatomy {
    serde_json::from_str(include_str!(
        "../../../../definitions/components/toggle.json"
    ))
    .unwrap()
}

fn slider_anatomy() -> SliderAnatomy {
    serde_json::from_str(include_str!(
        "../../../../definitions/components/slider.json"
    ))
    .unwrap()
}

fn slider_responses() -> SliderAppearance {
    serde_json::from_str(include_str!(
        "../../../../conformance/appearance/slider-appearance.json"
    ))
    .unwrap()
}

fn exact_range(color: &SrgbFallback) -> OpaqueSrgbRange {
    OpaqueSrgbRange::try_new(color.clone(), color.clone()).unwrap()
}

/// Resolves the track over the canvas, then the thumb over the canvas and the track. A thumb is
/// wider than the track, so it borders both. `whole_cover` takes every colour the track paints,
/// its exterior edge included, as the snapshot's conservative check does; otherwise the thumb
/// borders the track's resolved body, as the Toggle thumb does.
fn slider(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    anatomy: &SliderAnatomy,
    track_states: &[&str],
    thumb_states: &[&str],
    whole_cover: bool,
) -> Result<[SliderPartPaintIr; 2], String> {
    let surface = surface_appearance();
    let responses = slider_responses();
    let canvas = [exact_range(&theme.canvas)];
    let track_intent = anatomy.track().intent(states(track_states));
    let track = resolve_slider_part_paint(
        &theme.theme,
        environment,
        SliderPartPaintInput {
            part: SliderPart::Track,
            read_only: false,
            surface: &track_intent,
            size: SLIDER_TRACK_SIZE,
            appearance: &surface,
            interaction_appearance: &responses,
            foreground_role: anatomy.track().content_role(),
            post_treatment_backdrop: Some(&theme.canvas),
            adjacent_ranges: &canvas,
            surrounding_ranges: Some(&canvas),
            minimum_content_contrast: 1.0,
            minimum_edge_contrast: 3.0,
        },
    )
    .map_err(|error| format!("{} {:?} track: {error}", theme.name, theme.assignment))?;
    let mut around = if whole_cover {
        track.paint().body().paint_color_ranges()
    } else {
        vec![exact_range(track.paint().body().pigment().body())]
    };
    around.extend(canvas);
    let thumb_intent = anatomy.thumb().intent(states(thumb_states));
    let thumb = resolve_slider_part_paint(
        &theme.theme,
        environment,
        SliderPartPaintInput {
            part: SliderPart::Thumb,
            read_only: false,
            surface: &thumb_intent,
            size: SLIDER_THUMB_SIZE,
            appearance: &surface,
            interaction_appearance: &responses,
            foreground_role: anatomy.thumb().content_role(),
            post_treatment_backdrop: Some(track.paint().body().pigment().body()),
            adjacent_ranges: &around,
            surrounding_ranges: Some(&around),
            minimum_content_contrast: 1.0,
            minimum_edge_contrast: 3.0,
        },
    )
    .map_err(|error| format!("{} {:?} thumb: {error}", theme.name, theme.assignment))?;
    Ok([track, thumb])
}

fn light(ambient: Value, interaction: Value) -> MaterialLightChannels {
    serde_json::from_value(json!({"ambient": ambient, "interaction": interaction})).unwrap()
}

fn command(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    body: &CommandBody,
    names: &[&str],
    light: &MaterialLightChannels,
) -> Result<CommandPaintIr, CommandPaintError> {
    command_of(theme, environment, body, &body.intent(states(names)), light)
}

fn command_of(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    body: &CommandBody,
    intent: &SurfaceIntent,
    light: &MaterialLightChannels,
) -> Result<CommandPaintIr, CommandPaintError> {
    let relief: ControlRelief =
        serde_json::from_str(include_str!("../../../../definitions/control-relief.json")).unwrap();
    resolve_command_paint(
        &theme.theme,
        environment,
        CommandPaintInput {
            surface: SurfacePaintInput {
                body: OpaqueSurfaceInput {
                    surface: intent,
                    size: COMMAND_SIZE,
                    appearance: &surface_appearance(),
                    foreground_role: body.content_role(),
                    post_treatment_backdrop: Some(&theme.canvas),
                    adjacent_color: &theme.canvas,
                    minimum_content_contrast: 4.5,
                    minimum_edge_contrast: 3.0,
                },
                surrounding_color: Some(&theme.canvas),
            },
            command_appearance: &appearance(theme.name),
            light,
            relief: &relief,
        },
    )
}

/// Resolves the track over the canvas, then the thumb over the actual track body: as in the
/// Toggle snapshot, the thumb edge must clear 3:1 against the painted track.
fn toggle(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    anatomy: &ToggleAnatomy,
    names: &[&str],
    light: &MaterialLightChannels,
) -> [TogglePartPaintIr; 2] {
    toggle_shaped(theme, environment, anatomy, names, light, None)
}

/// Resolves both parts with their bound shape replaced by `shape`, when given.
fn toggle_shaped(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    anatomy: &ToggleAnatomy,
    names: &[&str],
    light: &MaterialLightChannels,
    shape: Option<&str>,
) -> [TogglePartPaintIr; 2] {
    let surface = surface_appearance();
    let relief: ControlRelief =
        serde_json::from_str(include_str!("../../../../definitions/control-relief.json")).unwrap();
    let responses = appearance(theme.name);
    let part = |part: TogglePart, adjacent: &SrgbFallback| {
        let (anatomy_part, width, height) = match part {
            TogglePart::Track => (anatomy.track(), TRACK_SIZE.width, TRACK_SIZE.height),
            TogglePart::Thumb => (anatomy.thumb(), THUMB_SIZE.width, THUMB_SIZE.height),
        };
        let intent = reshaped(anatomy_part.intent(states(names)), shape);
        resolve_toggle_part_paint(
            &theme.theme,
            environment,
            TogglePartPaintInput {
                part,
                surface: SurfacePaintInput {
                    body: OpaqueSurfaceInput {
                        surface: &intent,
                        size: SurfaceSize { width, height },
                        appearance: &surface,
                        foreground_role: anatomy_part.content_role(),
                        post_treatment_backdrop: Some(adjacent),
                        adjacent_color: adjacent,
                        minimum_content_contrast: 1.0,
                        minimum_edge_contrast: 3.0,
                    },
                    surrounding_color: Some(&theme.canvas),
                },
                checked_color_role: anatomy_part.checked_color_role(),
                interaction_appearance: &responses,
                light,
                relief: &relief,
            },
        )
        .unwrap_or_else(|error| panic!("{} {part:?} {names:?}: {error}", theme.name))
    };
    let track = part(TogglePart::Track, &theme.canvas);
    let thumb = part(
        TogglePart::Thumb,
        track.paint().body().face().pigment().body(),
    );
    [track, thumb]
}

fn oklab_difference(first: &SrgbFallback, second: &SrgbFallback) -> f64 {
    let first = resina_color::srgb_to_oklab(first.components()).unwrap();
    let second = resina_color::srgb_to_oklab(second.components()).unwrap();
    first
        .iter()
        .zip(second)
        .map(|(first, second)| (first - second).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[test]
fn command_anatomy_resolves_every_state_in_the_authored_themes() {
    let anatomy = command_anatomy();
    let environment = environment();
    let mut resolved = 0;
    let lights = [
        light(Value::Null, Value::Null),
        light(
            json!(0.25),
            json!({"direction": {"x": 1, "y": 0.5}, "strength": 0.5}),
        ),
    ];
    for theme in themes(&environment) {
        for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
            let body = anatomy.variant(emphasis).body();
            for phase in PHASES {
                for focused in [false, true] {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(focused.then_some("focused"))
                        .collect();
                    for light in &lights {
                        if let Err(error) = command(&theme, &environment, body, &names, light) {
                            panic!("{} {emphasis:?} {names:?}: {error}", theme.name);
                        }
                        resolved += 1;
                    }
                }
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 2 * 4 * 2 * 2);
}

#[test]
fn toggle_anatomy_resolves_every_state_with_an_identifiable_thumb_edge() {
    let anatomy = toggle_anatomy();
    let environment = environment();
    let mut resolved = 0;
    let lights = [
        light(Value::Null, Value::Null),
        light(
            json!(0.25),
            json!({"direction": {"x": 1, "y": 0.5}, "strength": 0.5}),
        ),
    ];
    for theme in themes(&environment) {
        assert!(
            opaque_contrast_ratio(&theme.colors[&anatomy.label_color()], &theme.canvas).unwrap()
                >= 4.5
        );
        for checked in [false, true] {
            for phase in PHASES {
                for focused in [false, true] {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(focused.then_some("focused"))
                        .chain(checked.then_some("checked"))
                        .collect();
                    for light in &lights {
                        toggle(&theme, &environment, &anatomy, &names, light);
                        resolved += 1;
                    }
                }
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 2 * 4 * 2 * 2);
}

#[test]
fn slider_anatomy_resolves_at_rest_with_identifiable_edges() {
    let anatomy = slider_anatomy();
    let environment = environment();
    let mut resolved = 0;
    for theme in themes(&environment) {
        assert!(
            opaque_contrast_ratio(&theme.colors[&anatomy.label_color()], &theme.canvas).unwrap()
                >= 4.5
        );
        for track_states in [&["rest"][..], &["rest", "focused"]] {
            for thumb_states in [&["rest"][..], &["rest", "focused"]] {
                let [track, thumb] = slider(
                    &theme,
                    &environment,
                    &anatomy,
                    track_states,
                    thumb_states,
                    false,
                )
                .unwrap();
                assert!(track.paint().focus().is_none());
                assert_eq!(
                    thumb.paint().focus().is_some(),
                    thumb_states.contains(&"focused")
                );
                for (part, paint) in [("track", &track), ("thumb", &thumb)] {
                    let ratio = paint.paint().body().edge().contrast_ratio();
                    assert!(
                        ratio >= 3.0,
                        "{} {:?} {part} edge {ratio}",
                        theme.name,
                        theme.assignment
                    );
                }
                resolved += 1;
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 4);
}

/// The conservative whole-track check of the Slider snapshot admits no thumb under the authored
/// themes. The track's own exterior edge is among the colours around the thumb, and the thumb's
/// edge is chosen from the same outline roles, so no outline reaches 3:1 against it.
#[test]
fn whole_track_cover_leaves_the_thumb_edge_below_the_guard() {
    let anatomy = slider_anatomy();
    let environment = environment();
    let mut rejected = 0;
    for theme in themes(&environment) {
        let error = slider(&theme, &environment, &anatomy, &["rest"], &["rest"], true)
            .expect_err("the whole track cover cannot clear the thumb edge");
        assert!(error.contains("edge contrast is insufficient"), "{error}");
        rejected += 1;
    }
    assert_eq!(rejected, 2 * 28);
}

/// Body colour and final extrusion depth, the two channels through which a state may differ,
/// measured on the unlit body colour of a lit control body.
fn lit_channels(body: &resina_resolver::LitControlBodyIr) -> (SrgbFallback, f64) {
    (body.face().pigment().body().clone(), body.depth())
}

/// Measures every authored hover, pressed and disabled body against the rest body of the
/// same part, and the disabled body against the pressed body, for each body the shipped Command and Toggle anatomy bind in both themes and
/// under every family a theme may assign to each control role. A state is perceivable when
/// its body colour differs by at least ΔE_OK 0.04 or its final depth by at least 1 px.
#[test]
fn authored_state_responses_differ_perceivably_from_rest() {
    let commands = command_anatomy();
    let toggles = toggle_anatomy();
    let environment = environment();
    let mut failures = Vec::new();
    let mut measured = 0;
    for theme in themes(&environment) {
        let mut bodies = Vec::new();
        for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
            let body = commands.variant(emphasis).body();
            let painted = PHASES
                .into_iter()
                .map(|phase| {
                    command(
                        &theme,
                        &environment,
                        body,
                        &[phase],
                        &light(Value::Null, Value::Null),
                    )
                    .map(|paint| {
                        let body = paint.paint().body();
                        (body.material_family(), lit_channels(body))
                    })
                })
                .collect::<Result<Vec<_>, _>>();
            match painted {
                Ok(painted) => bodies.push((
                    format!("command {emphasis:?} {:?}", painted[0].0),
                    painted.into_iter().map(|(_, channels)| channels).collect(),
                )),
                Err(error) => failures.push(format!(
                    "{} {:?} command {emphasis:?}: {error}",
                    theme.name, theme.assignment
                )),
            }
        }
        for checked in [false, true] {
            let parts = PHASES.map(|phase| {
                let names: Vec<&str> = [phase]
                    .into_iter()
                    .chain(checked.then_some("checked"))
                    .collect();
                toggle(
                    &theme,
                    &environment,
                    &toggles,
                    &names,
                    &light(Value::Null, Value::Null),
                )
            });
            for (index, part) in [TogglePart::Track, TogglePart::Thumb]
                .into_iter()
                .enumerate()
            {
                let body = parts[0][index].paint().body();
                bodies.push((
                    format!(
                        "toggle {part:?} checked={checked} {:?} {:?}",
                        body.material_family(),
                        body.color_role()
                    ),
                    parts
                        .iter()
                        .map(|phases| lit_channels(phases[index].paint().body()))
                        .collect::<Vec<_>>(),
                ));
            }
        }
        for (label, painted) in bodies {
            for (from, to) in [(0, 1), (0, 2), (0, 3), (2, 3)] {
                let ((first, first_depth), (second, second_depth)) = (&painted[from], &painted[to]);
                let difference = oklab_difference(first, second);
                let depth_change = (second_depth - first_depth).abs();
                measured += 1;
                if difference < 0.04 && depth_change < 1.0 - 1e-12 {
                    failures.push(format!(
                        "{} {:?} {label} {} -> {}: ΔE_OK {difference:.4}, depth {first_depth} -> {second_depth} px",
                        theme.name, theme.assignment, PHASES[from], PHASES[to]
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {measured} state responses are imperceptible:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn full_light() -> MaterialLightChannels {
    light(
        json!(0.25),
        json!({"direction": {"x": 1, "y": 0.5}, "strength": 0.5}),
    )
}

fn without_light(mut body: Value) -> Value {
    body["face"].as_object_mut().unwrap().remove("light");
    body
}

/// The channels may change `face.light` and the sampled face only: every cue, guard, edge,
/// foreground and range stays, interaction light is admitted exactly in the hover and pressed
/// phases of an admitting environment, and the content rectangle keeps the exact body color.
fn assert_light_changes_only_the_face(
    label: &str,
    unlit: &LitControlBodyIr,
    lit: &LitControlBodyIr,
    phase: &str,
    admits: bool,
) {
    let lit_body = serde_json::to_value(lit).unwrap();
    assert_eq!(
        without_light(serde_json::to_value(unlit).unwrap()),
        without_light(lit_body.clone()),
        "{label}: light changed more than face.light"
    );
    let light = &lit_body["face"]["light"];
    assert_eq!(
        !light["interaction"].is_null(),
        admits && matches!(phase, "hover" | "pressed"),
        "{label}: interaction admission"
    );
    assert_eq!(
        light["ambient"].as_f64().unwrap() > 0.0,
        admits,
        "{label}: ambient admission"
    );
    let bounds = &lit_body["face"]["contentBounds"];
    let [x, y, width, height] =
        ["x", "y", "width", "height"].map(|name| bounds[name].as_f64().unwrap());
    for (px, py) in [
        (x, y),
        (x + width, y),
        (x, y + height),
        (x + width, y + height),
        (x + width / 2.0, y + height / 2.0),
    ] {
        assert_eq!(
            lit.sample_paint(PhysicalVector { x: px, y: py }).unwrap(),
            Some(lit.face().pigment().body().clone()),
            "{label}: content point ({px}, {py}) is not the exact body"
        );
    }
}

/// Light is never a state cue and never reaches the content plateau. For each authored body in
/// both themes and every family assignment, the full channels, admitted and suppressed, change
/// nothing but `face.light`, and the disabled Command label stays at its bound.
#[test]
fn light_never_changes_cues_content_or_the_disabled_bound() {
    let commands = command_anatomy();
    let toggles = toggle_anatomy();
    let unlit = light(Value::Null, Value::Null);
    let lit = full_light();
    let mut compared = 0;
    for (admits, high_contrast, environment) in [
        (false, true, environment()),
        (true, false, admitting_environment()),
    ] {
        let bound = if high_contrast { 4.5 } else { 3.0 };
        for theme in themes(&environment) {
            for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
                let body = commands.variant(emphasis).body();
                for phase in PHASES {
                    let label = format!(
                        "{} {:?} command {emphasis:?} {phase} admits={admits}",
                        theme.name, theme.assignment
                    );
                    let first = command(&theme, &environment, body, &[phase], &unlit).unwrap();
                    let second = command(&theme, &environment, body, &[phase], &lit).unwrap();
                    let face = second.paint().body().face();
                    let content = &theme.colors[&body.content_role()];
                    let ratio =
                        opaque_contrast_ratio(face.foreground(), face.pigment().body()).unwrap();
                    assert!(
                        (ratio - face.content_contrast_ratio()).abs() <= 1e-12,
                        "{label}: reported content contrast"
                    );
                    if phase == "disabled" {
                        let undimmed =
                            opaque_contrast_ratio(content, face.pigment().body()).unwrap();
                        if undimmed > bound + 1e-9 {
                            assert!(
                                (bound - 1e-12..=bound + 1e-9).contains(&ratio),
                                "{label}: disabled label contrast {ratio} is not the {bound}:1 bound"
                            );
                        } else {
                            assert_eq!(face.foreground(), content, "{label}: undimmed label");
                        }
                    } else {
                        assert_eq!(face.foreground(), content, "{label}: label color");
                        assert!(ratio >= 4.5, "{label}: content contrast {ratio}");
                    }
                    assert_light_changes_only_the_face(
                        &label,
                        first.paint().body(),
                        second.paint().body(),
                        phase,
                        admits,
                    );
                    compared += 1;
                }
            }
            for checked in [false, true] {
                for phase in PHASES {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(checked.then_some("checked"))
                        .collect();
                    let first = toggle(&theme, &environment, &toggles, &names, &unlit);
                    let second = toggle(&theme, &environment, &toggles, &names, &lit);
                    for (part, (first, second)) in first.iter().zip(&second).enumerate() {
                        assert_light_changes_only_the_face(
                            &format!(
                                "{} {:?} toggle part {part} {names:?} admits={admits}",
                                theme.name, theme.assignment
                            ),
                            first.paint().body(),
                            second.paint().body(),
                            phase,
                            admits,
                        );
                        compared += 1;
                    }
                }
            }
        }
    }
    assert_eq!(compared, 2 * 2 * 28 * (2 * 4 + 2 * 4 * 2));
}

/// Every shape of the bundled Tier 0 appearance carries every control relief on every authored
/// part, so that no assignment a theme may draw on reaches the spec/88 `tau < 1` rejection.
#[test]
fn every_bundled_shape_assignment_carries_every_control_relief() {
    let appearance: Value = serde_json::from_str(include_str!(
        "../../../../definitions/tier0-surface-appearance.json"
    ))
    .unwrap();
    let shapes: Vec<&str> = appearance["shapeAssignments"]["profiles"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        shapes,
        ["capsule", "organic", "rounded", "soft", "structural"]
    );
    let commands = command_anatomy();
    let toggles = toggle_anatomy();
    let environment = environment();
    let unlit = light(Value::Null, Value::Null);
    let mut resolved = 0;
    for theme in themes(&environment)
        .into_iter()
        .filter(|theme| theme.assignment.is_none())
    {
        for shape in &shapes {
            for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
                let body = commands.variant(emphasis).body();
                let intent = reshaped(body.intent(states(&["rest"])), Some(shape));
                command_of(&theme, &environment, body, &intent, &unlit)
                    .unwrap_or_else(|error| panic!("{} {shape} {emphasis:?}: {error}", theme.name));
                resolved += 1;
            }
            toggle_shaped(
                &theme,
                &environment,
                &toggles,
                &["rest"],
                &unlit,
                Some(shape),
            );
            resolved += 2;
        }
    }
    assert_eq!(resolved, 2 * 5 * 4);
}

/// The published peak visibility rows are the resolver's own: each row's rest body is the resolved
/// body, and the colors painted at the two witness points, measured against that body, are the
/// row's ΔE_OK. The table is evidence of how visible the static key is, not a pass threshold.
#[test]
fn published_visibility_table_is_what_the_resolver_paints() {
    let table: Value = serde_json::from_str(include_str!(
        "../../../../conformance/materials/lit-control-visibility.json"
    ))
    .unwrap();
    let commands = command_anatomy();
    let toggles = toggle_anatomy();
    let environment = environment();
    let themes = themes(&environment);
    let unlit = light(Value::Null, Value::Null);
    for (part, size) in [
        ("commandBody", COMMAND_SIZE),
        ("toggleTrack", TRACK_SIZE),
        ("toggleThumb", THUMB_SIZE),
    ] {
        assert_eq!(table["parts"][part]["size"]["width"], size.width);
        assert_eq!(table["parts"][part]["size"]["height"], size.height);
    }
    let rows = table["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2 * 3 * 6);
    for row in rows {
        let (theme_name, family) = (
            row["theme"].as_str().unwrap(),
            row["materialFamily"].as_str().unwrap(),
        );
        let theme = themes
            .iter()
            .find(|theme| {
                theme.name == theme_name
                    && theme
                        .assignment
                        .is_some_and(|assignment| assignment.iter().all(|name| *name == family))
            })
            .unwrap();
        let checked = row["checked"].as_bool().unwrap_or(false);
        let names: Vec<&str> = ["rest"]
            .into_iter()
            .chain(checked.then_some("checked"))
            .collect();
        let part = row["part"].as_str().unwrap();
        let body = match part {
            "commandBody" => {
                let emphasis = match row["variant"].as_str().unwrap() {
                    "standard" => CommandEmphasis::Standard,
                    _ => CommandEmphasis::Primary,
                };
                let body = commands.variant(emphasis).body();
                command(theme, &environment, body, &names, &unlit)
                    .unwrap()
                    .paint()
                    .body()
                    .clone()
            }
            _ => {
                let [track, thumb] = toggle(theme, &environment, &toggles, &names, &unlit);
                if part == "toggleTrack" { track } else { thumb }
                    .paint()
                    .body()
                    .clone()
            }
        };
        let label = format!("{theme_name} {family} {part} {row}");
        assert_eq!(
            serde_json::to_value(body.color_role()).unwrap(),
            row["colorRole"],
            "{label}"
        );
        let resolved = body.face().pigment().body();
        for (published, actual) in row["body"]
            .as_array()
            .unwrap()
            .iter()
            .zip(resolved.components())
        {
            assert!(
                (published.as_f64().unwrap() - actual).abs() <= 1e-12,
                "{label}"
            );
        }
        for (witness, field) in [
            ("highlight", "highlightDeltaE"),
            ("shading", "shadingDeltaE"),
        ] {
            let point = &table["parts"][part][witness]["point"];
            let painted = body
                .sample_paint(PhysicalVector {
                    x: point["x"].as_f64().unwrap(),
                    y: point["y"].as_f64().unwrap(),
                })
                .unwrap()
                .unwrap();
            let difference = oklab_difference(resolved, &painted);
            assert!(
                (difference - row[field].as_f64().unwrap()).abs() <= 1e-9,
                "{label}: {field} {difference}"
            );
        }
    }
}
