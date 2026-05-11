use std::cell::Ref;
use std::collections::HashMap;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;

glib::wrapper! {
    pub struct NeoDockConfig(ObjectSubclass<imp::NeoDockConfigImpl>);
}

impl Default for NeoDockConfig {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}

impl NeoDockConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn destroy(&self) {
        self.imp().destroy();
    }

    pub fn get_app_id_substitution<'c>(&'c self) -> Ref<'c, HashMap<String, String>> {
        self.imp().app_id_substitution_.borrow()
    }

    pub fn get_substituted(&self, app_id: String) -> String {
        self.get_app_id_substitution().get(&app_id).cloned().unwrap_or(app_id)
    }
}

mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::fs;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gio, glib};
    use gtk4 as gtk;
    use serde::Deserialize;

    use crate::constants::{CONFIG_DIR, CONFIG_FILE};
    use crate::utils::log;

    type Obj = super::NeoDockConfig;

    #[derive(Deserialize)]
    struct Config {
        #[serde(default = "Config::default_launcher_command")]
        launcher_command: Vec<String>,
        #[serde(default)]
        pinned_apps: Vec<String>,
        #[serde(default)]
        app_id_substitution: HashMap<String, String>,
    }

    impl Config {
        fn default_launcher_command() -> Vec<String> {
            ["qs", "-c", "noctalia-shell", "ipc", "call", "launcher", "toggle"]
                .iter()
                .map(|&s| s.into())
                .collect()
        }
    }

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoDockConfigImpl {
        monitor: RefCell<Option<gio::FileMonitor>>,

        /// Command to run on launcher button clicked.
        #[property(get)]
        launcher_command: RefCell<Vec<String>>,
        /// Pinned applications.
        #[property(get)]
        pinned_apps: RefCell<Vec<String>>,
        /// Placeholder for `app_id_substitution` notifications.
        #[property(get)]
        app_id_substitution: Cell<bool>,
        /// `app_id` substitution dictionary.
        pub(super) app_id_substitution_: RefCell<HashMap<String, String>>,
    }

    impl NeoDockConfigImpl {
        pub(super) fn destroy(&self) {}

        fn on_constructed(&self) {
            self.reload();
            self.monitor_config();
        }

        /// Monitors user config file changes.
        fn monitor_config(&self) {
            let file = gio::File::for_path(CONFIG_DIR.join(CONFIG_FILE));
            let monitor = match file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, None::<&gio::Cancellable>) {
                Ok(file) => file,
                Err(err) => {
                    log::warning!("unable to monitor user config file: {err}");
                    return;
                }
            };
            // reloads config on changed.
            let obj = self.obj();
            monitor.connect_changed(glib::clone!(
                #[weak]
                obj,
                move |_, _, _, event| {
                    if event != gio::FileMonitorEvent::ChangesDoneHint {
                        return;
                    }
                    log::message!("user config updated");
                    obj.imp().reload();
                }
            ));
            // holds a reference so that it does not get dropped.
            self.monitor.replace(Some(monitor));
        }

        /// Reloads configurations from file.
        fn reload(&self) {
            let data = match fs::read(CONFIG_DIR.join(CONFIG_FILE)) {
                Ok(data) => data,
                Err(err) => {
                    log::warning!("failed to read config file: {err}");
                    return;
                }
            };
            let mut config = match toml::from_slice::<Config>(&data) {
                Ok(config) => config,
                Err(err) => {
                    log::warning!("failed to parse config data: {err}");
                    return;
                }
            };

            if *self.launcher_command.borrow() != config.launcher_command {
                self.launcher_command.replace(config.launcher_command);
                self.obj().notify_launcher_command();
            }

            config.pinned_apps.sort();
            if *self.pinned_apps.borrow() != config.pinned_apps {
                self.pinned_apps.replace(config.pinned_apps);
                self.obj().notify_pinned_apps();
            }

            if *self.app_id_substitution_.borrow() != config.app_id_substitution {
                self.app_id_substitution_.replace(config.app_id_substitution);
                self.obj().notify_app_id_substitution();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NeoDockConfigImpl {
        const NAME: &'static str = "NeoDockConfig";
        type Type = Obj;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for NeoDockConfigImpl {
        fn constructed(&self) {
            self.parent_constructed();
            self.on_constructed();
        }
    }
}
