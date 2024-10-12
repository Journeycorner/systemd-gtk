mod imp;

use adw::glib::Object;
use adw::glib;
use systemctl::State;

glib::wrapper! {
    pub struct UnitObject(ObjectSubclass<imp::UnitObject>);
}

impl UnitObject {
    pub fn new(unit: Result<systemctl::Unit, std::io::Error>) -> Self {
        if let Ok(u) = unit {
            Object::builder()
                .property("unit_file", u.name)
                .property("load", if u.state == State::Loaded { "loaded" } else { "masked" })
                .property("active", if u.active { "active" } else { "inactive" })
                .property("description", u.description)
                .build()
        } else {
            Object::builder()
                .property("unit_file", "not found")
                .property("load", "not found")
                .property("active", "not found")
                .property("description", "not found")
                .build()
        }
    }
}

pub fn units() -> Vec<UnitObject> {
    let systemctl = systemctl::SystemCtl::default();
    systemctl.list_units_full(Some("service"), None, None).unwrap()
        .iter()
        .take(20)
        .map(|unit| systemctl.create_unit(unit.unit_file.as_str()))
        .map(UnitObject::new)
        .collect::<Vec<UnitObject>>()
}