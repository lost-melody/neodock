use std::cell::Ref;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

use super::ipc;

glib::wrapper! {
    pub struct NiriWindow(ObjectSubclass<imp::NiriWindowImpl>);
}

impl Default for NiriWindow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl<'w> NiriWindow {
    pub fn get_layout(&'w self) -> Ref<'w, ipc::WindowLayout> {
        Ref::map(self.imp().layout_.borrow(), |l| &l.0)
    }
}

impl NiriWindow {
    pub fn new(win: ipc::Window) -> Self {
        let obj = Self::default();
        obj.update(win);
        obj
    }

    pub fn update(&self, win: ipc::Window) {
        self.set_id(win.id);
        self.set_title(win.title);
        self.set_app_id(win.app_id);
        self.set_pid(win.pid.unwrap_or_default());
        self.set_workspace_id(win.workspace_id.unwrap_or_default());
        self.set_is_focused(win.is_focused);
        self.set_is_floating(win.is_floating);
        self.set_is_urgent(win.is_urgent);
        self.set_layout_(win.layout);
        self.set_focus_timestamp_(win.focus_timestamp);
    }

    pub fn set_layout_(&self, layout: ipc::WindowLayout) {
        {
            self.imp().layout_.borrow_mut().0 = layout;
        }
        self.set_layout(false);
    }

    pub fn set_focus_timestamp_(&self, timestamp: Option<ipc::Timestamp>) {
        if let Some(timestamp) = timestamp {
            self.imp().focus_timestamp_.borrow_mut().0 = timestamp;
        } else {
            *self.imp().focus_timestamp_.borrow_mut() = imp::Timestamp::default();
        }
        self.set_focus_timestamp(false);
    }

    pub fn get_focus_timestamp(&self) -> ipc::Timestamp {
        self.imp().focus_timestamp_.borrow().0
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    use crate::services::niri;
    use niri::ipc;

    type Obj = super::NiriWindow;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NiriWindowImpl {
        #[property(get, set)]
        id: Cell<u64>,
        #[property(get, set, nullable)]
        title: RefCell<Option<String>>,
        #[property(get, set, nullable)]
        app_id: RefCell<Option<String>>,
        #[property(get, set)]
        pid: Cell<i32>,
        #[property(get, set)]
        workspace_id: Cell<u64>,
        #[property(get, set)]
        is_focused: Cell<bool>,
        #[property(get, set)]
        is_floating: Cell<bool>,
        #[property(get, set)]
        is_urgent: Cell<bool>,
        /// Placeholder for property `layout` notifications.
        ///
        /// Acquire data by [super::NiriWindow::get_layout].
        #[property(get, set)]
        layout: Cell<bool>,
        pub(super) layout_: RefCell<WindowLayout>,
        /// Placeholder for property `focus_timestamp`.
        ///
        /// Acquire data by [super::NiriWindow::get_focus_timestamp].
        #[property(get, set)]
        focus_timestamp: Cell<bool>,
        pub(super) focus_timestamp_: RefCell<Timestamp>,
        #[property(get, set)]
        closed: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NiriWindowImpl {
        const NAME: &'static str = "NiriWindow";
        type Type = Obj;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NiriWindowImpl {}

    pub(super) struct WindowLayout(pub(super) ipc::WindowLayout);
    pub(super) struct Timestamp(pub(super) ipc::Timestamp);

    impl Default for WindowLayout {
        fn default() -> Self {
            Self(ipc::WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0f64, 0f64),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0f64, 0f64),
            })
        }
    }

    impl Default for Timestamp {
        fn default() -> Self {
            Self(ipc::Timestamp { secs: 0, nanos: 0 })
        }
    }
}
