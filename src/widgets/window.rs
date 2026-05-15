use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use gtk4 as gtk;
use gtk4_layer_shell as layer_shell;
use layer_shell::{Edge, LayerShell};

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
        window.set_anchor(Edge::Bottom, true);
        // binds monitor's connector to dock view's output.
        let dock_view = window.view().unwrap();
        monitor
            .bind_property("connector", &dock_view, "output")
            .transform_to(|_, output: Option<String>| Some(output.unwrap_or_default()))
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

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
    use gtk4_layer_shell as layer_shell;
    use layer_shell::{Layer, LayerShell};

    use crate::config;
    use crate::prelude::*;
    use crate::widgets;

    type Obj = super::NeoWindow;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoWindowImpl {
        config: RefCell<Option<config::NeoDockConfig>>,

        #[property(get)]
        view: RefCell<Option<widgets::DockView>>,
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
                set_content: Some(&_) @widgets::DockView::new() view {}
            });

            self.view.replace(Some(view));

            self.bind_application();
        }

        fn bind_application(&self) {
            self.obj().with_neo_app(|win, app| {
                let config = app.config().unwrap();
                win.imp().connect_config(&config);
                win.imp().config.replace(Some(config));
            });
        }

        fn connect_config(&self, config: &config::NeoDockConfig) {
            let obj = self.obj();
            config.connect_auto_hide_notify(glib::clone!(
                #[weak]
                obj,
                move |config| {
                    if config.auto_hide() {
                        obj.set_exclusive_zone(0);
                    } else {
                        obj.auto_exclusive_zone_enable();
                    }
                }
            ));
            config.connect_dock_layer_notify(glib::clone!(
                #[weak]
                obj,
                move |config| {
                    match config.get_dock_layer() {
                        config::DockLayer::Bottom => {
                            obj.set_layer(Layer::Bottom);
                        }
                        config::DockLayer::Top => {
                            obj.set_layer(Layer::Top);
                        }
                        config::DockLayer::Overlay => {
                            obj.set_layer(Layer::Overlay);
                        }
                    }
                }
            ));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeoWindowImpl {
        const NAME: &'static str = "NeoDockWindow";
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
