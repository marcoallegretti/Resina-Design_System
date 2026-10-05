use resina_color::{OpaqueSrgbRange, resolve_srgb_fallback};
use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{
    ColorRole, PhysicalBounds, PhysicalVector, SliderAppearance, SliderPart, SliderValue,
    SurfaceIntent, SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, HitRegionError, HitRegionInput, HitRegionIr,
    SliderAccessibilityError, SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition,
    SliderOrientation, SliderPartPaintInput, SliderPartPaintIr, SliderPointerEvent,
    SliderPointerInput, SliderPointerRouting, SliderPointerState, SliderPointerTarget,
    SliderPresentation, SliderSnapshotError, SliderSnapshotInput, SliderStatesInput, SliderValueIr,
    SliderValuePolicy, SrgbFallback, compile_theme_source, resolve_command_label,
    resolve_hit_region, resolve_slider_layout, resolve_slider_part_paint, resolve_slider_pointer,
    resolve_slider_snapshot, resolve_slider_states, resolve_slider_value,
};
use serde_json::{Value, json};
use std::convert::Infallible;

fn size(width: f64, height: f64) -> SurfaceSize {
    SurfaceSize { width, height }
}
fn bounds(x: f64, y: f64, width: f64, height: f64) -> PhysicalBounds {
    PhysicalBounds {
        x,
        y,
        width,
        height,
    }
}
fn color(value: f64) -> SrgbFallback {
    resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[value,value,value]})).unwrap()
}
fn reserve(environment: &EnvironmentSnapshot, region: PhysicalBounds) -> HitRegionIr {
    resolve_hit_region(HitRegionInput {
        environment,
        visual_bounds: region,
        available_bounds: bounds(-300.0, -300.0, 1000.0, 1000.0),
        component_minimum: size(48.0, 48.0),
        occupied_regions: &[],
    })
    .unwrap()
}
struct Fixture {
    environment: EnvironmentSnapshot,
    presentation: SliderPresentation,
    pointer: SliderPointerState,
    layout: SliderLayoutIr,
    track: SliderPartPaintIr,
    thumb: SliderPartPaintIr,
    label: CommandLabelIr,
    black: SrgbFallback,
    white: SrgbFallback,
    canvas: Vec<OpaqueSrgbRange>,
    target: HitRegionIr,
    enabled: bool,
    focused: bool,
    read_only: bool,
}
fn layout(
    value: &SliderValueIr,
    direction: LayoutDirection,
    orientation: SliderOrientation,
) -> SliderLayoutIr {
    resolve_slider_layout(SliderLayoutInput {
        value,
        layout_direction: direction,
        orientation,
        minimum_position: SliderMinimumPosition::Start,
        allocation_size: if orientation == SliderOrientation::Horizontal {
            size(160.0, 40.0)
        } else {
            size(40.0, 160.0)
        },
        thumb_size: size(24.0, 24.0),
        track_thickness: 6.0,
        insets: &SafeArea {
            start: 8.0,
            end: 8.0,
            top: 8.0,
            bottom: 8.0,
        },
    })
    .unwrap()
}
impl Fixture {
    fn new(
        family: &str,
        direction: LayoutDirection,
        orientation: SliderOrientation,
        enabled: bool,
        focused: bool,
        read_only: bool,
        preview: bool,
    ) -> Self {
        let request: Value = serde_json::from_str(include_str!(
            "../../../../conformance/ir/command-paint-request.json"
        ))
        .unwrap();
        let body = &request["surface"]["body"];
        let mut source: Value =
            serde_json::from_str(body["theme"]["themeSource"].as_str().unwrap()).unwrap();
        source["tokens"]["palette"]["dark"] =
            json!({"$value":{"colorSpace":"srgb","components":[0.02,0.02,0.02]}});
        source["tokens"]["palette"]["middle"] =
            json!({"$value":{"colorSpace":"srgb","components":[0.5,0.5,0.5]}});
        source["materialAssignments"]["control"]["interactive"] = json!(family);
        for assignments in ["colorAssignments", "opaqueColorAssignments"] {
            source[assignments]["roles"]["surface.base"] = json!("palette.dark");
            source[assignments]["roles"]["surface.high"] = json!("palette.base");
            source[assignments]["roles"]["outline.strong"] = json!("palette.middle");
            source[assignments]["roles"]["focus"] = json!("palette.middle");
        }
        let theme = compile_theme_source(&source.to_string()).unwrap();
        let mut env = body["theme"]["environment"].clone();
        env["layoutDirection"] = serde_json::to_value(direction).unwrap();
        let environment: EnvironmentSnapshot = serde_json::from_value(env).unwrap();
        let current =
            resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 0.0).unwrap()).unwrap();
        let policy = SliderValuePolicy::Continuous;
        let initial = layout(&current, direction, orientation);
        let target = reserve(&environment, bounds(-12.0, -12.0, 320.0, 196.0));
        let mut pointer = SliderPointerState::idle();
        if preview {
            let thumb = initial.thumb_bounds();
            let region = reserve(&environment, thumb);
            let point = PhysicalVector {
                x: thumb.x + 12.0,
                y: thumb.y + 12.0,
            };
            for event in [
                SliderPointerEvent::Down {
                    id: "p",
                    point,
                    target: SliderPointerTarget::Thumb,
                    region: &region,
                },
                SliderPointerEvent::RoutingAcquired { id: "p" },
                SliderPointerEvent::Move {
                    id: "p",
                    point: PhysicalVector {
                        x: point.x + 24.0,
                        y: point.y,
                    },
                },
            ] {
                pointer = resolve_slider_pointer(SliderPointerInput {
                    state: &pointer,
                    current: &current,
                    revision: "r0",
                    value_policy: &policy,
                    layout: &initial,
                    control_region: &target,
                    enabled: true,
                    read_only: false,
                    routing: SliderPointerRouting::Continuous,
                    event,
                })
                .unwrap()
                .state()
                .clone();
            }
        }
        let presentation =
            SliderPresentation::try_new(&current, "r0", &policy, pointer.edit()).unwrap();
        let layout = layout(presentation.visible(), direction, orientation);
        let states = resolve_slider_states(SliderStatesInput {
            presentation: &presentation,
            pointer: &pointer,
            enabled,
            read_only,
            focused,
            hovered: false,
            key_pressed: false,
        })
        .unwrap();
        let mut surface = body["surface"].clone();
        surface["states"] = serde_json::to_value(states).unwrap();
        let track_surface: SurfaceIntent = serde_json::from_value(surface.clone()).unwrap();
        surface["colorRole"] = json!("surface.high");
        let thumb_surface: SurfaceIntent = serde_json::from_value(surface).unwrap();
        let appearance = serde_json::from_value(body["appearance"].clone()).unwrap();
        let interaction: SliderAppearance = serde_json::from_str(include_str!(
            "../../../../conformance/appearance/slider-appearance.json"
        ))
        .unwrap();
        let white = color(1.0);
        let black = color(0.0);
        let canvas = vec![OpaqueSrgbRange::try_new(white.clone(), white.clone()).unwrap()];
        let resolve_part = |part,
                            surface: &SurfaceIntent,
                            allocation: PhysicalBounds,
                            ranges: &[OpaqueSrgbRange]| {
            resolve_slider_part_paint(
                &theme,
                &environment,
                SliderPartPaintInput {
                    part,
                    read_only,
                    surface,
                    size: size(allocation.width, allocation.height),
                    appearance: &appearance,
                    interaction_appearance: &interaction,
                    foreground_role: ColorRole::ContentPrimary,
                    post_treatment_backdrop: Some(&white),
                    adjacent_ranges: ranges,
                    surrounding_ranges: Some(ranges),
                    minimum_content_contrast: 1.0,
                    minimum_edge_contrast: 3.0,
                },
            )
            .unwrap()
        };
        let track = resolve_part(
            SliderPart::Track,
            &track_surface,
            layout.track_bounds(),
            &canvas,
        );
        let mut surrounding = track.paint().body().paint_color_ranges();
        surrounding.extend_from_slice(&canvas);
        let thumb = resolve_part(
            SliderPart::Thumb,
            &thumb_surface,
            layout.thumb_bounds(),
            &surrounding,
        );
        let resolved = theme.resolve(&environment).unwrap();
        let label = resolve_command_label(
            CommandLabelInput {
                text: "Volume",
                typography: &resolved.typography()[&TypographyRole::Label],
                minimum_size: size(100.0, 64.0),
                maximum_size: size(100.0, 64.0),
                padding: SafeArea {
                    start: 0.0,
                    end: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                },
                direction,
            },
            |input| {
                Ok::<_, Infallible>(size(
                    80.0,
                    input.typography.font_size() * input.typography.line_height(),
                ))
            },
        )
        .unwrap();
        Self {
            environment,
            presentation,
            pointer,
            layout,
            track,
            thumb,
            label,
            black,
            white,
            canvas,
            target,
            enabled,
            focused,
            read_only,
        }
    }
    fn input(&self) -> SliderSnapshotInput<'_, '_> {
        SliderSnapshotInput {
            interaction: SliderStatesInput {
                presentation: &self.presentation,
                pointer: &self.pointer,
                enabled: self.enabled,
                read_only: self.read_only,
                focused: self.focused,
                hovered: false,
                key_pressed: false,
            },
            layout: &self.layout,
            track: &self.track,
            thumb: &self.thumb,
            label: &self.label,
            label_origin: PhysicalVector { x: 190.0, y: 0.0 },
            label_foreground: &self.black,
            label_background: &self.white,
            minimum_label_contrast: 4.5,
            description: Some("Output level"),
            value_text: Some("Current level"),
            focusable: true,
            canvas_ranges: &self.canvas,
            minimum_track_contrast: 3.0,
            minimum_thumb_contrast: 3.0,
            hit_region: self.target,
            environment: &self.environment,
            available_bounds: bounds(-300.0, -300.0, 1000.0, 1000.0),
            component_minimum: size(48.0, 48.0),
            occupied_regions: &[],
        }
    }
}
#[test]
fn coherent_parts_compose_across_families_directions_axes_and_permissions() {
    for family in ["cast", "frost", "elastomer"] {
        for direction in [LayoutDirection::Ltr, LayoutDirection::Rtl] {
            for orientation in [SliderOrientation::Horizontal, SliderOrientation::Vertical] {
                for enabled in [false, true] {
                    for focused in [false, true] {
                        for read_only in [false, true] {
                            let f = Fixture::new(
                                family,
                                direction,
                                orientation,
                                enabled,
                                focused,
                                read_only,
                                false,
                            );
                            let mut input = f.input();
                            input.focusable = enabled || focused;
                            let snapshot = resolve_slider_snapshot(input).unwrap();
                            assert_eq!(
                                snapshot.layout().value(),
                                snapshot.presentation().visible()
                            );
                            assert_eq!(
                                snapshot.accessibility().value(),
                                snapshot.presentation().visible().value()
                            );
                            assert_eq!(snapshot.accessibility().orientation(), orientation);
                            assert_eq!(snapshot.accessibility().state().read_only(), read_only);
                            assert_eq!(snapshot.accessibility().state().focused(), focused);
                            assert_eq!(snapshot.accessibility().state().enabled(), enabled);
                            assert_eq!(snapshot.accessibility().focusable(), enabled || focused);
                            assert_eq!(
                                snapshot.accessibility().actions()[0].available(),
                                enabled && !read_only
                            );
                            assert_eq!(snapshot.hit_region().bounds(), f.target.bounds());
                            assert!(snapshot.thumb_contrast_ratio() >= 3.0);
                            assert!(snapshot.track_contrast_ratio() >= 3.0);
                            assert_eq!(snapshot.label_contrast_ratio(), 21.0);
                            assert_eq!(snapshot.label().text(), "Volume");
                            assert!(snapshot.reduced_motion());
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn pointer_preview_drives_layout_paint_and_semantics_without_committing() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        true,
    );
    assert_ne!(f.presentation.visible(), f.presentation.committed());
    let snapshot = resolve_slider_snapshot(f.input()).unwrap();
    assert_eq!(
        snapshot.accessibility().value(),
        f.presentation.visible().value()
    );
    assert_eq!(
        snapshot.thumb().phase(),
        resina_model::SliderPhase::Dragging
    );
    let stale = layout(
        f.presentation.committed(),
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
    );
    let mut input = f.input();
    input.layout = &stale;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("current visible value")
    );
    let mut input = f.input();
    input.hit_region = reserve(&f.environment, bounds(-16.0, -12.0, 324.0, 196.0));
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("pointer ownership")
    );
}
#[test]
fn stale_states_size_direction_and_part_slots_are_rejected() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let focused = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        false,
    );
    let mut input = f.input();
    input.thumb = &focused.thumb;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("part states")
    );
    let mut input = f.input();
    input.interaction.read_only = true;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("readOnly")
    );
    let mut input = f.input();
    input.track = &f.thumb;
    input.thumb = &f.track;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("declared slots")
    );
    let rtl = layout(
        f.presentation.visible(),
        LayoutDirection::Rtl,
        SliderOrientation::Horizontal,
    );
    let mut input = f.input();
    input.layout = &rtl;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("direction")
    );
    let rtl_paint = Fixture::new(
        "cast",
        LayoutDirection::Rtl,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut input = f.input();
    input.track = &rtl_paint.track;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("part paint direction")
    );
    let vertical = layout(
        f.presentation.visible(),
        LayoutDirection::Ltr,
        SliderOrientation::Vertical,
    );
    let mut input = f.input();
    input.layout = &vertical;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("front size")
    );
}
#[test]
fn actual_ranges_labels_and_reserved_target_are_verified() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        true,
        false,
        false,
    );
    let dark = color(0.0);
    let dark_canvas = [OpaqueSrgbRange::try_new(dark.clone(), dark).unwrap()];
    let mut input = f.input();
    input.canvas_ranges = &dark_canvas;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("track edge contrast")
    );
    let mut input = f.input();
    input.canvas_ranges = &[];
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("canvas ranges")
    );
    let dimmer = color(0.9);
    let dimmer_canvas = [OpaqueSrgbRange::try_new(dimmer.clone(), dimmer).unwrap()];
    let mut input = f.input();
    input.canvas_ranges = &dimmer_canvas;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::OverstatedContrast {
            stage: "track edge",
            ..
        })
    ));
    let mut input = f.input();
    input.label_foreground = &f.white;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label contrast")
    );
    let mut input = f.input();
    input.label_origin = PhysicalVector { x: 20.0, y: 8.0 };
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label must not overlap")
    );
    let mut input = f.input();
    input.hit_region = reserve(&f.environment, bounds(20.0, -12.0, 288.0, 196.0));
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("swept thumb")
    );
    let mut input = f.input();
    input.minimum_thumb_contrast = f64::NAN;
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("thumb edge threshold")
    );
    let mut input = f.input();
    input.focusable = false;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Accessibility(
            SliderAccessibilityError::EnabledNotFocusable
        ))
    ));
    let mut input = f.input();
    input.available_bounds = bounds(0.0, 0.0, 200.0, 160.0);
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Target(HitRegionError::Clipped))
    ));
    let occupied = [bounds(300.0, 0.0, 20.0, 20.0)];
    let mut input = f.input();
    input.occupied_regions = &occupied;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::Target(HitRegionError::Occupied(0)))
    ));
}

#[test]
fn different_track_appearance_cannot_validate_stale_thumb_reports() {
    let frost = Fixture::new(
        "frost",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let cast = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut input = frost.input();
    input.thumb = &cast.thumb;
    assert!(matches!(
        resolve_slider_snapshot(input),
        Err(SliderSnapshotError::OverstatedContrast {
            stage: "thumb edge",
            ..
        })
    ));
}

#[test]
fn label_between_endpoints_cannot_overlap_swept_thumb() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let label = resolve_command_label(
        CommandLabelInput {
            text: "L",
            typography: f.label.typography(),
            minimum_size: size(20.0, 12.0),
            maximum_size: size(20.0, 12.0),
            padding: SafeArea {
                start: 0.0,
                end: 0.0,
                top: 0.0,
                bottom: 0.0,
            },
            direction: LayoutDirection::Ltr,
        },
        |_| Ok::<_, Infallible>(size(20.0, 12.0)),
    )
    .unwrap();
    let mut input = f.input();
    input.label = &label;
    input.label_origin = PhysicalVector { x: 80.0, y: 5.0 };
    assert!(
        resolve_slider_snapshot(input)
            .unwrap_err()
            .to_string()
            .contains("label must not overlap")
    );
}

#[test]
fn published_colors_and_preferences_do_not_borrow_live_context() {
    let f = Fixture::new(
        "cast",
        LayoutDirection::Ltr,
        SliderOrientation::Horizontal,
        true,
        false,
        false,
        false,
    );
    let mut foreground = f.black.clone();
    let mut environment = f.environment.clone();
    let mut input = f.input();
    input.label_foreground = &foreground;
    input.environment = &environment;
    let snapshot = resolve_slider_snapshot(input).unwrap();
    foreground = color(1.0);
    let mut updated = serde_json::to_value(&environment).unwrap();
    updated["accessibilityPreferences"]["reducedMotion"] = json!(false);
    environment = serde_json::from_value(updated).unwrap();
    assert_eq!(foreground.components(), [1.0; 3]);
    assert!(!environment.accessibility_preferences().reduced_motion);
    assert_eq!(snapshot.label_foreground().components(), [0.0; 3]);
    assert!(snapshot.reduced_motion());
}
