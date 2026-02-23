use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use gtk4 as gtk;
use gtk4_layer_shell as layer_shell;
use layer_shell::{Edge, Layer, LayerShell};

glib::wrapper! {
    pub struct NeoWindow(ObjectSubclass<imp::NeoWindowImpl>)
        @extends gtk::Window, gtk::Widget,
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

    use declarative::{block, construct};
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk4 as gtk;

    use crate::prelude::*;
    use crate::utils::log;
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
                set_child: Some(&_) @gtk::Overlay view {
                    hexpand: true
                    child: &_ @widgets::DockView::new() {
                    }
                    ~

                    add_overlay: &_ @gtk::Label {
                        label: "Overlay"
                    }
                }

                add_css_class: "neodock-window"
            });

            self.view.replace(None);

            win.with_application(|win, app| {
                match app.downcast::<crate::NeoDockApp>() {
                    Ok(app) => win.imp().connect_niri(&app),
                    Err(app) => {
                        log::critical!("a NeoDockApp is required");
                        app.quit();
                    }
                };
            });
        }

        fn connect_niri(&self, app: &crate::NeoDockApp) {
            let niri = app.niri();
            niri.connect_window_created_notify(|niri| {
                // NOTE: debugging.
                let win = niri.window_created();
                log::message!("new window: {}, {}", win.id(), win.title().unwrap_or_default());
                win.connect_closed_notify(|win| {
                    log::message!("window closed: {}, {}", win.id(), win.title().unwrap_or_default());
                });
            });
            niri.connect_focused_window_notify(|niri| {
                // NOTE: debugging.
                if let Some(win) = niri.focused_window() {
                    log::message!("focused window: {}, {}", win.id(), win.title().unwrap_or_default());
                } else {
                    log::message!("unfocused window");
                }
            });
            niri.connect_overview_is_open_notify(|niri| {
                // NOTE: debugging.
                if niri.overview_is_open() {
                    log::message!("overview opened");
                } else {
                    log::message!("overview closed");
                }
            });
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeoWindowImpl {
        const NAME: &'static str = "NeoDockNeoWindow";
        type Type = Obj;
        type ParentType = gtk::Window;
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
}
