use resina_environment::EnvironmentSnapshot;
use resina_model::{SurfaceSize, TypographyRole};
use resina_resolver::{
    CommandLabelError, CommandLabelInput, CommandLayoutIr, LabelMeasureInput,
    compile_theme_source_with_sources, resolve_command_label, resolve_command_layout_source,
    resolve_command_paint_source,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const CASES: &str = include_str!("../../../../conformance/layout/command-layout-cases.json");
const EXTENT_TOKENS: &str = include_str!("../../../../tokens/components/command.json");
const PROFILE: &str = include_str!("../../../../definitions/layout/command.json");
const FOUNDATION: &str = include_str!("../../../../tokens/foundation.json");
const LIGHT: &str = include_str!("../../../../tokens/themes/light.json");
const DARK: &str = include_str!("../../../../tokens/themes/dark.json");
const PAINT_REQUEST: &str = include_str!("../../../../conformance/ir/command-paint-request.json");
const CONTROL_RELIEF: &str = include_str!("../../../../definitions/control-relief.json");
const SHAPES: &str = include_str!("../../../../definitions/tier0-shapes.json");
const APPEARANCE: &str = include_str!("../../../../definitions/tier0-surface-appearance.json");

fn cases() -> Vec<Value> {
    serde_json::from_str(CASES).unwrap()
}

fn case(name: &str) -> Value {
    cases()
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap()
}

fn same(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => actual.as_f64() == expected.as_f64(),
        (Value::Object(actual), Value::Object(expected)) => {
            actual.len() == expected.len()
                && expected
                    .iter()
                    .all(|(key, value)| actual.get(key).is_some_and(|found| same(found, value)))
        }
        _ => actual == expected,
    }
}

#[test]
fn public_cases_resolve_to_their_expected_policy_or_diagnostic() {
    let mut resolved = 0;
    for case in cases() {
        let result = resolve_command_layout_source(&case["request"].to_string());
        match case.get("expected") {
            Some(expected) => {
                let actual = serde_json::to_value(result.unwrap()).unwrap();
                assert!(same(&actual, expected), "{}: {actual}", case["name"]);
                resolved += 1;
            }
            None => {
                let error = result.unwrap_err().to_string();
                if let Some(expected) = case.get("errorContains") {
                    assert!(
                        error.contains(expected.as_str().unwrap()),
                        "{}: {error}",
                        case["name"]
                    );
                }
            }
        }
    }
    assert_eq!(resolved, 32 + 11);
}

#[test]
fn authored_themes_supply_the_binding_used_by_the_public_cases() {
    let sources = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    for theme in [LIGHT, DARK] {
        let compiled = compile_theme_source_with_sources(theme, &sources).unwrap();
        for case in cases()
            .iter()
            .filter(|case| case.get("expected").is_some())
            .take(32)
        {
            assert!(
                same(
                    &serde_json::to_value(compiled.space()).unwrap(),
                    &case["request"]["spatialBinding"]
                ),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn authored_extent_tokens_are_the_ones_the_public_cases_use() {
    let authored: Value = serde_json::from_str(EXTENT_TOKENS).unwrap();
    let profile: Value = serde_json::from_str(PROFILE).unwrap();
    let tokens = resina_tokens::resolve_token_source(EXTENT_TOKENS).unwrap();
    let mut referenced = BTreeSet::new();
    for extent in profile["extents"].as_object().unwrap().values() {
        for size in ["minimumSize", "maximumSize"] {
            for axis in ["width", "height"] {
                let path = extent[size][axis].as_str().unwrap();
                let token = &tokens[path];
                assert_eq!(token.token_type, "dimension", "{path}");
                assert_eq!(token.value["unit"], "px", "{path}");
                referenced.insert(path.to_owned());
            }
        }
    }
    assert_eq!(referenced, tokens.keys().cloned().collect());
    for case in cases()
        .iter()
        .filter(|case| case.get("expected").is_some())
        .take(32)
    {
        assert_eq!(case["request"]["tokens"], authored, "{}", case["name"]);
        assert_eq!(case["request"]["profile"], profile, "{}", case["name"]);
    }
}

#[test]
fn target_minimum_follows_input_alone_and_may_exceed_the_visual_body() {
    let mut targets = BTreeMap::new();
    let mut touch = 0;
    let mut larger_than_body = false;
    for case in cases().iter().filter(|case| case.get("expected").is_some()) {
        let request = &case["request"];
        let expected = &case["expected"];
        let inputs = request["environment"]["inputCapabilities"].to_string();
        let coarse = ["coarsePointer", "directTouch"]
            .iter()
            .any(|input| inputs.contains(input));
        touch += usize::from(inputs.contains("directTouch") && !inputs.contains("coarsePointer"));
        let floor = if coarse { 48.0 } else { 24.0 };
        let target = &expected["targetMinimum"];
        assert_eq!(target["width"], floor, "{}", case["name"]);
        assert_eq!(target["height"], floor, "{}", case["name"]);
        let densities = targets.entry(inputs).or_insert_with(BTreeSet::new);
        densities.insert(expected["densityPreference"].to_string());
        larger_than_body |= floor > expected["minimumSize"]["height"].as_f64().unwrap();
    }
    assert_eq!(touch, 2);
    assert!(larger_than_body);
    assert!(
        targets
            .values()
            .filter(|densities| densities.len() == 4)
            .count()
            >= 2
    );
}

fn policy(name: &str) -> (CommandLayoutIr, EnvironmentSnapshot) {
    let request = case(name)["request"].clone();
    let layout = resolve_command_layout_source(&request.to_string()).unwrap();
    (
        layout,
        serde_json::from_value(request["environment"].clone()).unwrap(),
    )
}

// Supplied advances: every glyph is 0.5 em wide and a line is one label line height.
fn supplied_measure(input: LabelMeasureInput<'_>) -> Result<SurfaceSize, std::convert::Infallible> {
    let advance = input.typography.font_size() * 0.5;
    let line = input.typography.font_size() * input.typography.line_height();
    let glyphs = input.text.chars().count() as f64;
    let per_line = input
        .maximum_width
        .map_or(glyphs, |width| (width / advance).floor().max(1.0));
    let lines = (glyphs / per_line).ceil();
    Ok(SurfaceSize {
        width: per_line.min(glyphs) * advance,
        height: lines * line,
    })
}

fn label(
    text: &str,
    layout: &CommandLayoutIr,
    environment: &EnvironmentSnapshot,
) -> Result<resina_resolver::CommandLabelIr, CommandLabelError<std::convert::Infallible>> {
    let sources = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    let resolution = compile_theme_source_with_sources(LIGHT, &sources)
        .unwrap()
        .resolve(environment)
        .unwrap();
    let typography = resolution.typography()[&TypographyRole::Label].clone();
    resolve_command_label(
        CommandLabelInput {
            text,
            typography: &typography,
            minimum_size: layout.minimum_size(),
            maximum_size: layout.maximum_size(),
            padding: layout.padding().clone(),
            direction: layout.layout_direction(),
        },
        supplied_measure,
    )
}

#[test]
fn resolved_policies_reflow_long_labels_or_reject_them_explicitly() {
    let (wide, environment) = policy("standard text 1 fine ltr");
    let (narrow, _) = policy("narrow container clamps the maximum width");
    let text = "Reconnect to the shared workspace";
    let wide_label = label(text, &wide, &environment).unwrap();
    let narrow_label = label(text, &narrow, &environment).unwrap();
    let wide_box = serde_json::to_value(&wide_label).unwrap();
    let narrow_box = serde_json::to_value(&narrow_label).unwrap();
    assert_eq!(narrow_box["size"]["width"], 120.0);
    assert!(
        narrow_box["size"]["height"].as_f64().unwrap()
            > wide_box["size"]["height"].as_f64().unwrap()
    );
    let (short, _) = policy("short container clamps the maximum height");
    assert!(matches!(
        label(text, &short, &environment),
        Err(CommandLabelError::Invalid(
            "complete label exceeds available command height"
        ))
    ));
}

#[test]
fn doubled_text_keeps_the_line_count_of_a_label_that_fits_at_full_scale() {
    let text = "Reconnect to the shared workspace and resume every paused transfer";
    for density in ["compact", "standard", "comfortable", "immersive"] {
        let (single, single_environment) = policy(&format!("{density} text 1 fine ltr"));
        let (double, double_environment) = policy(&format!("{density} text 2 fine ltr"));
        let single = serde_json::to_value(label(text, &single, &single_environment).unwrap())
            .unwrap()["labelBounds"]["height"]
            .as_f64()
            .unwrap();
        let double = serde_json::to_value(label(text, &double, &double_environment).unwrap())
            .unwrap()["labelBounds"]["height"]
            .as_f64()
            .unwrap();
        assert!(double <= single * 2.0, "{density}: {double} > 2 × {single}");
    }
}

#[test]
fn every_density_contains_its_label_in_the_lit_command_body() {
    let sources = BTreeMap::from([("foundation.json".to_owned(), FOUNDATION.to_owned())]);
    let foundation: Value = serde_json::from_str(FOUNDATION).unwrap();
    let appearance: Value = serde_json::from_str(APPEARANCE).unwrap();
    let mut contained = 0;
    for theme in [LIGHT, DARK] {
        let compiled = compile_theme_source_with_sources(theme, &sources).unwrap();
        for density in ["compact", "standard", "comfortable", "immersive"] {
            for scale in [1.0, 1.5, 2.0] {
                for lines in [1.0, 2.0] {
                    for direction in ["ltr", "rtl"] {
                        let mut request =
                            case(&format!("{density} text 1 fine ltr"))["request"].clone();
                        request["environment"]["textScale"] = json!(scale);
                        request["environment"]["layoutDirection"] = json!(direction);
                        let layout = resolve_command_layout_source(&request.to_string()).unwrap();
                        let environment: EnvironmentSnapshot =
                            serde_json::from_value(request["environment"].clone()).unwrap();
                        let typography = compiled.resolve(&environment).unwrap().typography()
                            [&TypographyRole::Label]
                            .clone();
                        let label = resolve_command_label(
                            CommandLabelInput {
                                text: "Reconnect",
                                typography: &typography,
                                minimum_size: layout.minimum_size(),
                                maximum_size: layout.maximum_size(),
                                padding: layout.padding().clone(),
                                direction: layout.layout_direction(),
                            },
                            |input| {
                                let size = input.typography.font_size();
                                Ok::<_, std::convert::Infallible>(SurfaceSize {
                                    width: size * 4.0,
                                    height: lines * (size * input.typography.line_height()),
                                })
                            },
                        )
                        .unwrap();
                        let mut paint: Value = serde_json::from_str(PAINT_REQUEST).unwrap();
                        let body = &mut paint["surface"]["body"];
                        body["size"] = serde_json::to_value(label.size()).unwrap();
                        body["theme"]["environment"]["layoutDirection"] = json!(direction);
                        body["theme"]["environment"]["textScale"] = json!(scale);
                        body["surface"]["form"]["shape"] = json!("rounded");
                        let mut source: Value =
                            serde_json::from_str(body["theme"]["themeSource"].as_str().unwrap())
                                .unwrap();
                        source["tokens"]["radius"] = foundation["radius"].clone();
                        body["theme"]["themeSource"] = json!(source.to_string());
                        body["appearance"]["shapeAssignments"] =
                            serde_json::from_str(SHAPES).unwrap();
                        body["appearance"]["bands"] = appearance["bands"].clone();
                        paint["relief"] = serde_json::from_str(CONTROL_RELIEF).unwrap();
                        let paint = resolve_command_paint_source(&paint.to_string()).unwrap();
                        let lit = paint.paint().body();
                        assert_eq!(
                            lit.face().surface().outline().corners()[0].x,
                            (label.size().height / 2.0).min(24.0)
                        );
                        assert_eq!(
                            label.validate_lit_content(lit),
                            Ok(()),
                            "{density} {scale} {lines} {direction}: {:?} in {:?}",
                            label.label_bounds(),
                            label.size()
                        );
                        contained += 1;
                    }
                }
            }
        }
    }
    assert_eq!(contained, 2 * 4 * 3 * 2 * 2);
}
