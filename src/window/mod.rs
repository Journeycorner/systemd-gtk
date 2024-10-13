mod imp;

use crate::systemd;
use crate::systemd::UnitObject;
use adw::glib::Object;
use adw::prelude::{BoxExt, Cast, CastNone, ListItemExt, ObjectExt};
use adw::subclass::prelude::ObjectSubclassIsExt;
use adw::{gio, glib};
use gtk::prelude::{ActionableExtManual, EditableExt, FilterExt, SelectionModelExt, WidgetExt};
use gtk::{ColumnView, ColumnViewColumn, CustomFilter, CustomSorter, FilterChange, FilterListModel, Label, ListBoxRow, ListItem, ListItemFactory, SignalListItemFactory, SingleSelection, SortListModel};
use std::cell::RefCell;
use std::rc::Rc;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
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
        let column_view = self.imp().collections_list.get();
        Self::setup_columns(&column_view);

        let units = systemd::units();

        let model = gio::ListStore::new::<UnitObject>();
        model.extend_from_slice(&units);

        let filter_value: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        // Clone Rc for the filter closure
        let filter_value_for_filter = Rc::clone(&filter_value);
        let filter = CustomFilter::new(move |obj| {
            // Get `UnitObject` from `glib::Object`
            let unit_object = obj
                .downcast_ref::<UnitObject>()
                .expect("The object needs to be of type `UnitObject`.");

            // Check if unit_object's unit_file contains the filter value
            unit_object.unit_file().contains(&filter_value_for_filter.borrow().clone())
        });
        let filter_clone = filter.clone();

        // Now clone the Rc for the search_changed callback
        self.build_search_filter(filter, Rc::clone(&filter_value));

        // Now create the FilterListModel using the filter
        let filter_model = FilterListModel::new(Some(model), Some(filter_clone));

        let sorter = Self::build_sorter();

        // TODO trigger sorter on column selection
        let sort_model = SortListModel::new(Some(filter_model), Some(sorter.clone()));
        let single_selection = SingleSelection::new(Some(sort_model));
        self.connect_selection_changed(&single_selection);
        column_view.set_model(Some(&single_selection));
    }

    fn connect_selection_changed(&self, single_selection: &SingleSelection) {
        let action_button_clone = self.imp().action_button.clone();
        single_selection.connect_selection_changed(move |a, b, c| {
            let unit_object = a.selected_item()
                .unwrap().downcast::<UnitObject>()
                .unwrap();
            let active = unit_object.active().unwrap().eq("active");
            if active {
                action_button_clone.set_label("Stop");
                action_button_clone.connect_clicked(move |_| systemd::stop(unit_object.clone()));
            } else {
                action_button_clone.set_label("Start");
                action_button_clone.connect_clicked(move |_| systemd::start(unit_object.clone()));
            }
        });
    }

    fn build_sorter() -> CustomSorter {
        CustomSorter::new(move |obj1, obj2| {
            // Get `UnitObject` from `glib::Object`
            let unit_object_1 = obj1
                .downcast_ref::<UnitObject>()
                .expect("The object needs to be of type `UnitObject`.");
            let unit_object_2 = obj2
                .downcast_ref::<UnitObject>()
                .expect("The object needs to be of type `UnitObject`.");

            // Get property "number" from `UnitObject`
            let unit_file_1 = unit_object_1.unit_file();
            let unit_file_2 = unit_object_2.unit_file();

            // Reverse sorting order -> large numbers come first
            unit_file_1.cmp(&unit_file_2).into()
        })
    }

    fn setup_columns(column_view: &ColumnView) {
        let name_factory = SignalListItemFactory::new();
        Self::connect_name_factory(&name_factory, |unit_object| unit_object.unit_file().to_string());
        name_factory.connect_setup(Self::setup_factory());

        let load_factory = SignalListItemFactory::new();
        load_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&load_factory, |unit_object| unit_object.load().unwrap_or_default().to_string());

        let active_factory = SignalListItemFactory::new();
        active_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&active_factory, |unit_object| unit_object.active().unwrap_or_default().to_string());

        let description_factory = SignalListItemFactory::new();
        description_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&description_factory, |unit_object| unit_object.description().unwrap_or_default().to_string());

        column_view.append_column(&ColumnViewColumn::new(Some("UNIT"), Some(name_factory.upcast::<ListItemFactory>())));
        column_view.append_column(&ColumnViewColumn::new(Some("LOAD"), Some(load_factory.upcast::<ListItemFactory>())));
        column_view.append_column(&ColumnViewColumn::new(Some("ACTIVE"), Some(active_factory.upcast::<ListItemFactory>())));
        column_view.append_column(&ColumnViewColumn::new(Some("DESCRIPTION"), Some(description_factory.upcast::<ListItemFactory>())));
    }

    fn build_search_filter(&self, filter: CustomFilter, filter_value_for_search: Rc<RefCell<String>>) {
        let search_filter = self.imp().search_filter.get();

        search_filter.connect_search_changed(move |input| {
            // Update the filter_value inside RefCell
            *filter_value_for_search.borrow_mut() = input.text().to_string();

            // Notify that the filter has changed
            filter.changed(FilterChange::Different);
        });
    }

    fn setup_factory() -> fn(&SignalListItemFactory, &Object) {
        move |_, list_item| {
            let label = Label::new(None);
            list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&label));
        }
    }

    fn connect_name_factory<F>(name_factory: &SignalListItemFactory, transform_fn: F)
    where
        F: Fn(&UnitObject) -> String + 'static,
    {
        name_factory.connect_bind(move |_, list_item| {
            // Get `UnitObject` from `ListItem`
            let unit_object = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<UnitObject>()
                .expect("The item has to be an `UnitObject`.");

            // Get `Label` from `ListItem`
            let label = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<Label>()
                .expect("The child has to be a `Label`.");

            // Use the function passed as argument to get the label text
            let label_text = transform_fn(&unit_object);

            // Set the label text
            label.set_label(&label_text);
        });
    }

    fn build_row(unit: &UnitObject) -> ListBoxRow {
        let label = &Label::new(Some(unit.unit_file().as_str()));
        let bxx = gtk::Box::builder()
            .build();
        bxx.append(label);
        ListBoxRow::builder()
            .child(&bxx)
            .build()
    }
    // ANCHOR_END: setup_collections
}