use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{
    ActivationEvent, ActivationState, ColorRole, PhysicalBounds, PhysicalVector, SurfaceSize,
    TypographyRole,
};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, HitRegionError, HitRegionInput, HitRegionIr, SrgbFallback,
    ToggleAccessibilityError, ToggleLayoutInput, ToggleLayoutIr, TogglePart, TogglePartPaintIr,
    ToggleSnapshotError, ToggleSnapshotInput, compile_theme_source, resolve_activation,
    resolve_command_label, resolve_hit_region, resolve_srgb_fallback, resolve_toggle_layout,
    resolve_toggle_part_paint_source, resolve_toggle_snapshot,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Matrix {
    schema_version: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    activation: ActivationState,
    hovered: bool,
    checked: bool,
    layout_checked: bool,
    track_states: Vec<String>,
    thumb_states: Vec<String>,
    coherent: bool,
}
fn bounds(x: f64, y: f64, width: f64, height: f64) -> PhysicalBounds {
    PhysicalBounds {
        x,
        y,
        width,
        height,
    }
}
fn size(width: f64, height: f64) -> SurfaceSize {
    SurfaceSize { width, height }
}
fn available() -> PhysicalBounds {
    bounds(-300.0, -100.0, 600.0, 300.0)
}
fn reserve(env: &EnvironmentSnapshot, bounds: PhysicalBounds) -> HitRegionIr {
    resolve_hit_region(HitRegionInput {
        environment: env,
        visual_bounds: bounds,
        available_bounds: available(),
        component_minimum: size(48.0, 48.0),
        occupied_regions: &[],
    })
    .unwrap()
}
fn request(family: &str, direction: LayoutDirection) -> Value {
    let mut r: Value = serde_json::from_str(include_str!(
        "../../../../conformance/ir/toggle-part-paint-request.json"
    ))
    .unwrap();
    r["surface"]["body"]["size"] = json!({"width":64,"height":40});
    r["surface"]["body"]["surface"]["form"]["shape"] = json!("rounded");
    r["surface"]["body"]["appearance"]["shapeAssignments"] =
        serde_json::from_str::<Value>(include_str!("../../../../definitions/tier0-shapes.json"))
            .unwrap();
    let mut theme: Value = serde_json::from_str(
        r["surface"]["body"]["theme"]["themeSource"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    theme["materialAssignments"]["control"]["interactive"] = json!(family);
    theme["colorAssignments"]["roles"]["surface.high"] = json!("palette.base");
    theme["opaqueColorAssignments"]["roles"]["surface.high"] = json!("palette.base");
    r["surface"]["body"]["theme"]["themeSource"] = json!(theme.to_string());
    r["surface"]["body"]["theme"]["environment"]["layoutDirection"] =
        serde_json::to_value(direction).unwrap();
    r
}
struct Fixture {
    request: Value,
    environment: EnvironmentSnapshot,
    label: CommandLabelIr,
    label_foreground: SrgbFallback,
    label_background: SrgbFallback,
    track: TogglePartPaintIr,
    thumb: TogglePartPaintIr,
    layout: ToggleLayoutIr,
    target: HitRegionIr,
    activation: ActivationState,
    checked: bool,
    hovered: bool,
}
impl Fixture {
    fn new(family: &str, direction: LayoutDirection, case: &Case) -> Self {
        let mut r = request(family, direction);
        r["surface"]["body"]["surface"]["states"]["states"] = json!(case.track_states);
        let track = resolve_toggle_part_paint_source(&r.to_string()).unwrap();
        let mut t = r.clone();
        t["part"] = json!("thumb");
        t["surface"]["body"]["size"] = json!({"width":16,"height":16});
        t["surface"]["body"]["surface"]["states"]["states"] = json!(case.thumb_states);
        t["surface"]["body"]["surface"]["colorRole"] = json!("surface.high");
        t["checkedColorRole"] = json!("surface.high");
        t["surface"]["body"]["adjacentColor"] =
            serde_json::to_value(track.paint().body().pigment().body()).unwrap();
        t["surface"]["body"]["postTreatmentBackdrop"] =
            t["surface"]["body"]["adjacentColor"].clone();
        let thumb = resolve_toggle_part_paint_source(&t.to_string()).unwrap();
        let environment: EnvironmentSnapshot =
            serde_json::from_value(r["surface"]["body"]["theme"]["environment"].clone()).unwrap();
        let theme = compile_theme_source(
            r["surface"]["body"]["theme"]["themeSource"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let resolved = theme.resolve(&environment).unwrap();
        let label = resolve_command_label(
            CommandLabelInput {
                text: "Automatic updates",
                typography: &resolved.typography()[&TypographyRole::Label],
                minimum_size: size(96.0, 40.0),
                maximum_size: size(96.0, 40.0),
                padding: SafeArea {
                    start: 8.0,
                    end: 8.0,
                    top: 12.0,
                    bottom: 12.0,
                },
                direction,
            },
            |_| Ok::<_, Infallible>(size(80.0, 16.0)),
        )
        .unwrap();
        let layout = resolve_toggle_layout(ToggleLayoutInput {
            track_size: size(64.0, 40.0),
            thumb_size: size(16.0, 16.0),
            insets: &SafeArea {
                start: 12.0,
                end: 12.0,
                top: 12.0,
                bottom: 12.0,
            },
            layout_direction: direction,
            checked: case.layout_checked,
        })
        .unwrap();
        let target = reserve(
            &environment,
            match direction {
                LayoutDirection::Ltr => bounds(-20.0, -20.0, 220.0, 80.0),
                LayoutDirection::Rtl => bounds(-140.0, -20.0, 240.0, 80.0),
            },
        );
        Self {
            request: r,
            environment,
            label,
            label_foreground: resolved.color_fallbacks()[&ColorRole::ContentPrimary].clone(),
            label_background: resolved.color_fallbacks()[&ColorRole::SurfaceChrome].clone(),
            track,
            thumb,
            layout,
            target,
            activation: case.activation.clone(),
            checked: case.checked,
            hovered: case.hovered,
        }
    }
    fn input(&self) -> ToggleSnapshotInput<'_, '_> {
        ToggleSnapshotInput {
            label: &self.label,
            label_foreground: &self.label_foreground,
            label_background: &self.label_background,
            minimum_label_contrast: 4.5,
            label_origin: PhysicalVector {
                x: if self.environment.layout_direction() == LayoutDirection::Ltr {
                    80.0
                } else {
                    -112.0
                },
                y: 0.0,
            },
            layout: &self.layout,
            track: &self.track,
            thumb: &self.thumb,
            hit_region: self.target,
            activation: &self.activation,
            hovered: self.hovered,
            checked: self.checked,
            description: Some("Install updates automatically"),
            focusable: true,
            minimum_thumb_contrast: 3.0,
            environment: &self.environment,
            available_bounds: available(),
            component_minimum: size(48.0, 48.0),
            occupied_regions: &[],
        }
    }
}
fn cases() -> Vec<Case> {
    let matrix: Matrix = serde_json::from_str(include_str!(
        "../../../../conformance/interaction/toggle-snapshot-cases.json"
    ))
    .unwrap();
    assert_eq!(matrix.schema_version, "0.1.0");
    matrix.cases
}
fn fixture() -> Fixture {
    Fixture::new("cast", LayoutDirection::Ltr, &cases()[0])
}

#[test]
fn public_current_and_stale_signals_compose_across_families_and_directions() {
    let cases = cases();
    let mut count = 0;
    for family in ["cast", "frost", "elastomer"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for case in &cases {
                let f = Fixture::new(family, direction, case);
                let result = resolve_toggle_snapshot(f.input());
                assert_eq!(
                    result.is_ok(),
                    case.coherent,
                    "{family} {direction:?} {}: {result:?}",
                    case.name
                );
                if case.coherent {
                    let snapshot = result.unwrap();
                    assert!(std::ptr::eq(snapshot.track(), &f.track));
                    assert!(std::ptr::eq(snapshot.thumb(), &f.thumb));
                    assert!(std::ptr::eq(snapshot.layout(), &f.layout));
                    assert!(std::ptr::eq(snapshot.label(), &f.label));
                    assert_eq!(snapshot.hit_region().bounds(), f.target.bounds());
                    assert_eq!(snapshot.accessibility().name(), "Automatic updates");
                    assert_eq!(snapshot.accessibility().state().checked(), case.checked);
                    assert_eq!(
                        snapshot.accessibility().state().focused(),
                        case.activation.focused()
                    );
                    assert_eq!(
                        snapshot.accessibility().state().enabled(),
                        case.activation.enabled()
                    );
                    assert_eq!(
                        snapshot.accessibility().actions()[0].available(),
                        case.activation.enabled()
                    );
                    assert_eq!(
                        snapshot.thumb_contrast_ratio(),
                        f.thumb.paint().body().edge().contrast_ratio()
                    );
                    let footprint = f
                        .thumb
                        .paint()
                        .body()
                        .geometry()
                        .silhouette()
                        .bounds()
                        .unwrap();
                    for endpoint in [f.layout.off_thumb_bounds(), f.layout.on_thumb_bounds()] {
                        for tx in [0.0, 0.5, 1.0] {
                            for ty in [0.0, 0.5, 1.0] {
                                let point = PhysicalVector {
                                    x: endpoint.x + footprint.x + tx * footprint.width,
                                    y: endpoint.y + footprint.y + ty * footprint.height,
                                };
                                assert_eq!(
                                    f.track.paint().body().sample_paint(point).unwrap().as_ref(),
                                    Some(f.track.paint().body().pigment().body())
                                );
                            }
                        }
                    }
                    assert_eq!(
                        snapshot.label_origin().x,
                        if direction == LayoutDirection::Ltr {
                            80.0
                        } else {
                            -112.0
                        }
                    );
                } else {
                    assert!(matches!(
                        result.unwrap_err(),
                        ToggleSnapshotError::StatesMismatch { .. }
                            | ToggleSnapshotError::SelectionMismatch
                    ));
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 210);
}

#[test]
fn allocation_fit_cannot_certify_uniform_backdrop_or_wrong_part_size() {
    let mut f = fixture();
    f.layout = resolve_toggle_layout(ToggleLayoutInput {
        track_size: size(64.0, 40.0),
        thumb_size: size(16.0, 16.0),
        insets: &SafeArea {
            start: 1.0,
            end: 12.0,
            top: 12.0,
            bottom: 12.0,
        },
        layout_direction: LayoutDirection::Ltr,
        checked: false,
    })
    .unwrap();
    assert!(matches!(
        resolve_toggle_snapshot(f.input()),
        Err(ToggleSnapshotError::ThumbOutsideContent("off"))
    ));
    f.layout = resolve_toggle_layout(ToggleLayoutInput {
        track_size: size(64.0, 40.0),
        thumb_size: size(16.0, 16.0),
        insets: &SafeArea {
            start: 12.0,
            end: 1.0,
            top: 12.0,
            bottom: 12.0,
        },
        layout_direction: LayoutDirection::Ltr,
        checked: false,
    })
    .unwrap();
    assert!(matches!(
        resolve_toggle_snapshot(f.input()),
        Err(ToggleSnapshotError::ThumbOutsideContent("on"))
    ));
    f.layout = resolve_toggle_layout(ToggleLayoutInput {
        track_size: size(65.0, 40.0),
        thumb_size: size(16.0, 16.0),
        insets: &SafeArea {
            start: 12.0,
            end: 12.0,
            top: 12.0,
            bottom: 12.0,
        },
        layout_direction: LayoutDirection::Ltr,
        checked: false,
    })
    .unwrap();
    assert!(matches!(
        resolve_toggle_snapshot(f.input()),
        Err(ToggleSnapshotError::SizeMismatch(TogglePart::Track))
    ));
    let f = fixture();
    let mut i = f.input();
    i.thumb = &f.track;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::PartMismatch)
    ));
}

#[test]
fn actual_track_contrast_rejects_stale_adjacency_and_insufficient_edges() {
    let mut f = fixture();
    let mut t = f.request.clone();
    t["part"] = json!("thumb");
    t["surface"]["body"]["size"] = json!({"width":16,"height":16});
    f.thumb = resolve_toggle_part_paint_source(&t.to_string()).unwrap();
    assert!(matches!(
        resolve_toggle_snapshot(f.input()),
        Err(ToggleSnapshotError::ThumbAdjacencyMismatch { .. })
    ));
    let f = fixture();
    let mut i = f.input();
    i.minimum_thumb_contrast = 21.0;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::ThumbContrast { .. })
    ));
    for value in [f64::NAN, f64::INFINITY, 0.0, 22.0] {
        let mut i = f.input();
        i.minimum_thumb_contrast = value;
        assert!(matches!(
            resolve_toggle_snapshot(i),
            Err(ToggleSnapshotError::InvalidContrast)
        ));
    }
}

#[test]
fn labels_targets_direction_and_semantics_publish_only_when_coherent() {
    let f = fixture();
    let mut i = f.input();
    i.label_origin.x = 20.0;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::LabelOverlap)
    ));
    let mut i = f.input();
    i.label_origin.x = 150.0;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::TargetCoverage("label layout box"))
    ));
    let mut i = f.input();
    i.label_origin.x = f64::MAX;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::NumericRange)
    ));
    let small = reserve(&f.environment, bounds(1.0, -20.0, 199.0, 80.0));
    let mut i = f.input();
    i.hit_region = small;
    let result = resolve_toggle_snapshot(i);
    assert!(
        matches!(
            result,
            Err(ToggleSnapshotError::TargetCoverage("track paint"))
        ),
        "{result:?}"
    );
    let mut i = f.input();
    i.component_minimum = size(300.0, 48.0);
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::TargetResize)
    ));
    let occupied = [f.target.bounds()];
    let mut i = f.input();
    i.occupied_regions = &occupied;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::Target(HitRegionError::Occupied(0)))
    ));
    let mut i = f.input();
    i.available_bounds = bounds(0.0, -20.0, 200.0, 80.0);
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::Target(HitRegionError::Clipped))
    ));
    let mut i = f.input();
    i.description = Some(" ");
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::Accessibility(
            ToggleAccessibilityError::BlankDescription
        ))
    ));
    let mut i = f.input();
    i.focusable = false;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::Accessibility(
            ToggleAccessibilityError::EnabledNotFocusable
        ))
    ));
    let mut env = f.environment.clone();
    let mut value = serde_json::to_value(&env).unwrap();
    value["layoutDirection"] = json!("rtl");
    env = serde_json::from_value(value).unwrap();
    let mut i = f.input();
    i.environment = &env;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::DirectionMismatch)
    ));
}

#[test]
fn reserved_navigation_and_label_box_remain_stable_through_feedback() {
    let cases = cases();
    let focused = cases.iter().find(|c| c.name == "off focused rest").unwrap();
    let f = Fixture::new("elastomer", LayoutDirection::Ltr, focused);
    let mut i = f.input();
    i.hit_region = reserve(&f.environment, bounds(-1.0, -2.0, 200.0, 48.0));
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::TargetCoverage("track navigation"))
    ));
    let reserved = fixture().target;
    for case in cases.iter().filter(|c| c.coherent) {
        let f = Fixture::new("elastomer", LayoutDirection::Ltr, case);
        let mut i = f.input();
        i.hit_region = reserved;
        assert_eq!(
            resolve_toggle_snapshot(i).unwrap().hit_region().bounds(),
            reserved.bounds()
        );
    }
}

#[test]
fn snapshot_does_not_retain_live_activation_or_layout_context() {
    let f = fixture();
    let mut activation = f.activation.clone();
    let mut i = f.input();
    i.activation = &activation;
    let snapshot = resolve_toggle_snapshot(i).unwrap();
    activation = resolve_activation(
        &activation,
        &ActivationEvent::Availability { enabled: false },
    )
    .unwrap()
    .state()
    .clone();
    assert!(!activation.enabled());
    assert!(snapshot.accessibility().state().enabled());
    let mut i = f.input();
    i.activation = &activation;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::StatesMismatch { .. })
    ));
}

#[test]
fn contrast_metadata_tolerance_never_relaxes_the_minimum() {
    for (delta, coherent) in [(1e-14, true), (1e-10, false)] {
        let mut f = fixture();
        let mut t = f.request.clone();
        t["part"] = json!("thumb");
        t["surface"]["body"]["size"] = json!({"width":16,"height":16});
        t["surface"]["body"]["surface"]["colorRole"] = json!("surface.high");
        let color = f.track.paint().body().pigment().body();
        let adjacent = color.components().map(|c| c + delta);
        t["surface"]["body"]["adjacentColor"] =
            json!({"colorSpace":"srgb","components":adjacent,"alpha":1});
        f.thumb = resolve_toggle_part_paint_source(&t.to_string()).unwrap();
        let result = resolve_toggle_snapshot(f.input());
        assert_eq!(result.is_ok(), coherent, "delta={delta}: {result:?}");
        if !coherent {
            assert!(matches!(
                result.unwrap_err(),
                ToggleSnapshotError::ThumbAdjacencyMismatch { .. }
            ));
        }
    }
    let f = fixture();
    let mut i = f.input();
    i.minimum_thumb_contrast = f.thumb.paint().body().edge().contrast_ratio() + 1e-13;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::ThumbContrast { .. })
    ));
}

#[test]
fn external_label_contrast_uses_existing_independent_color_vectors() {
    let f = fixture();
    let vectors: Vec<Value> = serde_json::from_str(include_str!(
        "../../../../conformance/color/contrast-vectors.json"
    ))
    .unwrap();
    for vector in vectors {
        let foreground = resolve_srgb_fallback(&vector["foreground"]).unwrap();
        let background = resolve_srgb_fallback(&vector["background"]).unwrap();
        let mut i = f.input();
        i.label_foreground = &foreground;
        i.label_background = &background;
        i.minimum_label_contrast = 1.0;
        let result = resolve_toggle_snapshot(i);
        if let Some(expected) = vector.get("expected") {
            let snapshot = result.unwrap();
            assert!((snapshot.label_contrast_ratio() - expected.as_f64().unwrap()).abs() <= 1e-12);
            assert_eq!(snapshot.label_foreground(), &foreground);
            assert_eq!(snapshot.label_background(), &background);
        } else {
            let error = result.unwrap_err();
            let ToggleSnapshotError::LabelColor(cause) = &error else {
                panic!("{error}")
            };
            assert_eq!(format!("{cause:?}"), vector["error"].as_str().unwrap());
            assert!(std::error::Error::source(&error).is_some());
        }
    }
}

#[test]
fn track_readability_cannot_certify_an_external_label() {
    let f = fixture();
    assert!(f.track.paint().body().content_contrast_ratio() >= 4.5);
    let black = resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[0,0,0]})).unwrap();
    let white = resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[1,1,1]})).unwrap();
    let mut i = f.input();
    i.label_foreground = &black;
    i.label_background = &black;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::LabelContrast {
            actual: 1.0,
            minimum: 4.5
        })
    ));
    let mut i = f.input();
    i.label_foreground = &white;
    i.label_background = &black;
    let snapshot = resolve_toggle_snapshot(i).unwrap();
    assert_eq!(snapshot.label_contrast_ratio(), 21.0);
    assert_eq!(snapshot.label_foreground(), &white);
    assert_eq!(snapshot.track().paint().body().foreground(), &black);
}

#[test]
fn label_minimum_is_explicit_finite_and_strict() {
    let f = fixture();
    for minimum in [f64::NAN, f64::INFINITY, 0.0, 21.0000000000001] {
        let mut i = f.input();
        i.minimum_label_contrast = minimum;
        assert!(matches!(
            resolve_toggle_snapshot(i),
            Err(ToggleSnapshotError::InvalidLabelContrast)
        ));
    }
    let gray =
        resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[0.5,0.5,0.5]})).unwrap();
    let mut i = f.input();
    i.label_foreground = &gray;
    i.label_background = &gray;
    i.minimum_label_contrast = 1.0;
    assert_eq!(
        resolve_toggle_snapshot(i).unwrap().label_contrast_ratio(),
        1.0
    );
    let mut i = f.input();
    i.label_foreground = &gray;
    i.label_background = &gray;
    i.minimum_label_contrast = 1.0 + 1e-13;
    assert!(matches!(
        resolve_toggle_snapshot(i),
        Err(ToggleSnapshotError::LabelContrast { .. })
    ));
}

#[test]
fn published_label_colors_do_not_borrow_live_palette_inputs() {
    let f = fixture();
    let mut foreground = f.label_foreground.clone();
    let mut background = f.label_background.clone();
    let mut i = f.input();
    i.label_foreground = &foreground;
    i.label_background = &background;
    let snapshot = resolve_toggle_snapshot(i).unwrap();
    std::mem::swap(&mut foreground, &mut background);
    assert_eq!(snapshot.label_foreground(), &f.label_foreground);
    assert_eq!(snapshot.label_background(), &f.label_background);
    assert_ne!(snapshot.label_foreground(), &foreground);
    assert_ne!(snapshot.label_background(), &background);
}
