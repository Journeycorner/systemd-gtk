use gtk::glib;
use gtk::glib::Properties;
use std::cell::RefCell;
use gtk::subclass::prelude::{*};
use gtk::prelude::ObjectExt;

// Object holding the state
#[derive(Properties, Default)]
#[properties(wrapper_type = super::UnitObject)]
pub struct UnitObject {
    #[property(get, set)]
    unit_file: RefCell<String>,

    #[property(get, set)]
    state: RefCell<String>,
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