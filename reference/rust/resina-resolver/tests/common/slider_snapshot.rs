use resina_color::{OpaqueSrgbRange, resolve_srgb_fallback};
use resina_environment::{EnvironmentSnapshot, LayoutDirection, SafeArea};
use resina_model::{
    ColorRole, PhysicalBounds, PhysicalVector, SliderAppearance, SliderPart, SliderValue,
    SurfaceIntent, SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandLabelInput, CommandLabelIr, HitRegionInput, HitRegionIr, LabelMeasureInput,
    SliderLayoutInput, SliderLayoutIr, SliderMinimumPosition, SliderOrientation,
    SliderPartPaintInput, SliderPartPaintIr, SliderPointerEvent, SliderPointerInput,
    SliderPointerRouting, SliderPointerState, SliderPointerTarget, SliderPresentation,
    SliderSnapshotInput, SliderStatesInput, SliderValueIr, SliderValuePolicy, SrgbFallback,
    compile_theme_source, resolve_command_label, resolve_hit_region, resolve_slider_layout,
    resolve_slider_part_paint, resolve_slider_pointer, resolve_slider_states, resolve_slider_value,
};
use serde_json::{Value, json};
use std::fmt::Debug;

pub fn size(width: f64, height: f64) -> SurfaceSize {
    SurfaceSize { width, height }
}
pub fn bounds(x: f64, y: f64, width: f64, height: f64) -> PhysicalBounds {
    PhysicalBounds {
        x,
        y,
        width,
        height,
    }
}
pub fn color(value: f64) -> SrgbFallback {
    resolve_srgb_fallback(&json!({"colorSpace":"srgb","components":[value,value,value]})).unwrap()
}
pub fn reserve(environment: &EnvironmentSnapshot, region: PhysicalBounds) -> HitRegionIr {
    resolve_hit_region(HitRegionInput {
        environment,
        visual_bounds: region,
        available_bounds: bounds(-300.0, -300.0, 1000.0, 1000.0),
        component_minimum: size(48.0, 48.0),
        occupied_regions: &[],
    })
    .unwrap()
}
pub struct FixtureConfig<'a> {
    pub family: &'a str,
    pub direction: LayoutDirection,
    pub orientation: SliderOrientation,
    pub enabled: bool,
    pub focused: bool,
    pub read_only: bool,
    pub preview: bool,
    pub tracking: f64,
    pub text_scale: f64,
    pub text: &'a str,
    pub label_maximum_size: SurfaceSize,
}
pub struct Fixture {
    pub environment: EnvironmentSnapshot,
    pub presentation: SliderPresentation,
    pub pointer: SliderPointerState,
    pub layout: SliderLayoutIr,
    pub track: SliderPartPaintIr,
    pub thumb: SliderPartPaintIr,
    pub label: CommandLabelIr,
    pub black: SrgbFallback,
    pub white: SrgbFallback,
    pub canvas: Vec<OpaqueSrgbRange>,
    pub target: HitRegionIr,
    pub enabled: bool,
    pub focused: bool,
    pub read_only: bool,
}
pub fn layout(
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
    pub fn resolve<E: Debug>(
        config: FixtureConfig<'_>,
        measure: impl FnMut(LabelMeasureInput<'_>) -> Result<SurfaceSize, E>,
    ) -> Self {
        let FixtureConfig {
            family,
            direction,
            orientation,
            enabled,
            focused,
            read_only,
            preview,
            tracking,
            text_scale,
            text,
            label_maximum_size,
        } = config;
        let request: Value = serde_json::from_str(include_str!(
            "../../../../../conformance/ir/command-paint-request.json"
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
        source["tokens"]["type"]["tracking"]["$value"]["value"] = json!(tracking);
        let theme = compile_theme_source(&source.to_string()).unwrap();
        let mut env = body["theme"]["environment"].clone();
        env["textScale"] = json!(text_scale);
        env["layoutDirection"] = serde_json::to_value(direction).unwrap();
        let environment: EnvironmentSnapshot = serde_json::from_value(env).unwrap();
        let current =
            resolve_slider_value(&SliderValue::try_new(-10.0, 30.0, 0.0).unwrap()).unwrap();
        let policy = SliderValuePolicy::Continuous;
        let initial = layout(&current, direction, orientation);
        let target = reserve(
            &environment,
            bounds(
                -12.0,
                -12.0,
                (label_maximum_size.width + 208.0).max(320.0),
                (label_maximum_size.height + 24.0).max(196.0),
            ),
        );
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
                        x: point.x
                            + if orientation == SliderOrientation::Horizontal {
                                24.0
                            } else {
                                0.0
                            },
                        y: point.y
                            + if orientation == SliderOrientation::Vertical {
                                24.0
                            } else {
                                0.0
                            },
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
            "../../../../../conformance/appearance/slider-appearance.json"
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
                text,
                typography: &resolved.typography()[&TypographyRole::Label],
                minimum_size: size(100.0, 64.0),
                maximum_size: label_maximum_size,
                padding: SafeArea {
                    start: 0.0,
                    end: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                },
                direction,
            },
            measure,
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
    pub fn input(&self) -> SliderSnapshotInput<'_, '_> {
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
