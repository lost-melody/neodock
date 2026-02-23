use std::env;

use gtk::{gdk, gio, glib};
use gtk4 as gtk;

use crate::constants::TEXT_DOMAIN;
use crate::utils::gresource::resource_path;

pub fn init() -> anyhow::Result<()> {
    init_gettext().map_err(|e| anyhow::anyhow!("gettext: {e}"))?;
    init_gresources().map_err(|e| anyhow::anyhow!("gresources: {e}"))?;
    gtk::init().map_err(|e| anyhow::anyhow!("gtk: {e}"))?;
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
    let Some(display) = gdk::Display::default() else {
        return Err(anyhow::anyhow!("unable to retrieve gdk display"));
    };
    gtk::IconTheme::for_display(&display).add_resource_path(&resource_path("icons"));

    Ok(())
}
