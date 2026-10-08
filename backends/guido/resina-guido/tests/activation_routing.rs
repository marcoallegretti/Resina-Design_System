#![cfg(feature = "testing")]

use guido::{prelude::*, widget_prelude::*};
use resina_guido::activation_key_event;
use resina_model::{ActivationEvent, ActivationState};
use resina_resolver::resolve_activation;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

struct KeyboardProbe {
    state: Rc<RefCell<ActivationState>>,
    activations: Rc<Cell<usize>>,
    mounted: Rc<Cell<Option<WidgetId>>>,
    focus_id: RwSignal<Option<WidgetId>>,
    surface_active: RwSignal<bool>,
}

impl KeyboardProbe {
    fn observe_focus(&self, id: WidgetId) {
        let focused = self.surface_active.get_untracked() && guido::reactive::focus::has_focus(id);
        let next =
            resolve_activation(&self.state.borrow(), &ActivationEvent::Focus { focused }).unwrap();
        *self.state.borrow_mut() = next.state().clone();
    }
}

impl Widget for KeyboardProbe {
    fn layout(&mut self, ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        if self.mounted.get().is_none() {
            self.mounted.set(Some(ctx.id()));
            self.focus_id.set(Some(ctx.id()));
            guido::reactive::focus::request_focus(ctx.tree_ref(), ctx.id());
        }
        constraints.constrain(Size::new(64.0, 64.0))
    }

    fn paint(&self, _ctx: &mut PaintContext) {}

    fn event(&mut self, _tree: &mut Tree, id: WidgetId, event: &Event) -> EventResponse {
        if matches!(event, Event::FocusOut) {
            self.surface_active.set(false);
            let next = resolve_activation(
                &self.state.borrow(),
                &ActivationEvent::Focus { focused: false },
            )
            .unwrap();
            *self.state.borrow_mut() = next.state().clone();
            return EventResponse::Handled;
        }
        if matches!(event, Event::FocusIn) {
            self.surface_active.set(true);
        }
        self.observe_focus(id);
        let Some(mapped) = activation_key_event(&self.state.borrow(), event) else {
            return EventResponse::Ignored;
        };
        let result = resolve_activation(&self.state.borrow(), &mapped).unwrap();
        *self.state.borrow_mut() = result.state().clone();
        if result.activate() {
            self.activations.set(self.activations.get() + 1);
        }
        EventResponse::Handled
    }
}

#[test]
fn mounted_key_delivery_preserves_release_and_cancels_on_focus_transfer() {
    let mut app = guido::testing::Headless::new().expect("native GUIdo GPU is required");
    let state = Rc::new(RefCell::new(
        ActivationState::try_new(true, false, None).unwrap(),
    ));
    let activations = Rc::new(Cell::new(0));
    let mounted = Rc::new(Cell::new(None));
    let other = Rc::new(Cell::new(None));
    let owner_state = state.clone();
    let owner_activations = activations.clone();
    let owner_mounted = mounted.clone();
    let other_slot = other.clone();
    let surface = app.surface(SurfaceConfig::new().height(64), move || {
        let focus_id = create_signal(None);
        let surface_active = create_signal(false);
        let observed_state = owner_state.clone();
        create_effect(move || {
            let current = guido::reactive::focus::focus_path().widget();
            let focused =
                surface_active.get() && focus_id.get().is_some() && focus_id.get() == current;
            let next = resolve_activation(
                &observed_state.borrow(),
                &ActivationEvent::Focus { focused },
            )
            .unwrap();
            *observed_state.borrow_mut() = next.state().clone();
        });
        let probe = KeyboardProbe {
            state: owner_state,
            activations: owner_activations,
            mounted: owner_mounted,
            focus_id,
            surface_active,
        };
        let reference = create_widget_ref();
        other_slot.set(Some(reference));
        container()
            .width(128.0)
            .height(64.0)
            .child(probe)
            .child(container().width(64.0).height(64.0).widget_ref(reference))
    });
    app.configure(surface, 128, 64, 1.0);
    app.step();
    app.event_at(surface, Event::FocusIn, Instant::now());
    app.step();
    assert_eq!(guido::reactive::focus::focused_widget(), mounted.get());
    assert!(state.borrow().focused());
    for key in [Key::Char(' '), Key::Enter] {
        let before = activations.get();
        for repeat in [false, true, true] {
            app.event_at(
                surface,
                Event::KeyDown {
                    key,
                    modifiers: Modifiers::default(),
                    repeat,
                },
                Instant::now(),
            );
            app.step();
            assert!(state.borrow().hold().is_some());
            assert_eq!(activations.get(), before + usize::from(key == Key::Enter));
        }
        app.event_at(
            surface,
            Event::KeyUp {
                key,
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::default()
                },
            },
            Instant::now(),
        );
        app.step();
        assert!(state.borrow().hold().is_none());
        assert_eq!(activations.get(), before + 1);
    }
    let before = activations.get();
    app.event_at(
        surface,
        Event::KeyDown {
            key: Key::Char(' '),
            modifiers: Modifiers::default(),
            repeat: false,
        },
        Instant::now(),
    );
    app.step();
    assert!(state.borrow().pressed());
    app.event_at(surface, Event::FocusOut, Instant::now());
    app.step();
    assert!(!state.borrow().focused());
    assert!(state.borrow().hold().is_none());
    app.event_at(
        surface,
        Event::KeyDown {
            key: Key::Enter,
            modifiers: Modifiers::default(),
            repeat: false,
        },
        Instant::now(),
    );
    app.step();
    assert!(!state.borrow().focused());
    assert!(state.borrow().hold().is_none());
    assert_eq!(activations.get(), before);
    app.event_at(surface, Event::FocusIn, Instant::now());
    app.event_at(
        surface,
        Event::KeyUp {
            key: Key::Char(' '),
            modifiers: Modifiers::default(),
        },
        Instant::now(),
    );
    app.step();
    assert!(state.borrow().focused());
    assert_eq!(activations.get(), before);
    app.event_at(
        surface,
        Event::KeyDown {
            key: Key::Char(' '),
            modifiers: Modifiers::default(),
            repeat: false,
        },
        Instant::now(),
    );
    app.step();
    assert!(state.borrow().pressed());
    other.get().unwrap().focus();
    app.step();
    assert!(other.get().unwrap().is_focused());
    assert!(!state.borrow().focused());
    assert!(state.borrow().hold().is_none());
    app.event_at(
        surface,
        Event::KeyUp {
            key: Key::Char(' '),
            modifiers: Modifiers::default(),
        },
        Instant::now(),
    );
    app.step();
    assert_eq!(activations.get(), before);
    assert!(app.frames_presented(surface) > 0);
}
