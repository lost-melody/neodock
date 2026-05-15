use std::cell::Ref;
use std::collections::HashMap;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk4 as gtk;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Config {
    #[serde(default = "Config::default_auto_hide")]
    pub auto_hide: bool,
    #[serde(default = "Config::default_auto_hide_delay")]
    pub auto_hide_delay: u64,
    #[serde(default = "Config::default_show_in_overview")]
    pub show_in_overview: bool,
    #[serde(default)]
    pub dock_layer: DockLayer,
    #[serde(default)]
    pub filter_windows: WindowsFilter,
    #[serde(default = "Config::default_launcher_command")]
    pub launcher_command: Vec<String>,
    #[serde(default)]
    pub pinned_apps: Vec<String>,
    #[serde(default)]
    pub app_id_substitution: HashMap<String, String>,
}

impl Config {
    fn default_auto_hide() -> bool {
        true
    }
    fn default_auto_hide_delay() -> u64 {
        800
    }
    fn default_show_in_overview() -> bool {
        true
    }
    fn default_launcher_command() -> Vec<String> {
        ["qs", "-c", "noctalia-shell", "ipc", "call", "launcher", "toggle"]
            .iter()
            .map(|&s| s.into())
            .collect()
    }
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Serialize)]
pub enum DockLayer {
    #[serde(alias = "bottom")]
    Bottom,
    #[default]
    #[serde(alias = "top")]
    Top,
    #[serde(alias = "overlay")]
    Overlay,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Serialize)]
pub enum WindowsFilter {
    #[serde(alias = "all")]
    All,
    #[default]
    #[serde(alias = "same_output", alias = "output")]
    SameOutput,
    #[serde(alias = "same_workspace", alias = "workspace")]
    SameWorkspace,
}

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

    pub fn get_dock_layer(&self) -> DockLayer {
        self.imp().dock_layer_.get()
    }

    pub fn get_filter_windows(&self) -> WindowsFilter {
        self.imp().filter_windows_.get()
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

    use crate::constants::{CONFIG_DIR, CONFIG_FILE};
    use crate::utils::log;

    type Obj = super::NeoDockConfig;

    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = Obj)]
    pub struct NeoDockConfigImpl {
        monitor: RefCell<Option<gio::FileMonitor>>,

        /// Whether dock should hide automatically after a delay.
        /// Exclusive zone is enabled when `auto_hide` is off.
        #[property(get)]
        auto_hide: Cell<bool>,
        /// Delay before auto hiding in milliseconds.
        #[property(get)]
        auto_hide_delay: Cell<u64>,
        /// Whether dock should always display in overview even when auto hide is on.
        #[property(get)]
        show_in_overview: Cell<bool>,
        /// Placeholder for `dock_layer` notifications.
        ///
        /// In which layer dock window should display.
        #[property(get)]
        dock_layer: Cell<bool>,
        pub(super) dock_layer_: Cell<super::DockLayer>,
        /// Command to run on launcher button clicked.
        #[property(get)]
        launcher_command: RefCell<Vec<String>>,
        /// Placeholder for `windows_filter` notifications.
        ///
        /// Filters app icons and windows by their `output`s and `workspace`s.
        #[property(get)]
        filter_windows: Cell<bool>,
        pub(super) filter_windows_: Cell<super::WindowsFilter>,
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
            let mut config = match toml::from_slice::<super::Config>(&data) {
                Ok(config) => config,
                Err(err) => {
                    log::warning!("failed to parse config data: {err}");
                    return;
                }
            };

            if self.auto_hide.get() != config.auto_hide {
                self.auto_hide.set(config.auto_hide);
                self.obj().notify_auto_hide();
            }

            if self.auto_hide_delay.get() != config.auto_hide_delay {
                self.auto_hide_delay.set(config.auto_hide_delay);
                self.obj().notify_auto_hide_delay();
            }

            if self.show_in_overview.get() != config.show_in_overview {
                self.show_in_overview.set(config.show_in_overview);
                self.obj().notify_show_in_overview();
            }

            if self.dock_layer_.get() != config.dock_layer {
                self.dock_layer_.set(config.dock_layer);
                self.obj().notify_dock_layer();
            }

            if self.filter_windows_.get() != config.filter_windows {
                self.filter_windows_.set(config.filter_windows);
                self.obj().notify_filter_windows();
            }

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
