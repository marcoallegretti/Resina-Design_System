#![cfg(feature = "testing")]

use guido::{prelude::*, testing::Headless};
use std::{cell::Cell, rc::Rc};

#[test]
fn closing_surface_suppresses_queued_clicks_without_suppressing_other_surfaces() {
    let mut app = Headless::new().expect("GUIdo GPU is required");
    let mut dialogs = Vec::new();
    for _ in 0..2 {
        let actions = Rc::new(Cell::new(0));
        let delivered = actions.clone();
        let lifetime = Rc::new(());
        let owned = lifetime.clone();
        let identity = Rc::new(Cell::new(None));
        let assigned = identity.clone();
        let surface = app.surface(SurfaceConfig::new().height(40), move || {
            container().width(160.0).height(40.0).on_click(move || {
                let _alive = &owned;
                delivered.set(delivered.get() + 1);
                guido::surface::surface_handle(identity.get().expect("configured surface")).close();
            })
        });
        assigned.set(Some(surface));
        app.configure(surface, 160, 40, 1.0);
        dialogs.push((surface, actions, lifetime));
    }
    app.step();
    for _ in 0..2 {
        for (surface, _, _) in &dialogs {
            app.click(*surface, 20.0, 20.0);
        }
    }
    app.step();
    for (_, actions, _) in &dialogs {
        assert_eq!(actions.get(), 1, "only the first queued click can act");
    }
    app.step();
    assert!(app.surfaces_live().is_empty());
    for (_, _, lifetime) in &dialogs {
        assert_eq!(Rc::strong_count(lifetime), 1, "closed widget is disposed");
    }
}
