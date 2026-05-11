use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;

/// Helper used to implement "connect_once".
pub struct OnceCallback<F>(Rc<RefCell<(Option<F>, Option<glib::SignalHandlerId>)>>);

impl<F> Clone for OnceCallback<F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<F> OnceCallback<F> {
    pub fn new(f: F) -> Self {
        let once = Self(Default::default());
        once.0.borrow_mut().0.replace(f);
        once
    }

    /// Stores the callback function and signal id.
    pub fn store(&self, id: glib::SignalHandlerId) {
        self.0.borrow_mut().1.replace(id);
    }

    /// Disconnects signal and returns callback if present.
    pub fn disconnect(&self, obj: &impl IsA<glib::Object>) -> Option<F> {
        let mut tuple = self.0.borrow_mut();
        if let Some(id) = tuple.1.take() {
            obj.disconnect(id);
            return tuple.0.take();
        }
        None
    }
}

pub trait AssignCallbackExt {
    /// Assigns signal handler id to [OnceCallback].
    fn assign_callback<F>(self, once: &OnceCallback<F>);
}

impl AssignCallbackExt for glib::SignalHandlerId {
    fn assign_callback<F>(self, once: &OnceCallback<F>) {
        once.store(self);
    }
}
