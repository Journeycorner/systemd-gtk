mod imp;

use crate::systemd;
use crate::systemd::UnitObject;
use adw::glib::{clone, Object};
use adw::prelude::{Cast, CastNone, ListItemExt};
use adw::subclass::prelude::ObjectSubclassIsExt;
use adw::{gio, glib, Toast};
use gtk::prelude::{EditableExt, FilterExt, SelectionModelExt, WidgetExt};
use gtk::{Align, ColumnView, ColumnViewColumn, CustomFilter, CustomSorter, FilterChange, FilterListModel, Label, ListItem, ListItemFactory, SignalListItemFactory, SingleSelection, SortListModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

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
        // Create channel that can hold at most 1 message at a time
        let (units_sender, units_receiver) = async_channel::bounded(1);
        let (toast_text_sender, toast_text_receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            let start = Instant::now();
            let items = systemd::units();
            let items_len = items.len();
            units_sender.clone()
                .send_blocking(items)
                .expect("The channel needs to be open.");
            let duration = start.elapsed().as_millis();
            let info_text = format!("Fetched {} units in {}ms", items_len, duration);
            toast_text_sender.clone().send_blocking(info_text).expect("The channel needs to be open.");
        });

        let model = gio::ListStore::new::<UnitObject>();
        let column_view = self.imp().collections_list.get();
        Self::setup_columns(&column_view);

        let filter_input_value: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        // Clone Rc for the filter closure
        let filter_input_lower_case = Rc::clone(&filter_input_value);
        let filter = CustomFilter::new(move |obj| {
            // Get `UnitObject` from `glib::Object`
            let unit_object = obj
                .downcast_ref::<UnitObject>()
                .expect("The object needs to be of type `UnitObject`.");

            // Check if unit_object's unit_file contains the filter value
            let input = &filter_input_lower_case.borrow().to_string();
            if unit_object.unit_file().to_lowercase().contains(input) {
                true
            } else if let Some(desc) = unit_object.description() {
                desc.to_lowercase().contains(input)
            } else {
                false
            }
        });
        let filter_clone = filter.clone();

        // Now clone the Rc for the search_changed callback
        self.build_search_filter(filter, Rc::clone(&filter_input_value));

        // Now create the FilterListModel using the filter
        let filter_model = FilterListModel::new(Some(model.clone()), Some(filter_clone));

        let sorter = Self::build_sorter();

        // TODO trigger sorter on column selection
        let sort_model = SortListModel::new(Some(filter_model), Some(sorter.clone()));
        let single_selection = SingleSelection::new(Some(sort_model));
        self.connect_selection_changed(&single_selection);
        column_view.set_model(Some(&single_selection));

        // The main loop executes the asynchronous block
        glib::spawn_future_local(clone!(
            #[weak]
            model,
            async move {
                while let Ok(items) = units_receiver.recv().await {
                    model.extend_from_slice(&items);
                }
            }
        ));
        let overlay_clone = self.imp().overlay.clone();
        glib::spawn_future_local(
            async move {
                while let Ok(toast_text) = toast_text_receiver.recv().await {
                    overlay_clone.add_toast(Toast::new(&*toast_text));
                }
            }
        );
    }

    fn connect_selection_changed(&self, single_selection: &SingleSelection) {
        let action_button_clone = self.imp().action_button.clone();
        let bottom_bar_clone = self.imp().bottom_bar.clone();
        single_selection.connect_selection_changed(move |selection, _, _| {
            bottom_bar_clone.set_revealed(true);
            let unit_object = selection.selected_item()
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
        Self::connect_name_factory(&name_factory, |unit_object| unit_object.unit_file().to_string(), 30);
        name_factory.connect_setup(Self::setup_factory());

        let load_factory = SignalListItemFactory::new();
        load_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&load_factory, |unit_object| unit_object.load().unwrap_or_default().to_string(), 1_000);

        let active_factory = SignalListItemFactory::new();
        active_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&active_factory, |unit_object| unit_object.active().unwrap_or_default().to_string(), 1_000);

        let description_factory = SignalListItemFactory::new();
        description_factory.connect_setup(Self::setup_factory());
        Self::connect_name_factory(&description_factory, |unit_object| unit_object.description().unwrap_or_default().to_string(), 1_000);

        column_view.append_column(&Self::with_expand("UNIT", name_factory));
        column_view.append_column(&Self::with_expand("LOAD",load_factory));
        column_view.append_column(&Self::with_expand("ACTIVE",active_factory));
        column_view.append_column(&Self::with_expand("DESCRIPTION",description_factory));
    }

    fn with_expand(unit_name: &str, name_factory: SignalListItemFactory) -> ColumnViewColumn {
        let column = ColumnViewColumn::new(Some(unit_name), Some(name_factory.upcast::<ListItemFactory>()));
        column.set_expand(true);
        column
    }

    fn build_search_filter(&self, filter: CustomFilter, filter_value_for_search: Rc<RefCell<String>>) {
        let search_filter = self.imp().search_filter.get();

        search_filter.connect_search_changed(move |input| {
            // Update the filter_value inside RefCell
            *filter_value_for_search.borrow_mut() = input.text().to_lowercase();

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

    fn connect_name_factory<F>(name_factory: &SignalListItemFactory, transform_fn: F, max_len: usize)
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
            label.set_halign(Align::Start);
            // Use the function passed as argument to get the label text
            let label_text = transform_fn(&unit_object);
            let label_text_short = Self::shorten_string(label_text, max_len);
            // Set the label text
            label.set_label(&label_text_short);
        });
    }

    fn shorten_string(s: String, max_len: usize) -> String {
        if s.len() > max_len {
            format!("{}...", &s[..max_len])
        } else {
            s
        }
    }
}