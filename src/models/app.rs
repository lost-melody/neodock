use std::cmp::Ordering;

use gtk::glib;
use gtk::prelude::*;
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

    /// Returns the window at `pos` of the sorted store.
    pub fn get_window(&self, pos: u32) -> Option<niri::NiriWindow> {
        self.sorted_windows().unwrap().item(pos).and_downcast()
    }

    pub fn add_window(&self, window: niri::NiriWindow) {
        self.windows().unwrap().append(&window);
        self.notify_windows();
        // reorders windows when their layouts changed.
        window.connect_layout_notify(glib::clone!(
            #[weak(rename_to=obj)]
            self,
            move |window| {
                let windows = obj.windows().unwrap();
                let id = window.id();
                if let Some(index) =
                    windows.find_with_equal_func(|o| o.downcast_ref::<niri::NiriWindow>().is_some_and(|w| w.id() == id))
                {
                    // By marking window at `index` as dirty, triggers a sorting process.
                    windows.items_changed(index, 1, 1);
                    obj.notify_windows();
                }
            }
        ));
    }

    /// Removes the window by the given id, and returns how many windows remaining.
    pub fn remove_window(&self, id: u64) -> u32 {
        let windows = self.windows().unwrap();
        if let Some(index) =
            windows.find_with_equal_func(|o| o.downcast_ref::<niri::NiriWindow>().is_some_and(|w| w.id() == id))
        {
            windows.remove(index);
        }
        self.notify_windows();
        windows.n_items()
    }
}

/// Compares two [App]s by `app_id`.
pub fn compare_apps(a: &App, b: &App) -> Ordering {
    a.app_id().cmp(&b.app_id())
}

/// Compares two [niri::NiriWindow]s by `app_id` and then `id`.
pub fn compare_windows(a: &niri::NiriWindow, b: &niri::NiriWindow) -> Ordering {
    a.app_id()
        .cmp(&b.app_id())
        .then(compare_windows_pos(a, b))
        .then(a.id().cmp(&b.id()))
}

/// Compares two [niri::NiriWindow]s by their `pos_in_scrolling_layout`s.
pub fn compare_windows_pos(a: &niri::NiriWindow, b: &niri::NiriWindow) -> Ordering {
    let (a, b) = (
        a.get_layout().pos_in_scrolling_layout.unwrap_or_default(),
        b.get_layout().pos_in_scrolling_layout.unwrap_or_default(),
    );
    a.cmp(&b)
}

mod imp {
    use std::cell::RefCell;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};
    use gtk4 as gtk;

    use crate::services::niri;

    type Obj = super::App;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct AppImpl {
        /// Source windows store, where windows should be inserted or removed.
        #[property(get)]
        windows: RefCell<Option<gio::ListStore>>,
        /// Sorted windows store, where windows should be retrieved.
        #[property(get)]
        sorted_windows: RefCell<Option<gtk::SortListModel>>,
        /// The application id of windows, and the filename of `.desktop` entries.
        #[property(get, set)]
        app_id: RefCell<String>,
        /// Corresponding [gio_unix::DesktopAppInfo] object, if present.
        #[property(get, set, nullable)]
        info: RefCell<Option<gio_unix::DesktopAppInfo>>,
    }

    impl AppImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let windows = gio::ListStore::new::<niri::NiriWindow>();
            let sorter = gtk::CustomSorter::new(|a, b| {
                super::compare_windows(
                    a.downcast_ref::<niri::NiriWindow>().unwrap(),
                    b.downcast_ref::<niri::NiriWindow>().unwrap(),
                )
                .into()
            });
            let sorted_windows = gtk::SortListModel::new(Some(windows.clone()), Some(sorter));
            self.windows.replace(Some(windows));
            self.sorted_windows.replace(Some(sorted_windows));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AppImpl {
        const NAME: &'static str = "NeoDockAppInfo";
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
