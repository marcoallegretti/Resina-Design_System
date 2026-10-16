use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAnatomy, CommandAppearance, CommandBody, CommandEmphasis,
    OpaqueSurfaceAppearance, StateSet, SurfaceSize, ToggleAnatomy,
};
use resina_resolver::{
    CommandPaintError, CommandPaintInput, CommandPaintIr, CompiledTheme, OpaqueSurfaceInput,
    SrgbFallback, SurfacePaintInput, TogglePart, TogglePartPaintInput, TogglePartPaintIr,
    compile_theme_source_with_sources, opaque_contrast_ratio, resolve_command_paint,
    resolve_toggle_part_paint,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const FAMILIES: [&str; 3] = ["cast", "frost", "elastomer"];
const PHASES: [&str; 4] = ["rest", "hover", "pressed", "disabled"];

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

fn command(
    theme: &Theme,
    environment: &EnvironmentSnapshot,
    body: &CommandBody,
    names: &[&str],
) -> Result<CommandPaintIr, CommandPaintError> {
    let intent = body.intent(states(names));
    resolve_command_paint(
        &theme.theme,
        environment,
        CommandPaintInput {
            surface: SurfacePaintInput {
                body: OpaqueSurfaceInput {
                    surface: &intent,
                    size: SurfaceSize {
                        width: 96.0,
                        height: 48.0,
                    },
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
) -> [TogglePartPaintIr; 2] {
    let surface = surface_appearance();
    let responses = appearance(theme.name);
    let part = |part: TogglePart, adjacent: &SrgbFallback| {
        let (anatomy_part, width, height) = match part {
            TogglePart::Track => (anatomy.track(), 52.0, 32.0),
            TogglePart::Thumb => (anatomy.thumb(), 20.0, 20.0),
        };
        let intent = anatomy_part.intent(states(names));
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
            },
        )
        .unwrap_or_else(|error| panic!("{} {part:?} {names:?}: {error}", theme.name))
    };
    let track = part(TogglePart::Track, &theme.canvas);
    let thumb = part(TogglePart::Thumb, track.paint().body().pigment().body());
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
    for theme in themes(&environment) {
        for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
            let body = anatomy.variant(emphasis).body();
            for phase in PHASES {
                for focused in [false, true] {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(focused.then_some("focused"))
                        .collect();
                    if let Err(error) = command(&theme, &environment, body, &names) {
                        panic!("{} {emphasis:?} {names:?}: {error}", theme.name);
                    }
                    resolved += 1;
                }
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 2 * 4 * 2);
}

#[test]
fn toggle_anatomy_resolves_every_state_with_an_identifiable_thumb_edge() {
    let anatomy = toggle_anatomy();
    let environment = environment();
    let mut resolved = 0;
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
                    toggle(&theme, &environment, &anatomy, &names);
                    resolved += 1;
                }
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 2 * 4 * 2);
}

/// Body colour and final extrusion depth, the two channels through which a state may differ.
fn channels(body: &resina_resolver::OpaqueSurfaceIr) -> (SrgbFallback, f64) {
    let offset = body.lighting().side_offset();
    (body.pigment().body().clone(), offset.x.hypot(offset.y))
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
                    command(&theme, &environment, body, &[phase]).map(|paint| {
                        let body = paint.paint().body();
                        (body.material_family(), channels(body))
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
                toggle(&theme, &environment, &toggles, &names)
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
                        .map(|phases| channels(phases[index].paint().body()))
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
