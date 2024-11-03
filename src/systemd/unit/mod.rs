mod imp;

use adw::glib;
use adw::glib::Object;
use systemctl::UnitService;

glib::wrapper! {
    pub struct UnitObject(ObjectSubclass<imp::UnitObject>);
}

impl UnitObject {
    pub fn new(u: UnitService) -> Self {
        Object::builder()
            .property("unit_name", u.unit_name)
            .property("load", u.loaded)
            .property("state", u.state)
            .property("sub_state", u.sub_state)
            .property("description", u.description)
            .build()
    }
}
