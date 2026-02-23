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

    lib::application::NeoDockApp::default().run()
}
