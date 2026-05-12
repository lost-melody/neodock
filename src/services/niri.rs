use std::cell::Ref;
use std::collections::HashMap;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

pub use socket::Socket;
pub use window::NiriWindow;
pub use workspace::NiriWorkspace;

pub mod ipc;
pub mod socket;
pub mod window;
pub mod workspace;

glib::wrapper! {
    pub struct Niri(ObjectSubclass<imp::NiriWindowImpl>);
}

impl Default for Niri {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl<'n> Niri {
    pub fn get_windows(&'n self) -> Ref<'n, HashMap<u64, window::NiriWindow>> {
        self.imp().windows_.borrow()
    }

    pub fn get_workspaces(&'n self) -> Ref<'n, HashMap<u64, workspace::NiriWorkspace>> {
        self.imp().workspaces_.borrow()
    }
}

impl Niri {
    /// Requests for event stream and sync events to niri state.
    /// Calls `f(err)` on errors occurring.
    pub fn spawn_event_stream<F: AsyncFnOnce(anyhow::Error) + 'static>(&self, f: Option<F>) {
        glib::spawn_future_local({
            let niri = self.clone();
            async move {
                if let Err(err) = niri.start_event_stream().await
                    && let Some(f) = f
                {
                    f(err).await;
                }
            }
        });
    }

    /// Requests for event stream and sync events to niri state.
    pub async fn start_event_stream(&self) -> anyhow::Result<()> {
        let socket = Socket::default();
        match socket.send(ipc::Request::EventStream).await? {
            Ok(_) => {
                let mut read_event = socket.read_events().await;
                loop {
                    if let Some(event) = read_event().await? {
                        self.apply(event);
                    }
                }
            }
            Err(err) => Err(anyhow::anyhow!("failed to start event stream: {}", err)),
        }
    }

    /// Applies the [ipc::Event] to niri state.
    pub fn apply(&self, event: ipc::Event) {
        self.imp().apply(event);
    }

    /// Requests an [ipc::Action] and ignores the reply.
    pub fn do_action(&self, action: ipc::Action) {
        glib::spawn_future_local({
            let niri = self.clone();
            async move {
                let _ = niri.send_action(action).await;
            }
        });
    }

    /// Requests an [ipc::Action].
    pub async fn send_action(&self, action: ipc::Action) -> anyhow::Result<ipc::Reply> {
        let request = ipc::Request::Action(action);
        self.send_request(request).await
    }

    /// Sends a [ipc::Request] and returns a [ipc::Reply].
    pub async fn send_request(&self, request: ipc::Request) -> anyhow::Result<ipc::Reply> {
        self.imp().socket.send(request).await
    }
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::{HashMap, HashSet};
    use std::rc::Rc;

    use super::ipc;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    use crate::services::niri;
    use niri::{NiriWindow, NiriWorkspace, Socket};

    type Obj = super::Niri;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NiriWindowImpl {
        pub(super) socket: Rc<Socket>,
        /// Placeholder for property `workspaces` notifications.
        ///
        /// This is notified only when workspaces created or closed.
        ///
        /// Acquire data by [super::Niri::get_workspaces].
        #[property(get, set)]
        workspaces: Cell<bool>,
        pub(super) workspaces_: RefCell<HashMap<u64, NiriWorkspace>>,
        /// Placeholder for property `windows` notifications.
        ///
        /// This is notified only when windows created or closed.
        ///
        /// Acquire data by [super::Niri::get_windows].
        #[property(get, set)]
        windows: Cell<bool>,
        pub(super) windows_: RefCell<HashMap<u64, NiriWindow>>,
        #[property(get, set)]
        overview_is_open: Cell<bool>,
        #[property(get, set)]
        window_created: RefCell<NiriWindow>,
        #[property(get, set)]
        focused_workspace: RefCell<Option<NiriWorkspace>>,
        #[property(get, set, nullable)]
        focused_window: RefCell<Option<NiriWindow>>,
    }

    impl NiriWindowImpl {
        pub(super) fn apply(&self, event: ipc::Event) {
            use ipc::Event;

            match event {
                // workspaces.
                Event::WorkspacesChanged { workspaces } => self.apply_workspaces_changed(workspaces),
                Event::WorkspaceUrgencyChanged { id, urgent } => {
                    if let Some(workspace) = self.workspaces_.borrow().get(&id) {
                        workspace.set_is_urgent(urgent);
                    }
                }
                Event::WorkspaceActivated { id, focused } => {
                    if let Some(workspace) = self.workspaces_.borrow().get(&id) {
                        for ws in self.workspaces_.borrow().values() {
                            // active workspace changed on the same output.
                            if ws.output() == workspace.output() {
                                if ws.is_active() && ws.id() != id {
                                    ws.set_is_active(false);
                                } else if !ws.is_active() && ws.id() == id {
                                    ws.set_is_active(true);
                                }
                            }
                            // focused workspace changed.
                            if focused {
                                if ws.is_focused() && ws.id() != id {
                                    ws.set_is_focused(false);
                                } else if !ws.is_focused() && ws.id() == id {
                                    ws.set_is_focused(true);
                                    self.obj().set_focused_workspace(ws);
                                }
                            }
                        }
                    }
                }
                Event::WorkspaceActiveWindowChanged {
                    workspace_id,
                    active_window_id,
                } => {
                    if let Some(workspace) = self.workspaces_.borrow().get(&workspace_id) {
                        workspace.set_active_window_id(active_window_id.unwrap_or_default());
                    }
                }
                // windows.
                Event::WindowsChanged { windows } => self.apply_windows_changed(windows),
                Event::WindowOpenedOrChanged { window } => self.apply_window_opened_or_changed(window),
                Event::WindowClosed { id } => {
                    if let Some(win) = self.windows_.borrow_mut().remove(&id) {
                        win.set_closed(true);
                    }
                    self.obj().notify_windows();
                }
                Event::WindowFocusChanged { id } => self.apply_window_focus_changed(id),
                Event::WindowFocusTimestampChanged { id, focus_timestamp } => {
                    if let Some(win) = self.windows_.borrow().get(&id) {
                        win.set_focus_timestamp_(focus_timestamp);
                    }
                }
                Event::WindowUrgencyChanged { id, urgent } => {
                    if let Some(win) = self.windows_.borrow().get(&id) {
                        win.set_is_urgent(urgent);
                    }
                }
                Event::WindowLayoutsChanged { changes } => {
                    for (id, layout) in changes {
                        if let Some(win) = self.windows_.borrow().get(&id) {
                            win.set_layout_(layout);
                        }
                    }
                }
                // overview.
                Event::OverviewOpenedOrClosed { is_open } => {
                    self.obj().set_overview_is_open(is_open);
                }
                // ignored.
                _ => (),
            }
        }

        fn apply_workspaces_changed(&self, workspaces: Vec<ipc::Workspace>) {
            {
                // removed.
                let id_set: HashSet<_> = workspaces.iter().map(|ws| ws.id).collect();
                self.workspaces_.borrow_mut().retain(|id, _| id_set.contains(id));
            }
            for ws in workspaces {
                if let Some(w) = self.workspaces_.borrow().get(&ws.id) {
                    // updated.
                    w.update(ws);
                } else {
                    // created.
                    let ws = NiriWorkspace::new(ws);
                    self.workspaces_.borrow_mut().insert(ws.id(), ws);
                }
            }
            self.obj().notify_workspaces();
        }

        fn apply_windows_changed(&self, windows: Vec<ipc::Window>) {
            for win in windows {
                self.handle_window_opened_or_changed(win);
            }
            self.obj().notify_windows();
        }

        fn apply_window_opened_or_changed(&self, window: ipc::Window) {
            if self.handle_window_opened_or_changed(window) {
                self.obj().notify_windows();
            }
        }

        fn apply_window_focus_changed(&self, id: Option<u64>) {
            let id = id.unwrap_or_default();
            if let Some(win) = self.focused_window.borrow().as_ref() {
                win.set_is_focused(false);
            }
            self.obj()
                .set_focused_window(self.windows_.borrow().get(&id).inspect(|w| w.set_is_focused(true)));
        }

        /// Handles window created or changed,
        /// and returns whether a new window is created.
        fn handle_window_opened_or_changed(&self, window: ipc::Window) -> bool {
            let is_opened = false;
            let win = if let Some(win) = self.windows_.borrow().get(&window.id).cloned() {
                // changed.
                win.update(window);
                win
            } else {
                // opened.
                let win = NiriWindow::new(window);
                self.connect_window_workspace_id(&win);
                self.windows_.borrow_mut().insert(win.id(), win.clone());
                self.obj().set_window_created(&win);
                win
            };
            if win.is_focused() {
                self.apply_window_focus_changed(Some(win.id()));
            }
            is_opened
        }

        fn connect_window_workspace_id(&self, window: &NiriWindow) {
            let obj = self.obj();
            window.connect_workspace_id_notify(glib::clone!(
                #[weak]
                obj,
                move |win| {
                    obj.imp().on_window_workspace_id_changed(win);
                }
            ));
            self.on_window_workspace_id_changed(window);
        }

        fn on_window_workspace_id_changed(&self, window: &NiriWindow) {
            if let Some(ws) = self.workspaces_.borrow().get(&window.workspace_id()) {
                window.bind_workspace(ws);
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NiriWindowImpl {
        const NAME: &'static str = "NiriService";
        type Type = Obj;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NiriWindowImpl {}
}
