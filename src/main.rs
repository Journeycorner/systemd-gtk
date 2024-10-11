mod window;
mod systemd;

use crate::window::Window;
use gtk::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "com.journeycorner.systemd-gtk";

fn main() -> glib::ExitCode {
    // Register and include resources
    gio::resources_register_include!("systemd-gtk.gresource")
        .expect("Failed to register resources.");

    // Build application
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();
    app.connect_activate(build_ui);
    // Run application
    app.run()
}

fn build_ui(app: &adw::Application) {
    // Create new window and present it
    let window = Window::new(app);
    window.present();
}