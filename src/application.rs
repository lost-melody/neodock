use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use gtk4 as gtk;

glib::wrapper! {
    pub struct NeoDockApp(ObjectSubclass<imp::NeoDockAppImpl>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for NeoDockApp {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl NeoDockApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }
}

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::prelude::*;
    use gtk::{gdk, gio, glib};
    use gtk4 as gtk;

    use crate::constants;
    use crate::services::niri;
    use crate::utils::{gresource, log};
    use crate::widgets;

    type Obj = super::NeoDockApp;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoDockAppImpl {
        activated: Cell<bool>,
        user_css_monitor: RefCell<Option<gio::FileMonitor>>,

        #[property(get)]
        pub niri: RefCell<niri::Niri>,
    }

    impl NeoDockAppImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            let obj = self.obj();
            obj.set_application_id(Some(constants::APP_ID));
        }

        fn on_activate(&self) {
            if self.activated.get() {
                return;
            }
            self.activated.set(true);

            let Some(display) = gdk::Display::default() else {
                log::critical!("unable to retrieve gdk display!");
                return;
            };

            // loads application styles and user styles.
            self.load_styles(&display);
            self.load_user_styles(&display);

            let app = self.obj().clone();
            let niri = self.niri.borrow().clone();

            niri.spawn_event_stream({
                let app = app.clone();
                Some(async move |err| {
                    log::critical!("failed to sync event stream: {}", err);
                    app.quit();
                })
            });

            // creates docks for monitors.
            let monitors = display.monitors();
            for monitor in monitors.iter().flatten() {
                self.create_window(&monitor);
            }

            // creates docks on monitors connected.
            monitors.connect_items_changed(glib::clone!(
                #[weak]
                app,
                move |model, pos, _, added| {
                    for monitor in model.iter().skip(pos as usize).take(added as usize).flatten() {
                        app.imp().create_window(&monitor);
                    }
                }
            ));

            // a blank window prevents the application from quitting.
            gtk::Window::new().set_application(Some(&app));
        }

        fn create_window(&self, monitor: &gdk::Monitor) {
            let app = self.obj().clone();
            let window = widgets::NeoWindow::new(&app, monitor);

            // closes dock window on monitor invalidated.
            monitor.connect_invalidate(glib::clone!(
                #[weak]
                window,
                move |_| {
                    window.close();
                }
            ));

            window.set_output(monitor.connector());
            window.present();
        }

        fn load_styles(&self, display: &gdk::Display) {
            let provider = gtk::CssProvider::new();
            provider.load_from_resource(&gresource::resource_path("css/style.css"));
            gtk::style_context_add_provider_for_display(display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        fn load_user_styles(&self, display: &gdk::Display) {
            let css_path = constants::CONFIG_DIR.join("style.css");
            let provider = gtk::CssProvider::new();
            provider.load_from_path(&css_path);
            gtk::style_context_add_provider_for_display(display, &provider, gtk::STYLE_PROVIDER_PRIORITY_USER + 1);

            let file = gio::File::for_path(&css_path);
            match file.monitor_file(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>) {
                Ok(monitor) => {
                    monitor.connect_changed(move |_, _, _, _| {
                        provider.load_from_path(&css_path);
                    });
                    self.user_css_monitor.replace(Some(monitor));
                }
                Err(err) => {
                    log::warning!("unable to monitor user css file: {err}");
                }
            };
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeoDockAppImpl {
        const NAME: &'static str = "NeoDockApp";
        type Type = Obj;
        type ParentType = adw::Application;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NeoDockAppImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }

    impl GtkApplicationImpl for NeoDockAppImpl {}
    impl ApplicationImpl for NeoDockAppImpl {
        fn activate(&self) {
            self.parent_activate();
            self.on_activate();
        }
    }
    impl AdwApplicationImpl for NeoDockAppImpl {}
}
