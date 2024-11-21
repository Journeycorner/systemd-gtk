mod imp;

use crate::systemd::{unit::UnitObject, SystemCtrlAction};
use crate::{systemd, table};
use adw::gio::{ActionEntry, ListStore};
use adw::glib::{clone, Object};
use adw::prelude::{ActionMapExtManual, AdwDialogExt, Cast};
use adw::subclass::prelude::ObjectSubclassIsExt;
use adw::{gio, glib, Toast, ToastOverlay};
use async_channel::{Receiver, Sender};
use gtk::prelude::{ButtonExt, EditableExt, FilterExt, SelectionModelExt, TextViewExt, WidgetExt};
use gtk::{
    Button, CustomFilter, FilterChange, FilterListModel, SingleSelection, SortListModel, TextBuffer,
};
use std::cell::RefCell;
use std::fmt::Write;
use std::future::Future;
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

    fn setup_column_view(&self) {
        // Create channel that can hold at most 1 message at a time
        let (units_sender, units_receiver) = async_channel::bounded(1);
        let (toast_text_sender, toast_text_receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || Self::load_units(units_sender, toast_text_sender));

        let model = ListStore::new::<UnitObject>();
        let filter_input_value: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        // Clone Rc for the filter closure
        let filter_input_lower_case = Rc::clone(&filter_input_value);
        let filter =
            CustomFilter::new(move |obj| Self::search_filter(&filter_input_lower_case, obj));

        // Now clone the Rc for the search_changed callback
        self.build_search_filter(filter.clone(), Rc::clone(&filter_input_value));

        // Now create the FilterListModel using the filter
        let filter_model = FilterListModel::new(Some(model.clone()), Some(filter.clone()));

        let column_view = self.imp().column_view.get();
        let sort_model = SortListModel::new(Some(filter_model), column_view.sorter());

        let single_selection = SingleSelection::new(Some(sort_model));
        single_selection.set_autoselect(false);
        self.connect_selection_changed(&single_selection);

        column_view.set_model(Some(&single_selection));
        table::setup_columns(&column_view);

        // The main loop executes the asynchronous block
        glib::spawn_future_local(Self::await_units_data(units_receiver, model));
        let overlay_clone = self.imp().overlay.clone();
        glib::spawn_future_local(Self::await_units_toast(toast_text_receiver, overlay_clone));
    }

    async fn await_units_toast(toast_text_receiver: Receiver<String>, overlay_clone: ToastOverlay) {
        while let Ok(toast_text) = toast_text_receiver.recv().await {
            overlay_clone.add_toast(Toast::new(&toast_text));
        }
    }

    fn await_units_data(
        units_receiver: Receiver<Vec<UnitObject>>,
        model: ListStore,
    ) -> impl Future<Output=()> + Sized {
        clone!(
            #[weak]
            model,
            async move {
                while let Ok(items) = units_receiver.recv().await {
                    model.extend_from_slice(&items);
                }
            }
        )
    }

    fn search_filter(filter_input_lower_case: &Rc<RefCell<String>>, obj: &Object) -> bool {
        // Get `UnitObject` from `glib::Object`
        let unit_object = obj
            .downcast_ref::<UnitObject>()
            .expect("The object needs to be of type `UnitObject`.");

        // Check if unit_object's unit_name contains the filter value
        let input = &filter_input_lower_case.borrow().to_string();
        if unit_object.unit_name().to_lowercase().contains(input) {
            true
        } else {
            unit_object.description().to_lowercase().contains(input)
        }
    }

    fn load_units(units_sender: Sender<Vec<UnitObject>>, toast_text_sender: Sender<String>) {
        let start = Instant::now();
        let items = systemd::units();
        let items_len = items.len();
        units_sender
            .clone()
            .send_blocking(items)
            .expect("The channel needs to be open.");
        let duration = start.elapsed().as_millis();
        let info_text = format!("Fetched {} units in {}ms", items_len, duration);
        toast_text_sender
            .clone()
            .send_blocking(info_text)
            .expect("The channel needs to be open.");
    }

    fn connect_selection_changed(&self, single_selection: &SingleSelection) {
        let bottom_bar_clone = self.imp().bottom_bar.clone();
        let view_unit_button_clone = self.imp().view_unit_button.clone();
        let dialog_clone = self.imp().dialog.clone();
        let text_view_clone = self.imp().text_view.clone();
        let self_clone = self.clone();
        let search_bar_clone = self.imp().search_bar.clone();

        let start_button_clone = self.imp().start_button.clone();
        let stop_button_clone = self.imp().stop_button.clone();
        let restart_button_clone = self.imp().restart_button.clone();
        let enable_button_clone = self.imp().enable_button.clone();
        let disable_button_clone = self.imp().disable_button.clone();

        single_selection.connect_selection_changed(move |selection, _, _| {
            search_bar_clone.set_search_mode(false);
            bottom_bar_clone.set_revealed(true);
            let unit_object = selection
                .selected_item()
                .unwrap()
                .downcast::<UnitObject>()
                .unwrap();
            view_unit_button_clone.connect_clicked(clone!(
                #[weak]
                dialog_clone,
                #[weak]
                self_clone,
                move |_| {
                    dialog_clone.present(Some(&self_clone));
                }
            ));

            let unit_file_content = systemd::cat(unit_object.clone());
            if let Ok(content) = unit_file_content {
                // open new text buffer, otherwise the content will be concatenated
                text_view_clone.set_buffer(Some(&TextBuffer::default()));
                text_view_clone
                    .buffer()
                    .write_str(content.as_str())
                    .expect("Couldn't write to buffer.");
                view_unit_button_clone.set_sensitive(true);
                text_view_clone.set_vexpand(true);
                text_view_clone.set_hexpand(true);

                let file_path = content
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string(); // Clone as an owned String
                dialog_clone.set_title(&file_path);
            } else {
                view_unit_button_clone.set_sensitive(false);
            }

            // Define a list of actions and their corresponding buttons
            let actions_buttons = [
                (&SystemCtrlAction::Start, &start_button_clone),
                (&SystemCtrlAction::Stop, &stop_button_clone),
                (&SystemCtrlAction::Restart, &restart_button_clone),
                (&SystemCtrlAction::Enable, &enable_button_clone),
                (&SystemCtrlAction::Disable, &disable_button_clone),
            ];

            // Get the available actions once
            let available_actions = SystemCtrlAction::available_actions(&unit_object);

            // Iterate over each (action, button) pair
            for (action, button) in actions_buttons {
                if available_actions.contains(action) {
                    enable_button(action, button, unit_object.clone());
                } else {
                    disable_button(button);
                }
            }
        });
    }

    fn build_search_filter(
        &self,
        filter: CustomFilter,
        filter_value_for_search: Rc<RefCell<String>>,
    ) {
        let search_filter = self.imp().search_filter.get();

        search_filter.connect_search_changed(move |input| {
            // Update the filter_value inside RefCell
            *filter_value_for_search.borrow_mut() = input.text().to_lowercase();

            // Notify that the filter has changed
            filter.changed(FilterChange::Different);
        });
    }

    fn setup_actions(&self) {
        let search_bar_action = ActionEntry::builder("search_bar_show")
            .activate(|window: &Self, _, _| window.imp().search_bar.set_search_mode(true))
            .build();
        let view_unit_action = ActionEntry::builder("view_unit_action")
            .activate(|window: &Self, _, _| {
                let button = &window.imp().view_unit_button;
                if button.get_sensitive() {
                    button.emit_clicked()
                }
            })
            .build();

        self.add_action_entries([search_bar_action, view_unit_action]);
    }
}

fn enable_button(action: &SystemCtrlAction, button: &Button, unit: UnitObject) {
    match action {
        SystemCtrlAction::Start => button.connect_clicked(move |_| systemd::start(unit.clone())),
        SystemCtrlAction::Stop => button.connect_clicked(move |_| systemd::stop(unit.clone())),
        SystemCtrlAction::Restart => {
            button.connect_clicked(move |_| systemd::restart(unit.clone()))
        }
        SystemCtrlAction::Enable => button.connect_clicked(move |_| systemd::enable(unit.clone())),
        SystemCtrlAction::Disable => {
            button.connect_clicked(move |_| systemd::disable(unit.clone()))
        }
    };
    button.set_visible(true);
}

fn disable_button(button: &Button) {
    button.set_visible(false);
}
