mod imp;

use gtk::glib;
use gtk::glib::Object;
use systemctl::UnitList;
use gtk::prelude::ObjectExt;

glib::wrapper! {
    pub struct UnitObject(ObjectSubclass<imp::UnitObject>);
}

impl UnitObject {
    pub fn new(unit: &UnitList) -> Self {
        Object::builder()
            .property("unit_file", &unit.unit_file)
            .property("state", &unit.state)
            .build()
    }
}

pub fn units() -> Vec<UnitObject> {
    systemctl::list_units_full(Some("service"), None, None).unwrap()
        .iter()
        .map(UnitObject::new)
        .collect::<Vec<UnitObject>>()
}