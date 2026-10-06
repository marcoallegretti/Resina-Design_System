use resina_environment::EnvironmentSnapshot;
use resina_model::{
    ColorRole, CommandAnatomy, CommandAppearance, CommandEmphasis, OpaqueSurfaceAppearance,
    StateSet, SurfaceSize, ToggleAnatomy,
};
use resina_resolver::{
    CommandPaintInput, CompiledTheme, OpaqueSurfaceInput, SrgbFallback, SurfacePaintInput,
    TogglePart, TogglePartPaintInput, compile_theme_source_with_sources, opaque_contrast_ratio,
    resolve_command_paint, resolve_toggle_part_paint,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const FAMILIES: [&str; 3] = ["cast", "frost", "elastomer"];
const PHASES: [&str; 4] = ["rest", "hover", "pressed", "disabled"];

struct Theme {
    name: &'static str,
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

#[test]
fn command_anatomy_resolves_every_state_in_the_authored_themes() {
    let anatomy: CommandAnatomy = serde_json::from_str(include_str!(
        "../../../../definitions/components/command.json"
    ))
    .unwrap();
    let environment = environment();
    let surface = surface_appearance();
    let mut resolved = 0;
    for theme in themes(&environment) {
        let responses = appearance(theme.name);
        for emphasis in [CommandEmphasis::Standard, CommandEmphasis::Primary] {
            let body = anatomy.variant(emphasis).body();
            for phase in PHASES {
                for focused in [false, true] {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(focused.then_some("focused"))
                        .collect();
                    let intent = body.intent(states(&names));
                    let result = resolve_command_paint(
                        &theme.theme,
                        &environment,
                        CommandPaintInput {
                            surface: SurfacePaintInput {
                                body: OpaqueSurfaceInput {
                                    surface: &intent,
                                    size: SurfaceSize {
                                        width: 96.0,
                                        height: 48.0,
                                    },
                                    appearance: &surface,
                                    foreground_role: body.content_role(),
                                    post_treatment_backdrop: Some(&theme.canvas),
                                    adjacent_color: &theme.canvas,
                                    minimum_content_contrast: 4.5,
                                    minimum_edge_contrast: 3.0,
                                },
                                surrounding_color: Some(&theme.canvas),
                            },
                            command_appearance: &responses,
                        },
                    );
                    if let Err(error) = result {
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
    let anatomy: ToggleAnatomy = serde_json::from_str(include_str!(
        "../../../../definitions/components/toggle.json"
    ))
    .unwrap();
    let environment = environment();
    let surface = surface_appearance();
    let mut resolved = 0;
    for theme in themes(&environment) {
        assert!(
            opaque_contrast_ratio(&theme.colors[&anatomy.label_color()], &theme.canvas).unwrap()
                >= 4.5
        );
        let responses = appearance(theme.name);
        for checked in [false, true] {
            for phase in PHASES {
                for focused in [false, true] {
                    let names: Vec<&str> = [phase]
                        .into_iter()
                        .chain(focused.then_some("focused"))
                        .chain(checked.then_some("checked"))
                        .collect();
                    let part = |part: TogglePart, adjacent: &SrgbFallback| {
                        let (anatomy_part, size) = match part {
                            TogglePart::Track => (anatomy.track(), 52.0),
                            TogglePart::Thumb => (anatomy.thumb(), 20.0),
                        };
                        let intent = anatomy_part.intent(states(&names));
                        resolve_toggle_part_paint(
                            &theme.theme,
                            &environment,
                            TogglePartPaintInput {
                                part,
                                surface: SurfacePaintInput {
                                    body: OpaqueSurfaceInput {
                                        surface: &intent,
                                        size: SurfaceSize {
                                            width: size,
                                            height: if part == TogglePart::Track {
                                                32.0
                                            } else {
                                                20.0
                                            },
                                        },
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
                        .unwrap_or_else(|error| {
                            panic!("{} {part:?} {names:?}: {error}", theme.name)
                        })
                    };
                    let track = part(TogglePart::Track, &theme.canvas);
                    // As in the Toggle snapshot, the thumb edge must clear 3:1 against
                    // the actual track body.
                    part(TogglePart::Thumb, track.paint().body().pigment().body());
                    resolved += 1;
                }
            }
        }
    }
    assert_eq!(resolved, 2 * 28 * 2 * 4 * 2);
}
