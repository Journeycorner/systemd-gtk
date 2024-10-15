use adw::{glib, SplitButton};
use adw::glib::subclass::InitializingObject;
use adw::subclass::prelude::*;
use gtk::{ColumnView, CompositeTemplate, SearchEntry};

// Object holding the state
#[derive(CompositeTemplate, Default)]
#[template(resource = "/com/journeycorner/systemd-gtk/window.xml")]
pub struct Window {
    #[template_child]
    pub collections_list: TemplateChild<ColumnView>,

    #[template_child]
    pub search_filter: TemplateChild<SearchEntry>,

    #[template_child]
    pub action_button: TemplateChild<SplitButton>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for Window {
    // `NAME` needs to match `class` attribute of template
    const NAME: &'static str = "MyGtkAppWindow";
    type Type = super::Window;
    type ParentType = adw::ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

// Trait shared by all GObjects
impl ObjectImpl for Window {
    fn constructed(&self) {
        // Call "constructed" on parent
        self.parent_constructed();

        // Setup
        let obj = self.obj();
        obj.setup_collections();
    }
}

// Trait shared by all widgets
impl WidgetImpl for Window {}

impl WindowImpl for Window {}

// Trait shared by all application windows
impl ApplicationWindowImpl for Window {}

// ANCHOR: adw_application_window_impl
// Trait shared by all adwaita application windows
impl AdwApplicationWindowImpl for Window {}
// ANCHOR_END: adw_application_window_impl

