use adw::glib;
use adw::glib::Properties;
use std::cell::RefCell;
use adw::glib::Type;
use adw::subclass::prelude::{*};
use adw::prelude::{ObjectExt, StaticType};

// Object holding the state
#[derive(Properties, Default)]
#[properties(wrapper_type = super::UnitObject)]
pub struct UnitObject {
    #[property(get, set)]
    unit_file: RefCell<String>,

    #[property(get, set)]
    load: RefCell<Option<String>>,

    #[property(get, set)]
    active: RefCell<Option<String>>,

    #[property(get, set)]
    description: RefCell<Option<String>>,
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