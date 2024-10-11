mod imp;

use gtk::glib::Object;
use gtk::prelude::{ObjectExt, StaticType};
use gtk::glib;
use systemctl::State;

glib::wrapper! {
    pub struct UnitObject(ObjectSubclass<imp::UnitObject>);
}

impl UnitObject {
    pub fn new(unit: Result<systemctl::Unit, std::io::Error>) -> Self {
        if let Ok(u) = unit {
            Object::builder()
                .property("unit_file", u.name)
                .property("load", "loaded") // TODO
                .property("description", u.description)
                .build()
        } else {
            Object::builder()
                .property("unit_file", "not found")
                .property("load", "not found") // TODO
                .property("description", "not found")
                .build()
        }
    }
}

pub fn units() -> Vec<UnitObject> {
    let systemctl = systemctl::SystemCtl::default();
    systemctl.list_units_full(Some("service"), None, None).unwrap()
        .iter()
        .map(|unit| systemctl.create_unit(unit.unit_file.as_str()))
        .map(UnitObject::new)
        .collect::<Vec<UnitObject>>()
}