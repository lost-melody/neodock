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
    use gio_unix;
    use gtk::prelude::*;
    use gtk::{gdk, gio, glib};
    use gtk4 as gtk;

    use crate::constants;
    use crate::models;
    use crate::services::niri;
    use crate::utils::{gresource, log};
    use crate::widgets;

    type Obj = super::NeoDockApp;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoDockAppImpl {
        activated: Cell<bool>,
        user_css_monitor: RefCell<Option<gio::FileMonitor>>,

        /// Source applications store, where apps are inserted and removed.
        #[property(get)]
        apps: RefCell<Option<gio::ListStore>>,
        /// Sorted applications store, where apps are retrieved.
        #[property(get)]
        sorted_apps: RefCell<Option<gtk::SortListModel>>,
        #[property(get)]
        niri: RefCell<niri::Niri>,
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

            // initializes apps store and sorted store.
            let apps = gio::ListStore::new::<models::App>();
            let sorter = gtk::CustomSorter::new(|a, b| {
                models::app::compare_apps(
                    a.downcast_ref::<models::App>().unwrap(),
                    b.downcast_ref::<models::App>().unwrap(),
                )
                .into()
            });
            let sorted_apps = gtk::SortListModel::new(Some(apps.clone()), Some(sorter));
            self.apps.replace(Some(apps));
            self.sorted_apps.replace(Some(sorted_apps));

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
            self.connect_niri_signals(&niri);

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

            window.present();
        }

        fn connect_niri_signals(&self, niri: &niri::Niri) {
            let app = self.obj().clone();
            // adds windows to store on created, and removes it on closed.
            niri.connect_window_created_notify(glib::clone!(
                #[weak]
                app,
                move |niri| {
                    let window = niri.window_created();
                    window.connect_closed_notify(glib::clone!(
                        #[weak]
                        app,
                        move |w| {
                            app.imp().remove_window_from_apps(w);
                        }
                    ));
                    app.imp().add_window_to_apps(window);
                }
            ));
        }

        /// Adds window to ListStore, grouped by `app_id`.
        fn add_window_to_apps(&self, window: niri::NiriWindow) {
            let apps = self.obj().apps().unwrap();
            let app_id = window.app_id().unwrap_or_default();
            // finds app in ListStore.
            if let Some(index) =
                apps.find_with_equal_func(|o| o.downcast_ref::<models::App>().is_some_and(|a| a.app_id() == app_id))
            {
                // adds the window to app info object.
                let app_info = apps.item(index).and_then(|o| o.downcast::<models::App>().ok()).unwrap();
                app_info.add_window(window);
            } else {
                // creates a new app info object.
                let app_info = models::App::new();
                let app_id = window.app_id().unwrap_or_default();
                let gio_app_info = gio_unix::DesktopAppInfo::new(&format!("{app_id}.desktop"));
                app_info.set_app_id(app_id);
                app_info.set_info(gio_app_info);
                app_info.add_window(window);
                // and adds it to source store.
                apps.append(&app_info);
            }
        }

        fn remove_window_from_apps(&self, window: &niri::NiriWindow) {
            let apps = self.obj().apps().unwrap();
            let app_id = window.app_id().unwrap_or_default();
            // finds app in ListStore, and removes window from it.
            if let Some(index) =
                apps.find_with_equal_func(|o| o.downcast_ref::<models::App>().is_some_and(|a| a.app_id() == app_id))
            {
                let app_info = apps.item(index).and_then(|o| o.downcast::<models::App>().ok()).unwrap();
                let remaining = app_info.remove_window(window.id());
                // removes app if no windows remaining.
                if remaining == 0 {
                    apps.remove(index);
                }
            }
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
