use std::sync::LazyLock;

use gtk::glib;
use gtk4 as gtk;

pub const APP_ID: &str = "io.github.lost-melody.NeoDock";
pub const APP_DOMAIN: &str = "NeoDock";
pub const LOG_DOMAIN: &str = APP_DOMAIN;
pub const TEXT_DOMAIN: &str = "neodock";

pub const CONFIG_FILE: &str = "config.toml";
pub const STYLE_FILE: &str = "style.css";

pub static CONFIG_DIR: LazyLock<std::path::PathBuf> = LazyLock::new(|| glib::user_config_dir().join(APP_ID));
