use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use gtk4 as gtk;
use gtk4_layer_shell as layer_shell;
use layer_shell::{Edge, Layer, LayerShell};

glib::wrapper! {
    pub struct NeoWindow(ObjectSubclass<imp::NeoWindowImpl>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Default for NeoWindow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl NeoWindow {
    pub fn new(application: &impl IsA<gtk::Application>, monitor: &gdk::Monitor) -> Self {
        let window = Self::default();

        // application.
        window.set_application(Some(application));
        // layer shell.
        window.init_layer_shell();
        window.set_namespace(Some("neodock"));
        window.set_monitor(Some(monitor));
        window.set_layer(Layer::Top);
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 0);

        window
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }
}

mod imp {
    use std::cell::RefCell;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use declarative::block;
    use gtk::glib;
    use gtk4 as gtk;

    use crate::widgets;

    type Obj = super::NeoWindow;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoWindowImpl {
        #[property(get)]
        view: RefCell<Option<String>>,
    }

    impl NeoWindowImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let win = self.obj();

            block!(win.clone() {
                set_width_request: -1
                set_height_request: -1
                set_resizable: false
                add_css_class: "neodock-window"
                set_content: Some(&_) @widgets::DockView::new() {}
            });

            self.view.replace(None);
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeoWindowImpl {
        const NAME: &'static str = "NeoDockNeoWindow";
        type Type = Obj;
        type ParentType = adw::Window;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NeoWindowImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }

    impl WidgetImpl for NeoWindowImpl {}
    impl WindowImpl for NeoWindowImpl {}
    impl AdwWindowImpl for NeoWindowImpl {}
}
