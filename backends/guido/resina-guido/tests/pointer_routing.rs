#![cfg(feature = "testing")]

use guido::{prelude::*, widget_prelude::*};
use resina_model::{ActivationEvent, ActivationState};
use resina_resolver::resolve_activation;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

#[derive(Debug, PartialEq)]
enum Delivery {
    Down(MouseButton),
    Move(f32),
    Up(MouseButton),
}

struct PointerProbe(Rc<RefCell<Vec<Delivery>>>);

impl Widget for PointerProbe {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(100.0, 20.0))
    }

    fn paint(&self, _ctx: &mut PaintContext) {}

    fn event(&mut self, _tree: &mut Tree, _id: WidgetId, event: &Event) -> EventResponse {
        let delivery = match event {
            Event::MouseDown { button, .. } => Delivery::Down(*button),
            Event::MouseMove { at: Some(at), .. } => Delivery::Move(at.x),
            Event::MouseUp { button, .. } => Delivery::Up(*button),
            _ => return EventResponse::Ignored,
        };
        self.0.borrow_mut().push(delivery);
        EventResponse::Ignored
    }
}

#[test]
fn mounted_press_route_survives_secondary_click_until_final_release() {
    let mut app = guido::testing::Headless::new().expect("native GUIdo GPU is required");
    let deliveries: Vec<_> = (0..5).map(|_| Rc::new(RefCell::new(Vec::new()))).collect();
    let mounted = deliveries.clone();
    let surface = app.surface(SurfaceConfig::new().height(20), move || {
        container()
            .width(500.0)
            .height(20.0)
            .layout(Flex::row())
            .children(
                mounted
                    .into_iter()
                    .map(|log| Box::new(PointerProbe(log)) as Box<dyn Widget>)
                    .collect::<Vec<_>>(),
            )
    });
    app.configure(surface, 500, 20, 1.0);
    app.step();

    for secondary in [MouseButton::Right, MouseButton::Middle] {
        for release_primary_first in [false, true] {
            app.event_at(surface, Event::mouse_move(10.0, 10.0), Instant::now());
            app.step();
            for log in &deliveries {
                log.borrow_mut().clear();
            }
            let (first, last) = if release_primary_first {
                (MouseButton::Left, secondary)
            } else {
                (secondary, MouseButton::Left)
            };
            for event in [
                Event::mouse_down(10.0, 10.0, MouseButton::Left),
                Event::mouse_move(450.0, 10.0),
                Event::mouse_down(450.0, 10.0, secondary),
                Event::mouse_up(450.0, 10.0, first),
                Event::mouse_move(460.0, 10.0),
                Event::mouse_up(460.0, 10.0, last),
            ] {
                app.event_at(surface, event, Instant::now());
                app.step();
            }
            assert_eq!(
                *deliveries[0].borrow(),
                [
                    Delivery::Down(MouseButton::Left),
                    Delivery::Move(450.0),
                    Delivery::Down(secondary),
                    Delivery::Up(first),
                    Delivery::Move(460.0),
                    Delivery::Up(last),
                ],
                "secondary {secondary:?}, release primary first {release_primary_first}"
            );
            for (index, log) in deliveries.iter().enumerate().skip(2) {
                assert!(
                    log.borrow()
                        .iter()
                        .all(|entry| matches!(entry, Delivery::Move(_))
                            || matches!(entry, Delivery::Up(button) if *button == last)),
                    "probe {index}, secondary {secondary:?}, release primary first {release_primary_first}: {:?}",
                    log.borrow()
                );
            }

            app.event_at(surface, Event::mouse_move(450.0, 10.0), Instant::now());
            app.step();
            deliveries[0].borrow_mut().clear();
            app.event_at(
                surface,
                Event::mouse_down(460.0, 10.0, secondary),
                Instant::now(),
            );
            app.step();
            assert!(
                deliveries[0].borrow().is_empty(),
                "final release must drop the route"
            );
            assert_eq!(
                deliveries[4].borrow().last(),
                Some(&Delivery::Down(secondary))
            );
            app.event_at(
                surface,
                Event::mouse_up(460.0, 10.0, secondary),
                Instant::now(),
            );
            app.step();
        }
    }
    assert!(app.frames_presented(surface) > 0);
}

// This is widget-tree injection into the real mounted GUIdo runtime. It is not
// compositor input, native capture-effect execution, or accessibility evidence.
struct CommandPointerProbe {
    state: Rc<RefCell<ActivationState>>,
    effects: Rc<Cell<usize>>,
}

impl Widget for CommandPointerProbe {
    fn layout(&mut self, _ctx: &mut LayoutCtx, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(100.0, 20.0))
    }

    fn paint(&self, _ctx: &mut PaintContext) {}

    fn event(&mut self, _tree: &mut Tree, _id: WidgetId, event: &Event) -> EventResponse {
        let inside = event
            .coords()
            .is_some_and(|at| at.x >= 0.0 && at.x < 100.0 && at.y >= 0.0 && at.y < 20.0);
        let id = "mouse-left".to_owned();
        let mapped = match event {
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            } => ActivationEvent::PointerDown { id, inside },
            Event::MouseMove { .. } | Event::MouseLeave => {
                ActivationEvent::PointerMove { id, inside }
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => ActivationEvent::PointerUp { id, inside },
            _ => return EventResponse::Ignored,
        };
        let result = resolve_activation(&self.state.borrow(), &mapped).unwrap();
        *self.state.borrow_mut() = result.state().clone();
        if result.activate() {
            // Observe the committed state as a consumer would, then revoke
            // availability reentrantly before another native event arrives.
            assert!(self.state.borrow().hold().is_none());
            self.effects.set(self.effects.get() + 1);
            change_availability(&self.state, false);
        }
        EventResponse::Handled
    }
}

fn change_availability(state: &RefCell<ActivationState>, enabled: bool) {
    let result =
        resolve_activation(&state.borrow(), &ActivationEvent::Availability { enabled }).unwrap();
    assert!(!result.activate());
    *state.borrow_mut() = result.state().clone();
}

#[test]
fn mounted_command_pointer_delivery_preserves_one_intent_and_revocation() {
    let mut app = guido::testing::Headless::new().expect("native GUIdo GPU is required");
    let state = Rc::new(RefCell::new(
        ActivationState::try_new(true, false, None).unwrap(),
    ));
    let effects = Rc::new(Cell::new(0));
    let mounted_state = state.clone();
    let mounted_effects = effects.clone();
    let surface = app.surface(SurfaceConfig::new().height(20), move || {
        container()
            .width(500.0)
            .height(20.0)
            .layout(Flex::row())
            .child(CommandPointerProbe {
                state: mounted_state,
                effects: mounted_effects,
            })
            .child(container().width(400.0).height(20.0))
    });
    app.configure(surface, 500, 20, 1.0);
    app.step();
    let mut deliver = |event| {
        app.event_at(surface, event, Instant::now());
        app.step();
    };

    // The pointer's hold is independent of keyboard focus. Continued outside
    // delivery must update Rust's pressed state, then reentry may arm release.
    deliver(Event::mouse_down(10.0, 10.0, MouseButton::Left));
    assert!(state.borrow().pressed());
    assert!(!state.borrow().focused());
    deliver(Event::mouse_move(450.0, 10.0));
    assert!(state.borrow().hold().is_some());
    assert!(!state.borrow().pressed());
    deliver(Event::mouse_down(450.0, 10.0, MouseButton::Right));
    deliver(Event::mouse_up(450.0, 10.0, MouseButton::Right));
    assert!(state.borrow().hold().is_some());
    deliver(Event::mouse_move(10.0, 10.0));
    assert!(state.borrow().pressed());
    deliver(Event::mouse_up(10.0, 10.0, MouseButton::Left));
    assert_eq!(effects.get(), 1);
    assert!(!state.borrow().enabled());
    assert!(state.borrow().hold().is_none());
    deliver(Event::mouse_up(10.0, 10.0, MouseButton::Left));
    deliver(Event::mouse_down(10.0, 10.0, MouseButton::Left));
    deliver(Event::mouse_up(10.0, 10.0, MouseButton::Left));
    assert_eq!(effects.get(), 1);
    assert!(state.borrow().hold().is_none());

    change_availability(&state, true);
    deliver(Event::mouse_down(10.0, 10.0, MouseButton::Left));
    change_availability(&state, false);
    change_availability(&state, true);
    deliver(Event::mouse_up(10.0, 10.0, MouseButton::Left));
    assert_eq!(
        effects.get(),
        1,
        "reenabling must not revive the interrupted press"
    );
    assert!(state.borrow().hold().is_none());

    deliver(Event::mouse_down(10.0, 10.0, MouseButton::Left));
    deliver(Event::mouse_move(450.0, 10.0));
    deliver(Event::mouse_up(450.0, 10.0, MouseButton::Left));
    assert_eq!(effects.get(), 1, "outside release must not activate");
    assert!(state.borrow().hold().is_none());

    deliver(Event::mouse_down(10.0, 10.0, MouseButton::Left));
    deliver(Event::mouse_up(10.0, 10.0, MouseButton::Left));
    assert_eq!(effects.get(), 2, "a fresh eligible gesture must still work");
    assert!(state.borrow().hold().is_none());
}
