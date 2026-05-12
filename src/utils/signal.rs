use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;

/// A signals store that manages object's event signals,
/// and disconnects them at once on dropped.
pub struct Signals<T: IsA<glib::Object>> {
    object: glib::WeakRef<T>,
    signals: Vec<Option<glib::SignalHandlerId>>,
}

impl<T: IsA<glib::Object>> Drop for Signals<T> {
    fn drop(&mut self) {
        self.disconnect_all();
    }
}

impl<T: IsA<glib::Object>> Signals<T> {
    pub fn new(object: &T) -> Self {
        Self {
            object: object.downgrade(),
            signals: Vec::default(),
        }
    }

    pub fn object(&self) -> Option<T> {
        self.object.upgrade()
    }

    /// Adds signal handler id to managed list.
    pub fn add_signal(&mut self, signal: glib::SignalHandlerId) {
        self.signals.push(Some(signal));
    }

    pub fn disconnect_all(&mut self) {
        if let Some(object) = self.object() {
            for signal in &mut self.signals {
                object.disconnect(signal.take().unwrap());
            }
        }
        self.signals = Vec::default();
    }
}

pub trait AssignSignalsExt<T: IsA<glib::Object>> {
    /// Assigns signal handler id to [Signals].
    fn assign_signals(self, signals: &mut Signals<T>);
}

impl<T: IsA<glib::Object>> AssignSignalsExt<T> for glib::SignalHandlerId {
    fn assign_signals(self, signals: &mut Signals<T>) {
        signals.add_signal(self);
    }
}

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
