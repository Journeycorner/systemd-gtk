mod imp;

use glib::Object;
use gtk::glib::clone;
use gtk::glib::property::PropertyGet;
use gtk::prelude::{BoxExt, Cast, CastNone, GtkWindowExt, ListBoxRowExt, ListItemExt, WidgetExt};
use gtk::subclass::prelude::ObjectSubclassIsExt;
use gtk::{gio, glib, Label, ListBoxRow, ListItem, SignalListItemFactory, SingleSelection, Switch};
use systemctl::UnitList;

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

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        // Create new window
        Object::builder().property("application", app).build()
    }

    // ANCHOR: setup_collections
    fn setup_collections(&self) {
        // Create a `ListBox` and add labels with integers from 0 to 100
        let list_box = self.imp().collections_list.get();

        let units = systemctl::list_units_full(Some("service"), None, None).unwrap()
            .iter()
            .map(UnitObject::new)
            .collect::<Vec<UnitObject>>();

        // Create new model
        let model = gio::ListStore::new::<UnitObject>();

        // Add the vector to the model
        model.extend_from_slice(&units);

        let factory = SignalListItemFactory::new();
        factory.connect_setup(move |_, list_item| {
            let label = Label::new(None);
            list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&label));
        });

        factory.connect_bind(move |_, list_item| {
            // Get `IntegerObject` from `ListItem`
            let integer_object = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<UnitObject>()
                .expect("The item has to be an `IntegerObject`.");

            // Get `Label` from `ListItem`
            let label = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<Label>()
                .expect("The child has to be a `Label`.");

            // Set "label" to "number"
            label.set_label(&integer_object.unit_file().to_string());
        });

        list_box.bind_model(
            Some(&model),
            clone!(
                #[weak(rename_to = window)]
                self,
                #[upgrade_or_panic]
                move |obj| {
                    let unit: &UnitObject = obj
                        .downcast_ref()
                        .expect("The object should be of type `UnitObject`.");
                    Self::build_row(unit).into()
                }
            ),
        );
        let selection_model = SingleSelection::new(Some(model));
        list_box.connect_row_activated(move |list_box, position| {
            let foo = position.child().unwrap().downcast::<gtk::Box>().unwrap()
                .first_child().unwrap().downcast::<Label>().unwrap();
            println!("{}", foo.label().as_str());
        });
    }

    fn build_row(unit: &UnitObject) -> ListBoxRow {
        let enabled = match unit.state().as_str() {
            "enabled" => true,
            _ => false
        };
        let switch = Switch::builder()
            .state(enabled)
            .build();

        let binding = unit.unit_file();
        let x = binding.as_str();
        let label = &Label::new(Some(x));
        let bxx = gtk::Box::builder()
            .build();
        bxx.append(label);
        bxx.append(&switch);
        let item = ListBoxRow::builder()
            .child(&bxx)
            .build();
        item
    }
    // ANCHOR_END: setup_collections
}