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

    /// Binds to workspace properties, and unbinds from the old ones.
    pub fn bind_workspace(&self, workspace: &super::NiriWorkspace) {
        self.imp().bind_workspace(workspace);
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
            self.imp().focus_timestamp_.replace(imp::Timestamp::default());
        }
        self.set_focus_timestamp(false);
    }

    pub fn get_focus_timestamp(&self) -> ipc::Timestamp {
        self.imp().focus_timestamp_.borrow().0
    }

    pub fn cmd_focus(&self) -> Vec<ipc::Action> {
        vec![ipc::Action::FocusWindow { id: self.id() }]
    }

    pub fn cmd_center(&self) -> Vec<ipc::Action> {
        vec![
            ipc::Action::FocusWindow { id: self.id() },
            ipc::Action::CenterWindow { id: Some(self.id()) },
        ]
    }

    pub fn cmd_toggle_fullscreen(&self) -> Vec<ipc::Action> {
        vec![
            ipc::Action::FocusWindow { id: self.id() },
            ipc::Action::FullscreenWindow { id: Some(self.id()) },
        ]
    }

    pub fn cmd_toggle_maximize(&self) -> Vec<ipc::Action> {
        vec![
            ipc::Action::FocusWindow { id: self.id() },
            ipc::Action::MaximizeColumn {},
        ]
    }

    pub fn cmd_toggle_floating(&self) -> Vec<ipc::Action> {
        vec![
            ipc::Action::FocusWindow { id: self.id() },
            ipc::Action::ToggleWindowFloating { id: Some(self.id()) },
        ]
    }

    pub fn cmd_close(&self) -> Vec<ipc::Action> {
        vec![ipc::Action::CloseWindow { id: Some(self.id()) }]
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    use crate::services::niri;
    use crate::utils::signal;
    use niri::ipc;

    type Obj = super::NiriWindow;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NiriWindowImpl {
        signals: RefCell<Option<signal::Signals<niri::NiriWorkspace>>>,

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
        workspace_idx: Cell<u8>,
        #[property(get, set)]
        output: RefCell<String>,
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

    impl NiriWindowImpl {
        pub(super) fn bind_workspace(&self, workspace: &niri::NiriWorkspace) {
            let obj = self.obj();
            // updates properties.
            obj.set_workspace_idx(workspace.idx());
            obj.set_output(workspace.output().unwrap_or_default());
            let mut signals = signal::Signals::new(workspace);
            // connects to workspace changes and assigns to `signals`.
            use signal::AssignSignalsExt;
            workspace
                .connect_idx_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |ws| {
                        obj.set_workspace_idx(ws.idx());
                    }
                ))
                .assign_signals(&mut signals);
            workspace
                .connect_output_notify(glib::clone!(
                    #[weak]
                    obj,
                    move |ws| {
                        obj.set_output(ws.output().unwrap_or_default());
                    }
                ))
                .assign_signals(&mut signals);
            // stores the new, dropping the old.
            self.signals.replace(Some(signals));
        }
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
