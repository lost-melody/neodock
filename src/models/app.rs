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

    /// Creates an [App] info for `app_id`.
    pub fn new_for_id(app_id: String) -> Self {
        let app = Self::new();
        app.set_app_id(app_id.clone());
        let gio_app_info = gio_unix::DesktopAppInfo::new(&format!("{app_id}.desktop"));
        app.set_info(gio_app_info);
        app
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
        self.imp().connect_window_updates(&window);
    }

    /// Removes the window by the given id, and returns how many windows remaining.
    pub fn remove_window(&self, id: u64) -> u32 {
        let windows = self.windows().unwrap();
        if let Some((index, _)) = self.imp().find_window(id) {
            windows.remove(index);
            self.imp().on_window_removed(id);
        }
        windows.n_items()
    }
}

/// Compares two [App]s by reversed `is_pinned` and then `app_id`.
pub fn compare_apps(a: &App, b: &App) -> Ordering {
    a.is_pinned()
        .cmp(&b.is_pinned())
        .reverse()
        .then(a.app_id().cmp(&b.app_id()))
}

/// Compares two [niri::NiriWindow]s by `app_id`, `output`, `workspace_idx`, `pos` and then `id`.
pub fn compare_windows(a: &niri::NiriWindow, b: &niri::NiriWindow) -> Ordering {
    a.app_id()
        .cmp(&b.app_id())
        .then(a.output().cmp(&b.output()))
        .then(a.workspace_idx().cmp(&b.workspace_idx()))
        .then(compare_windows_pos(a, b))
        .then(a.id().cmp(&b.id()))
}

/// Compares two [niri::NiriWindow]s by their `pos_in_scrolling_layout`s and `tile_pos_in_workspace_view`.
pub fn compare_windows_pos(a: &niri::NiriWindow, b: &niri::NiriWindow) -> Ordering {
    {
        let (pa, pb) = (
            a.get_layout().pos_in_scrolling_layout.unwrap_or_default(),
            b.get_layout().pos_in_scrolling_layout.unwrap_or_default(),
        );
        pa.cmp(&pb)
    }
    .then({
        let (pa, pb) = (
            a.get_layout().tile_pos_in_workspace_view.unwrap_or_default(),
            b.get_layout().tile_pos_in_workspace_view.unwrap_or_default(),
        );
        pa.partial_cmp(&pb).unwrap_or(Ordering::Equal)
    })
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};
    use gtk4 as gtk;

    use crate::services::niri;
    use crate::utils::signal;

    type Obj = super::App;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct AppImpl {
        /// Connected signals of windows mapped by window id,
        /// disconnected on windows removed.
        window_signals: RefCell<HashMap<u64, signal::Signals<niri::NiriWindow>>>,

        /// Source windows store, where windows should be inserted or removed.
        #[property(get)]
        windows: RefCell<Option<gio::ListStore>>,
        /// Sorted windows store, where windows should be retrieved.
        #[property(get)]
        sorted_windows: RefCell<Option<gtk::SortListModel>>,
        /// The application id of windows, and the filename of `.desktop` entries.
        #[property(get, set)]
        app_id: RefCell<String>,
        /// Whether this application is pinned to dock.
        #[property(get, set)]
        is_pinned: Cell<bool>,
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

        pub(super) fn connect_window_updates(&self, window: &niri::NiriWindow) {
            let mut signals = signal::Signals::new(window);
            use signal::AssignSignalsExt;
            window
                .connect_is_focused_notify(self.on_window_changed_callback())
                .assign_signals(&mut signals);
            // reorders windows on their workspace changed or updated.
            window
                .connect_output_notify(self.on_window_changed_callback())
                .assign_signals(&mut signals);
            window
                .connect_workspace_idx_notify(self.on_window_changed_callback())
                .assign_signals(&mut signals);
            // reorders windows when their layouts changed.
            window
                .connect_layout_notify(self.on_window_changed_callback())
                .assign_signals(&mut signals);
            self.window_signals.borrow_mut().insert(window.id(), signals);
        }

        pub(super) fn find_window(&self, id: u64) -> Option<(u32, niri::NiriWindow)> {
            let windows = self.obj().windows().unwrap();
            if let Some(index) =
                windows.find_with_equal_func(|o| o.downcast_ref::<niri::NiriWindow>().is_some_and(|w| w.id() == id))
                && let Some(window) = windows.item(index).and_downcast()
            {
                return Some((index, window));
            }
            None
        }

        pub(super) fn on_window_removed(&self, id: u64) {
            self.window_signals.borrow_mut().remove(&id);
        }

        fn on_window_changed_callback(&self) -> impl Fn(&niri::NiriWindow) + 'static {
            let obj = self.obj();
            glib::clone!(
                #[weak]
                obj,
                move |window| {
                    obj.imp().on_window_changed(window.id());
                }
            )
        }

        fn on_window_changed(&self, id: u64) {
            if let Some((index, _)) = self.find_window(id) {
                let windows = self.obj().windows().unwrap();
                // By marking window at `index` as dirty, triggers a sorting process.
                windows.items_changed(index, 1, 1);
            }
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
