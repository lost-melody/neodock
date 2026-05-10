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

    use crate::config;
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

        #[property(get)]
        config: RefCell<Option<config::NeoDockConfig>>,
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

            // initializes user config watcher.
            let config = config::NeoDockConfig::new();
            self.config.replace(Some(config));

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

            self.connect_pinned_apps();

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

        fn connect_pinned_apps(&self) {
            let obj = self.obj();
            let config = obj.config().unwrap();
            let apps = obj.apps().unwrap();
            // adds pinned apps into store.
            for app_id in config.pinned_apps() {
                let app_info = models::App::new_for_id(app_id);
                app_info.set_is_pinned(true);
                apps.append(&app_info);
            }
            config.connect_pinned_apps_notify(glib::clone!(
                #[weak]
                obj,
                move |config| {
                    obj.imp().update_pinned_apps(config);
                }
            ));
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

        fn update_pinned_apps(&self, config: &config::NeoDockConfig) {
            let obj = self.obj();
            let pinned = config.pinned_apps();
            let apps = obj.apps().unwrap();
            let mut to_remove = Vec::new();
            let mut already_pinned = Vec::new();
            // unpins apps that are not in the `pinned` list.
            for (index, item) in apps.into_iter().enumerate() {
                let Some(app_info) = item.ok().and_downcast::<models::App>() else {
                    continue;
                };
                let app_id = app_info.app_id();
                // unpinned apps.
                if app_info.is_pinned() {
                    if pinned.contains(&app_id) {
                        // collects already pinned apps.
                        already_pinned.push(app_info.app_id());
                    } else {
                        app_info.set_is_pinned(false);
                        // should be removed if no windows.
                        if app_info.windows().unwrap().n_items() == 0 {
                            to_remove.push(index as u32);
                        }
                    }
                }
            }
            // removes apps that are unpinned and have no windows (in reversed order).
            for index in to_remove.iter().rev() {
                apps.remove(*index);
            }
            // creates app info object for newly pinned apps.
            for app_id in pinned {
                if already_pinned.contains(&app_id) {
                    continue;
                }
                if let Some((_, app_info)) = self.find_app_info(&app_id) {
                    app_info.set_is_pinned(true);
                } else {
                    let app_info = models::App::new_for_id(app_id);
                    app_info.set_is_pinned(true);
                    self.add_app_to_store(&app_info);
                }
            }
        }

        /// Appends `app_info` into `apps`, and watches its `is_pinned` changes.
        fn add_app_to_store(&self, app_info: &models::App) {
            let apps = self.obj().apps().unwrap();
            apps.append(app_info);
            app_info.connect_is_pinned_notify(glib::clone!(
                #[weak]
                apps,
                move |app| {
                    if let Some(index) = apps.find(app) {
                        apps.items_changed(index, 1, 1);
                    }
                }
            ));
        }

        /// Finds app info by `app_id` and returns it with index.
        fn find_app_info(&self, app_id: &String) -> Option<(u32, models::App)> {
            let apps = self.obj().apps().unwrap();
            if let Some(index) =
                apps.find_with_equal_func(|o| o.downcast_ref::<models::App>().is_some_and(|a| &a.app_id() == app_id))
                && let Some(app_info) = apps.item(index).and_downcast::<models::App>()
            {
                return Some((index, app_info));
            }
            None
        }

        /// Adds window to ListStore, grouped by `app_id`.
        fn add_window_to_apps(&self, window: niri::NiriWindow) {
            let app_id = window.app_id().unwrap_or_default();
            // finds app in ListStore.
            if let Some((_, app_info)) = self.find_app_info(&app_id) {
                // adds the window to app info object.
                app_info.add_window(window);
            } else {
                // creates a new app info object.
                let app_info = models::App::new_for_id(window.app_id().unwrap_or_default());
                app_info.add_window(window);
                // and adds it to source store.
                self.add_app_to_store(&app_info);
            }
        }

        fn remove_window_from_apps(&self, window: &niri::NiriWindow) {
            let apps = self.obj().apps().unwrap();
            let app_id = window.app_id().unwrap_or_default();
            // finds app in ListStore, and removes window from it.
            if let Some((index, app_info)) = self.find_app_info(&app_id) {
                let remaining = app_info.remove_window(window.id());
                // removes app if not pinned and no windows remaining.
                if remaining == 0 && !app_info.is_pinned() {
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
            match file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, None::<&gio::Cancellable>) {
                Ok(monitor) => {
                    monitor.connect_changed(move |_, _, _, event| {
                        if event != gio::FileMonitorEvent::ChangesDoneHint {
                            return;
                        }
                        log::message!("user styles updated");
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
