mod imp;

use glib::Object;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gio, glib, Label};

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        // Create new window
        Object::builder().property("application", app).build()
    }

    // ANCHOR: setup_collections
    fn setup_collections(&self) {
        // Create a `ListBox` and add labels with integers from 0 to 100
        let list_box = self.imp().collections_list.get();
        for number in 0..=100 {
            let label = Label::new(Some(&number.to_string()));
            list_box.append(&label);
        }
    }
    // ANCHOR_END: setup_collections
}