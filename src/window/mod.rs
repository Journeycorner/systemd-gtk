mod imp;

use adw::glib::Propagation;
use glib::Object;
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gio, glib, Label, ListBoxRow, Switch};
use gtk::prelude::BoxExt;

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

        let units = systemctl::list_units_full(Some("service"), None, None).unwrap();
        for unit in units {
            let enabled = match unit.state.as_str() {
                "enabled" => true,
                _ => false
            };
            let mut switch = Switch::builder()
                .state(enabled)
                .build();

            let x = unit.unit_file.as_str();
            let label = &Label::new(Some(x));
            let bxx = gtk::Box::builder()
                .build();
            bxx.append(label);
            bxx.append(&switch);
            let item = ListBoxRow::builder()
                .child(&bxx)
                .build();
            list_box.append(&item);
            let switch_clone = switch.clone();
            switch.connect_state_set(move |_,target_state| {
                if !target_state {
                    systemctl::stop(&*unit.unit_file).expect("Could not stop");
                } else {
                    systemctl::start(&*unit.unit_file).expect("Could not start");
                }
                switch_clone.set_active(target_state);
                Propagation::Stop
            });
        }
    }
    // ANCHOR_END: setup_collections
}