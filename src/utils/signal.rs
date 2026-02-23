use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;

/// Helper used to implement "connect_once".
pub struct OnceCallback<F>(Rc<RefCell<Option<(F, glib::SignalHandlerId)>>>);

impl<F> Clone for OnceCallback<F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<F> Default for OnceCallback<F> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<F> OnceCallback<F> {
    /// Stores the callback function and signal id.
    pub fn store(&self, f: F, id: glib::SignalHandlerId) {
        self.0.borrow_mut().replace((f, id));
    }

    /// Disconnects signal and returns callback if present.
    pub fn disconnect(&self, obj: &impl IsA<glib::Object>) -> Option<F> {
        if let Some((f, id)) = self.0.borrow_mut().take() {
            obj.disconnect(id);
            return Some(f);
        }
        None
    }
}
