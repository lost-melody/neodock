use std::cell::Ref;
use std::cmp::Ordering;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

use crate::services::niri;

glib::wrapper! {
    /// Represents an application icon in the dock view.
    pub struct App(ObjectSubclass<imp::AppImpl>);
}

impl Default for App {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }

    pub fn windows<'a>(&'a self) -> Ref<'a, Vec<niri::NiriWindow>> {
        self.imp().windows.borrow()
    }

    pub fn find_window(&self, window: &niri::NiriWindow) -> Result<usize, usize> {
        let windows = self.imp().windows.borrow();
        windows.binary_search_by(|w| compare_windows(w, window))
    }

    pub fn add_window(&self, window: niri::NiriWindow) {
        let index = self.find_window(&window).unwrap_or_else(|e| e);
        let mut windows = self.imp().windows.borrow_mut();
        windows.insert(index, window);
    }

    /// Removes the window by the given id, and returns how many windows remaining.
    pub fn remove_window(&self, id: u64) -> usize {
        let mut windows = self.imp().windows.borrow_mut();
        if let Some(index) = windows.iter().position(|w| w.id() == id) {
            windows.remove(index);
        }
        windows.len()
    }
}

/// Compares two [niri::NiriWindow] by `app_id` and then `id`.
pub fn compare_windows(a: &niri::NiriWindow, b: &niri::NiriWindow) -> Ordering {
    a.app_id().cmp(&b.app_id()).then(a.id().cmp(&b.id()))
}

mod imp {
    use std::cell::RefCell;

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    use crate::services::niri;

    type Obj = super::App;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct AppImpl {
        pub(super) windows: RefCell<Vec<niri::NiriWindow>>,
        #[property(get, set)]
        app_id: RefCell<String>,
        #[property(get, set, nullable)]
        info: RefCell<Option<gio_unix::DesktopAppInfo>>,
    }

    impl AppImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {}
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppImpl {
        const NAME: &'static str = "NeoDockView";
        type Type = Obj;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for AppImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }
}
