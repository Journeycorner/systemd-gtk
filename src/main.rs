mod systemd;
mod table;
mod window;

use crate::window::Window;
use adw::prelude::*;
use adw::{gio, glib, Application};

const APP_ID: &str = "com.journeycorner.systemd-gtk";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("systemd-gtk.gresource")
        .expect("Failed to register resources.");

    // Build application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect to signals
    app.connect_activate(build_ui);
    setup_shortcuts(&app);

    // Run application
    app.run()
}
fn build_ui(app: &adw::Application) {
    // Create new window and present it
    let window = Window::new(app);
    window.present();
}

fn setup_shortcuts(app: &Application) {
    app.set_accels_for_action("win.search_filter", &["<Ctrl>f"]);
    app.set_accels_for_action("win.show-help-overlay", &["<Ctrl>h"]);
    // TODO add esc for closing search
    // TODO add double click for opening detail view
}
