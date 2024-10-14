use adw::glib;
use adw::glib::Properties;
use adw::glib::Type;
use adw::prelude::{ObjectExt, StaticType};
use adw::subclass::prelude::*;
use std::sync::Mutex;

// Object holding the state
#[derive(Properties, Default)]
#[properties(wrapper_type = super::UnitObject)]
pub struct UnitObject {
    #[property(get, set)]
    unit_file: Mutex<String>,

    #[property(get, set)]
    load: Mutex<Option<String>>,

    #[property(get, set)]
    active: Mutex<Option<String>>,

    #[property(get, set)]
    description: Mutex<Option<String>>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for UnitObject {
    const NAME: &'static str = "MyGtkAppUnitObject";
    type Type = super::UnitObject;
}

// Trait shared by all GObjects
#[glib::derived_properties]
impl ObjectImpl for UnitObject {}

impl StaticType for UnitObject {
    fn static_type() -> Type {
        // This ensures UnitObject is properly registered with the GObject type system
        Self::static_type()
    }
}