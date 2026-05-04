use std::env;

use gtk::{gdk, gio, glib};
use gtk4 as gtk;

use crate::constants::{CONFIG_DIR, TEXT_DOMAIN};
use crate::utils::gresource::resource_path;

pub fn init() -> anyhow::Result<()> {
    init_gettext().map_err(|e| anyhow::anyhow!("gettext: {e}"))?;
    init_gresources().map_err(|e| anyhow::anyhow!("gresources: {e}"))?;
    adw::init().map_err(|e| anyhow::anyhow!("gtk: {e}"))?;
    init_config_dir().map_err(|e| anyhow::anyhow!("create config dir: {e}"))?;
    init_icon_theme().map_err(|e| anyhow::anyhow!("icon theme: {e}"))?;

    Ok(())
}

fn init_gettext() -> anyhow::Result<()> {
    let dir = env::current_exe()?.parent().unwrap().join("locale");

    gettextrs::textdomain(TEXT_DOMAIN)?;
    gettextrs::bindtextdomain(TEXT_DOMAIN, dir)?;
    gettextrs::bind_textdomain_codeset(TEXT_DOMAIN, "UTF-8")?;

    Ok(())
}

fn init_gresources() -> anyhow::Result<()> {
    let gresource_bytes = include_bytes!("../target/resources.gresource");
    gio::resources_register(&gio::Resource::from_data(&glib::Bytes::from_static(gresource_bytes))?);

    Ok(())
}

fn init_icon_theme() -> anyhow::Result<()> {
    let display = gdk::Display::default().ok_or(anyhow::anyhow!("unable to retrieve gdk display"))?;
    gtk::IconTheme::for_display(&display).add_resource_path(&resource_path("icons"));

    Ok(())
}

fn init_config_dir() -> anyhow::Result<()> {
    std::fs::create_dir_all(&*CONFIG_DIR)?;

    for filename in ["config.toml", "style.css"] {
        let path = CONFIG_DIR.join(filename);
        if !path.exists() {
            std::fs::File::create(path)?;
        }
    }

    Ok(())
}
