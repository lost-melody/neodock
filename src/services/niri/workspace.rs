use super::ipc;
use gtk::glib;
use gtk4 as gtk;

glib::wrapper! {
    pub struct NiriWorkspace(ObjectSubclass<imp::NiriWorkspaceImpl>);
}

impl Default for NiriWorkspace {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl NiriWorkspace {
    pub fn new(ws: ipc::Workspace) -> Self {
        let obj = Self::default();
        obj.update(ws);
        obj
    }

    pub fn update(&self, ws: ipc::Workspace) {
        self.set_id(ws.id);
        self.set_idx(ws.idx);
        self.set_name(ws.name);
        self.set_output(ws.output);
        self.set_is_urgent(ws.is_urgent);
        self.set_is_active(ws.is_active);
        self.set_is_focused(ws.is_focused);
        self.set_active_window_id(ws.active_window_id.unwrap_or_default());
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    type Obj = super::NiriWorkspace;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NiriWorkspaceImpl {
        #[property(get, set)]
        id: Cell<u64>,
        #[property(get, set)]
        idx: Cell<u8>,
        #[property(get, set, nullable)]
        name: RefCell<Option<String>>,
        #[property(get, set, nullable)]
        output: RefCell<Option<String>>,
        #[property(get, set)]
        is_urgent: Cell<bool>,
        #[property(get, set)]
        is_active: Cell<bool>,
        #[property(get, set)]
        is_focused: Cell<bool>,
        #[property(get, set)]
        active_window_id: Cell<u64>,
        #[property(get, set)]
        removed: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NiriWorkspaceImpl {
        const NAME: &'static str = "NiriWorkspace";
        type Type = Obj;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NiriWorkspaceImpl {}
}
