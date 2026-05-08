use glib::ExitCode;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;

use lib::constants::LOG_DOMAIN;
use lib::init;
use neodock as lib;

fn main() -> ExitCode {
    if let Err(err) = init::init() {
        glib::g_critical!(LOG_DOMAIN, "failed to initialize: {err}.");
        return ExitCode::FAILURE;
    }

    let app = lib::application::NeoDockApp::new();

    // Tries registering application and detects whether it is duplicate.
    if let Err(err) = app.register(None::<&gtk::gio::Cancellable>) {
        glib::g_critical!(LOG_DOMAIN, "failed to register: {err}.");
        return ExitCode::FAILURE;
    }
    if app.is_remote() {
        glib::g_message!(LOG_DOMAIN, "another instance is already running.");
    }

    app.run()
}
