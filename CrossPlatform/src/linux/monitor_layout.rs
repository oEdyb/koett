use gtk4::{gdk, glib, prelude::*};
use std::{cell::Cell, rc::Rc};

pub(super) struct MonitorLayout {
    pub monitors: Vec<gdk::Monitor>,
    changed: Rc<Cell<bool>>,
    handlers: Vec<(glib::Object, glib::SignalHandlerId)>,
}

impl MonitorLayout {
    pub fn new() -> Self {
        let changed = Rc::new(Cell::new(false));
        let mut layout = Self {
            monitors: Vec::new(),
            changed,
            handlers: Vec::new(),
        };
        if let Some(display) = gdk::Display::default() {
            let monitors = display.monitors();
            let changed = layout.changed.clone();
            let handler = monitors.connect_items_changed(move |_, _, _, _| changed.set(true));
            layout.handlers.push((monitors.upcast(), handler));
            layout.monitors = display
                .monitors()
                .iter::<gdk::Monitor>()
                .filter_map(Result::ok)
                .filter(|monitor| monitor.is_valid())
                .collect();
            for monitor in &layout.monitors {
                let changed = layout.changed.clone();
                let handler = monitor.connect_notify_local(None, move |_, property| {
                    if matches!(
                        property.name(),
                        "geometry" | "scale" | "scale-factor" | "connector" | "valid"
                    ) {
                        changed.set(true);
                    }
                });
                layout.handlers.push((monitor.clone().upcast(), handler));
                let changed = layout.changed.clone();
                let handler = monitor.connect_invalidate(move |_| changed.set(true));
                layout.handlers.push((monitor.clone().upcast(), handler));
            }
        }
        layout
    }

    pub fn changed(&self) -> bool {
        self.changed.get()
    }
}

impl Drop for MonitorLayout {
    fn drop(&mut self) {
        for (object, handler) in self.handlers.drain(..) {
            object.disconnect(handler);
        }
    }
}
