use adw::glib;
use adw::glib::Properties;
use adw::prelude::ObjectExt;
use adw::subclass::prelude::*;
use std::sync::Mutex;

// Object holding the state
#[derive(Properties, Default)]
#[properties(wrapper_type = super::UnitObject)]
pub struct UnitObject {
    #[property(get, set)]
    unit_name: Mutex<String>,

    #[property(get, set)]
    load: Mutex<String>,

    #[property(get, set)]
    state: Mutex<String>,

    #[property(get, set)]
    sub_state: Mutex<String>,

    #[property(get, set)]
    description: Mutex<String>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for UnitObject {
    const NAME: &'static str = "UnitObject";
    type Type = super::UnitObject;
}

// Trait shared by all GObjects
#[glib::derived_properties]
impl ObjectImpl for UnitObject {}
