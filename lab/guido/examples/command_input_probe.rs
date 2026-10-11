//! Fixed-context native input verification fixture; not a reusable accessible control.
use guido::{prelude::*, renderer::DrawCommand, transform::Transform, widget_prelude::*};
use resina_environment::SafeArea;
use resina_guido::{activation_key_event, measure_command_label, prepare_command_content};
use resina_model::{
    ActivationEvent, ActivationState, CommandAnatomy, CommandEmphasis, PhysicalBounds,
    PhysicalVector, SurfaceSize, TypographyRole,
};
use resina_resolver::{
    CommandLabelInput, CommandSnapshotInput, HitRegionIr, SurfaceHitRegionInput,
    resolve_activation, resolve_command_label, resolve_command_paint_source,
    resolve_command_snapshot, resolve_command_states, resolve_surface_hit_region,
    resolve_theme_request_source,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

struct Frames {
    commands: Vec<[DrawCommand; 2]>,
    target: HitRegionIr,
    canvas: Color,
}
fn frame_index(state: &ActivationState, hovered: bool) -> usize {
    let phase = if !state.enabled() {
        3
    } else if state.pressed() {
        2
    } else if hovered {
        1
    } else {
        0
    };
    phase * 2 + usize::from(state.focused())
}

fn prepare(root: &str, scenes: &str, family: FontFamily) -> Result<Frames, Box<dyn Error>> {
    let catalog: Value = serde_json::from_str(scenes)?;
    let scene = catalog["scenarios"]
        .as_array()
        .ok_or("missing scenarios")?
        .iter()
        .find(|v| v["name"] == "light-elastomer-paint-focused")
        .ok_or("missing authored light scene")?;
    let anatomy: CommandAnatomy = serde_json::from_str(&std::fs::read_to_string(format!(
        "{root}/definitions/components/command.json"
    ))?)?;
    let variant = anatomy.variant(CommandEmphasis::Standard);
    let appearance: Value = serde_json::from_str(&std::fs::read_to_string(format!(
        "{root}/definitions/command-appearance-light.json"
    ))?)?;
    let theme = resolve_theme_request_source(&scene["request"]["body"]["theme"].to_string())?;
    let environment: resina_environment::EnvironmentSnapshot =
        serde_json::from_value(scene["request"]["body"]["theme"]["environment"].clone())?;
    let label = resolve_command_label(
        CommandLabelInput {
            text: "Activate probe",
            typography: &theme.typography()[&TypographyRole::Label],
            minimum_size: SurfaceSize {
                width: 160.0,
                height: 64.0,
            },
            maximum_size: SurfaceSize {
                width: 320.0,
                height: 160.0,
            },
            padding: SafeArea {
                start: 26.0,
                end: 26.0,
                top: 26.0,
                bottom: 26.0,
            },
            direction: environment.layout_direction(),
        },
        |input| measure_command_label(family, input),
    )?;
    let available = PhysicalBounds {
        x: -40.0,
        y: -40.0,
        width: 500.0,
        height: 240.0,
    };
    let minimum = SurfaceSize {
        width: 48.0,
        height: 48.0,
    };
    let mut commands = Vec::new();
    let mut target = None;
    for phase in 0..4 {
        for focused in [false, true] {
            let hold = (phase == 2).then(|| resina_model::PressHold::Pointer {
                id: "mouse-left".into(),
                inside: true,
            });
            let activation = ActivationState::try_new(phase != 3, focused, hold)?;
            let hovered = phase == 1;
            let mut surface = scene["request"].clone();
            surface["body"]["surface"] = serde_json::to_value(
                variant
                    .body()
                    .intent(resolve_command_states(&activation, hovered)),
            )?;
            surface["body"]["foregroundRole"] =
                serde_json::to_value(variant.body().content_role())?;
            surface["body"]["size"] = serde_json::to_value(label.size())?;
            surface["body"]["minimumContentContrast"] = json!(4.5);
            surface["body"]["minimumEdgeContrast"] = json!(3.0);
            let request =
                json!({"schemaVersion":"0.1.0","surface":surface,"commandAppearance":appearance});
            let paint = resolve_command_paint_source(&request.to_string())?;
            if target.is_none() {
                target = Some(resolve_surface_hit_region(SurfaceHitRegionInput {
                    environment: &environment,
                    body: paint.paint().body(),
                    available_bounds: available,
                    component_minimum: minimum,
                    occupied_regions: &[],
                })?);
            }
            let snapshot = resolve_command_snapshot(CommandSnapshotInput {
                label: &label,
                paint: &paint,
                hit_region: target.unwrap(),
                activation: &activation,
                hovered,
                description: Some("Increment the probe activation counter."),
                focusable: true,
                environment: &environment,
                available_bounds: available,
                component_minimum: minimum,
                occupied_regions: &[],
            })?;
            commands.push(prepare_command_content(&snapshot, family, 1.0, 4)?);
        }
    }
    let canvas = resina_resolver::resolve_srgb_fallback(&scene["request"]["surroundingColor"])?;
    if canvas.alpha() != 1.0 {
        return Err("input fixture requires an opaque authored canvas".into());
    }
    let [r, g, b] = canvas.components().map(|v| v as f32);
    Ok(Frames {
        commands,
        target: target.unwrap(),
        canvas: Color::rgb(r, g, b),
    })
}

struct Probe {
    frames: Rc<Frames>,
    state: Rc<RefCell<ActivationState>>,
    revision: RwSignal<bool>,
    generation: u64,
    native_id: Cell<Option<WidgetId>>,
    hovered: RwSignal<bool>,
    surface_active: RwSignal<bool>,
    mounted: RwSignal<Option<WidgetId>>,
    effects: Rc<Cell<usize>>,
    failure: Rc<RefCell<Option<String>>>,
}

impl Drop for Probe {
    fn drop(&mut self) {
        // GUIdo disposes a dynamic child's reactive owner before dropping its
        // widget. Retirement must not read or write disposed signal handles.
        let before = self.state.borrow().clone();
        let cancelled = resolve_activation(&before, &ActivationEvent::Cancel {}).unwrap();
        let unavailable = resolve_activation(
            cancelled.state(),
            &ActivationEvent::Availability { enabled: false },
        )
        .unwrap();
        let retired = resolve_activation(
            unavailable.state(),
            &ActivationEvent::Focus { focused: false },
        )
        .unwrap();
        *self.state.borrow_mut() = retired.state().clone();
        println!(
            "RESINA_NATIVE_RETIRED {}",
            json!({
                "generation": self.generation,
                "widgetId": self.native_id.get().map(WidgetId::as_u64),
                "previousHold": before.hold(),
                "state": retired.state(),
                "captureRequest": cancelled.capture(),
            })
        );
    }
}

fn build_probe(
    frames: Rc<Frames>,
    generation: u64,
    surface_active: RwSignal<bool>,
    effects: Rc<Cell<usize>>,
    failure: Rc<RefCell<Option<String>>>,
) -> Probe {
    let state = Rc::new(RefCell::new(
        ActivationState::try_new(true, false, None).unwrap(),
    ));
    let revision = create_signal(false);
    let hovered = create_signal(false);
    let mounted = create_signal(None);
    let observed = state.clone();
    create_effect(move || {
        let focused = surface_active.get()
            && mounted.get().is_some()
            && mounted.get() == guido::reactive::focus::focus_path().widget();
        let previous = observed.borrow().clone();
        let next = resolve_activation(&previous, &ActivationEvent::Focus { focused }).unwrap();
        if previous != *next.state() {
            *observed.borrow_mut() = next.state().clone();
            revision.set(!revision.get_untracked());
            println!(
                "RESINA_NATIVE_FOCUS {}",
                json!({"generation":generation,"focused":focused})
            );
        }
    });
    Probe {
        frames,
        state,
        revision,
        generation,
        native_id: Cell::new(None),
        hovered,
        surface_active,
        mounted,
        effects,
        failure,
    }
}

// A separate native surface provides real Wayland keyboard-focus transfer.
// It is a verification target, not a second Command or product action.
struct FocusPeer;
impl Widget for FocusPeer {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(200.0, 120.0))
    }
    fn paint(&self, ctx: &mut PaintContext) {
        ctx.draw_text_styled(
            "Focus target",
            Rect::new(16.0, 16.0, 168.0, 88.0),
            Color::BLACK,
            20.0,
            FontFamily::name("DejaVu Sans"),
            FontWeight::NORMAL,
            LineHeight::default(),
            0.0,
            None,
        );
        println!("RESINA_NATIVE_PEER_PAINT");
    }
    fn event(&mut self, tree: &mut Tree, id: WidgetId, event: &Event) -> EventResponse {
        match event {
            Event::MouseMove { .. } | Event::MouseEnter { .. } => {
                println!("RESINA_NATIVE_PEER_POINTER_READY");
                EventResponse::Handled
            }
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            }
            | Event::FocusIn => {
                guido::reactive::focus::request_focus(tree, id);
                if matches!(event, Event::FocusIn) {
                    println!("RESINA_NATIVE_PEER_FOCUS");
                } else {
                    println!("RESINA_NATIVE_PEER_POINTER");
                }
                EventResponse::Handled
            }
            Event::KeyUp {
                key: Key::Char(' '),
                ..
            } => {
                println!("RESINA_NATIVE_PEER_RELEASE");
                EventResponse::Handled
            }
            _ => EventResponse::Ignored,
        }
    }
}
impl Probe {
    fn fail(&self, message: &str) {
        *self.failure.borrow_mut() = Some(message.to_owned());
        eprintln!("native command probe: {message}");
        self.apply(ActivationEvent::Availability { enabled: false });
        guido::quit_app();
    }
    fn apply(&self, event: ActivationEvent) {
        let result = resolve_activation(&self.state.borrow(), &event).unwrap();
        *self.state.borrow_mut() = result.state().clone();
        self.revision.set(!self.revision.get_untracked());
        println!(
            "RESINA_NATIVE_STATE {}",
            json!({"generation":self.generation,"event":event,"state":result.state(),"captureRequest":result.capture()})
        );
        if result.activate() {
            self.effects.set(self.effects.get() + 1);
            println!(
                "RESINA_NATIVE_EFFECT {}",
                json!({"generation":self.generation,"effects":self.effects.get(),"state":*self.state.borrow()})
            );
            if self.effects.get() == 3 {
                self.apply(ActivationEvent::Availability { enabled: false });
            }
        }
    }
}
impl Widget for Probe {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        if self.mounted.get_untracked().is_none() {
            self.mounted.set(Some(ctx.id()));
            self.native_id.set(Some(ctx.id()));
            guido::reactive::focus::request_focus(ctx.tree_ref(), ctx.id());
        }
        let b = self.frames.target.bounds();
        let desired = Size::new(b.width as f32, b.height as f32);
        let actual = constraints.constrain(desired);
        if actual.width < desired.width || actual.height < desired.height {
            self.fail("native layout cannot contain the fixed target");
        }
        actual
    }
    fn paint(&self, ctx: &mut PaintContext) {
        if self.failure.borrow().is_some() {
            return;
        }
        if guido::outputs::outputs()
            .get()
            .iter()
            .any(|output| output.scale_factor != 1)
        {
            self.fail("input fixture requires unit output scale");
            return;
        }
        self.revision.get();
        let state = self.state.borrow().clone();
        let hovered = self.hovered.get();
        let native_focused = self.surface_active.get()
            && self
                .mounted
                .get()
                .is_some_and(guido::reactive::focus::has_focus);
        if state.focused() != native_focused {
            self.fail("committed focus disagrees with current native focus");
            return;
        }
        let b = self.frames.target.bounds();
        ctx.apply_transform(Transform::translate(-b.x as f32, -b.y as f32));
        for command in &self.frames.commands[frame_index(&state, hovered)] {
            match command {
                DrawCommand::Image {
                    source,
                    rect,
                    content_fit,
                    tint,
                    ..
                } => ctx.draw_image(source.clone(), *rect, *content_fit, *tint),
                DrawCommand::Text {
                    text,
                    rect,
                    color,
                    font_size,
                    font_family,
                    font_weight,
                    line_height,
                    letter_spacing,
                    align,
                    fit,
                } => ctx.draw_text_decorated(
                    text,
                    *rect,
                    *color,
                    *font_size,
                    *font_family,
                    *font_weight,
                    *line_height,
                    *letter_spacing,
                    *align,
                    None,
                    None,
                    *fit,
                ),
                _ => unreachable!(),
            }
        }
        println!(
            "RESINA_NATIVE_PAINT {}",
            json!({"generation":self.generation,"widgetId":self.native_id.get().map(WidgetId::as_u64),"state":state,"hovered":hovered,"target":b,"nativeFocused":native_focused})
        );
    }
    fn event(&mut self, tree: &mut Tree, id: WidgetId, event: &Event) -> EventResponse {
        if self.failure.borrow().is_some() {
            return EventResponse::Ignored;
        }
        let b = self.frames.target.bounds();
        let inside = event.coords().is_some_and(|at| {
            self.frames
                .target
                .contains(PhysicalVector {
                    x: f64::from(at.x) + b.x,
                    y: f64::from(at.y) + b.y,
                })
                .unwrap()
        });
        if matches!(event, Event::FocusOut) {
            self.surface_active.set(false);
            self.apply(ActivationEvent::Cancel {});
            self.apply(ActivationEvent::Focus { focused: false });
            return EventResponse::Handled;
        }
        if matches!(event, Event::FocusIn) {
            self.surface_active.set(true);
        }
        let focused = self.surface_active.get_untracked() && guido::reactive::focus::has_focus(id);
        if self.state.borrow().focused() != focused {
            self.apply(ActivationEvent::Focus { focused });
        }
        let mapped = match event {
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            } => {
                guido::reactive::focus::request_focus(tree, id);
                Some(ActivationEvent::PointerDown {
                    id: "mouse-left".into(),
                    inside,
                })
            }
            Event::MouseMove { .. } | Event::MouseEnter { .. } | Event::MouseLeave => {
                self.hovered.set(inside);
                Some(ActivationEvent::PointerMove {
                    id: "mouse-left".into(),
                    inside,
                })
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => Some(ActivationEvent::PointerUp {
                id: "mouse-left".into(),
                inside,
            }),
            _ => activation_key_event(&self.state.borrow(), event),
        };
        if let Some(mapped) = mapped {
            self.apply(mapped);
            EventResponse::Handled
        } else {
            EventResponse::Ignored
        }
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let peer_mode = args.len() == 4 && args[3] == "--focus-peer";
    if args.len() != 3 && !peer_mode {
        return Err(
            "usage: native-command-probe <repository> <scenes.json> <DejaVuSans.ttf> [--focus-peer]".into(),
        );
    }
    let font = std::fs::read(&args[2])?;
    let mut database = fontdb::Database::new();
    database.load_font_data(font.clone());
    if !database.faces().any(|face| {
        face.families
            .iter()
            .any(|(family, _)| family == "DejaVu Sans")
    }) {
        return Err("input fixture requires DejaVu Sans in the supplied font file".into());
    }
    guido::load_font(font);
    let frames = Rc::new(prepare(
        &args[0],
        &std::fs::read_to_string(&args[1])?,
        FontFamily::name("DejaVu Sans"),
    )?);
    let target = frames.target.bounds();
    if peer_mode {
        let canvas = frames.canvas;
        let reason = App::new().run(move |app| {
            app.add_surface(
                SurfaceConfig::new()
                    .width(200)
                    .height(120)
                    .anchor(Anchor::TOP | Anchor::RIGHT)
                    .keyboard_interactivity(KeyboardInteractivity::OnDemand)
                    .background_color(canvas),
                || FocusPeer,
            );
        });
        return match reason {
            guido::ExitReason::Quit => Ok(()),
            reason => Err(format!("focus target runtime exited: {reason:?}").into()),
        };
    }
    println!(
        "RESINA_NATIVE_PREPARED {}",
        json!({"target":target,"profile":"DPR1-input-probe","nativeAT":false,"placement":{"x":40,"y":40},"focusPeer":{"width":200,"height":120}})
    );
    let canvas = frames.canvas;
    let failure = Rc::new(RefCell::new(None));
    let native_failure = failure.clone();
    let reason = App::new().run(move |app| {
        let surface_active = create_signal(false);
        let generation = create_signal(0u64);
        let effects = Rc::new(Cell::new(0));
        app.add_surface(
            SurfaceConfig::new()
                .width(500)
                .height(240)
                .anchor(Anchor::TOP | Anchor::LEFT)
                .keyboard_interactivity(KeyboardInteractivity::OnDemand)
                .background_color(canvas),
            move || {
                let replacement_failure = native_failure.clone();
                container()
                    .width(500.0)
                    .height(240.0)
                    .padding(40.0)
                    .on_key_down(move |key, _, repeat| {
                        // Consumer verification shortcut; Command key semantics
                        // continue to accept only their existing Space/Enter map.
                        if key == Key::Escape && !repeat {
                            let next = generation.get_untracked().checked_add(1);
                            if let Some(next) = next {
                                generation.set(next);
                            } else {
                                *replacement_failure.borrow_mut() =
                                    Some("generation exhausted".into());
                                eprintln!("native command probe: generation exhausted");
                                guido::quit_app();
                            }
                        }
                    })
                    .child(move || {
                        build_probe(
                            frames.clone(),
                            generation.get(),
                            surface_active,
                            effects.clone(),
                            native_failure.clone(),
                        )
                    })
            },
        );
    });
    if let Some(error) = failure.borrow().clone() {
        return Err(error.into());
    }
    match reason {
        guido::ExitReason::Quit => Ok(()),
        reason => Err(format!("native runtime exited: {reason:?}").into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("native command probe: {error}");
        std::process::exit(1);
    }
}
