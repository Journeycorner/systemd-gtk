use crate::systemd::unit::UnitObject;
use adw::gio::ListStore;
use adw::glib::subclass::InitializingObject;
use adw::subclass::prelude::*;
use adw::{glib, Dialog, HeaderBar, ToastOverlay};
use gtk::{ActionBar, Button, ColumnView, CompositeTemplate, SearchBar, SearchEntry, TextView};
use std::cell::RefCell;

// Object holding the state
#[derive(CompositeTemplate, Default)]
#[template(resource = "/com/journeycorner/systemd-gtk/window.xml")]
pub struct Window {
    #[template_child]
    pub overlay: TemplateChild<ToastOverlay>,

    #[template_child]
    pub column_view: TemplateChild<ColumnView>,

    #[template_child]
    pub search_bar: TemplateChild<SearchBar>,

    #[template_child]
    pub search_filter: TemplateChild<SearchEntry>,

    #[template_child]
    pub bottom_bar: TemplateChild<ActionBar>,

    #[template_child]
    pub dialog: TemplateChild<Dialog>,

    #[template_child]
    pub start_button: TemplateChild<Button>,

    #[template_child]
    pub restart_button: TemplateChild<Button>,

    #[template_child]
    pub stop_button: TemplateChild<Button>,

    #[template_child]
    pub enable_button: TemplateChild<Button>,

    #[template_child]
    pub disable_button: TemplateChild<Button>,

    #[template_child]
    pub view_unit_button: TemplateChild<Button>,

    #[template_child]
    pub text_view: TemplateChild<TextView>,

    #[template_child]
    pub file_header_bar: TemplateChild<HeaderBar>,

    pub list_store: RefCell<Option<ListStore>>,
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for Window {
    // `NAME` needs to match `class` attribute of template
    const NAME: &'static str = "MainWindow";
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
        self.list_store
            .replace(Some(ListStore::new::<UnitObject>()));
        let obj = self.obj();
        obj.setup_column_view();
        obj.setup_actions();
    }
}

// Trait shared by all widgets
impl WidgetImpl for Window {}

impl WindowImpl for Window {}

// Trait shared by all application windows
impl ApplicationWindowImpl for Window {}

// Trait shared by all adwaita application windows
impl AdwApplicationWindowImpl for Window {}
