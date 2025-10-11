mod imp;

use crate::systemd::{unit::UnitObject, SystemCtrlAction};
use crate::{systemd, table};
use adw::gio::{ActionEntry, ListStore};
use adw::glib::{clone, Object};
use adw::prelude::{ActionMapExtManual, AdwDialogExt, Cast};
use adw::subclass::prelude::ObjectSubclassIsExt;
use adw::{gio, glib, Toast, ToastOverlay};
use async_channel::{Receiver, Sender};
use gtk::prelude::{
    ButtonExt, EditableExt, FilterExt, SelectionModelExt, TextBufferExt, TextViewExt, WidgetExt,
};
use gtk::{Button, CustomFilter, FilterChange, FilterListModel, SingleSelection, SortListModel};
use std::cell::RefCell;
use std::collections::HashSet;
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

    fn list_store(&self) -> ListStore {
        self.imp()
            .list_store
            .borrow()
            .clone()
            .expect("List store must be initialized")
    }

    fn setup_column_view(&self) {
        let (units_receiver, toast_text_receiver) = Self::start_update();

        let model = self.list_store();
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

        Self::await_update(
            self.imp().overlay.clone(),
            units_receiver,
            toast_text_receiver,
            model,
        );
    }

    fn start_await_update(model: ListStore, overlay: ToastOverlay) {
        let (units_receiver, toast_text_receiver) = Self::start_update();
        Self::await_update(overlay, units_receiver, toast_text_receiver, model);
    }

    fn await_update(
        overlay_clone: ToastOverlay,
        units_receiver: Receiver<Vec<UnitObject>>,
        toast_text_receiver: Receiver<String>,
        model: ListStore,
    ) {
        // The main loop executes the asynchronous block
        glib::spawn_future_local(Self::await_units_data(units_receiver, model));
        glib::spawn_future_local(Self::await_units_toast(toast_text_receiver, overlay_clone));
    }

    fn start_update() -> (Receiver<Vec<UnitObject>>, Receiver<String>) {
        // Create a channel that can hold at most 1 message at a time
        let (units_sender, units_receiver) = async_channel::bounded(1);
        let (toast_text_sender, toast_text_receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || Self::load_units(units_sender, toast_text_sender));
        (units_receiver, toast_text_receiver)
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
                    model.remove_all();
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
            .send_blocking(items)
            .expect("The channel needs to be open.");
        let duration = start.elapsed().as_millis();
        let info_text = format!("Fetched {} units in {}ms", items_len, duration);
        toast_text_sender
            .send_blocking(info_text)
            .expect("The channel needs to be open.");
    }

    fn connect_selection_changed(&self, single_selection: &SingleSelection) {
        single_selection.connect_selection_changed(clone!(
            #[weak(rename_to = window)]
            self,
            move |selection, _, _| {
                window.on_selection_changed(selection);
            }
        ));
    }

    fn on_selection_changed(&self, selection: &SingleSelection) {
        let imp = self.imp();
        imp.search_bar.set_search_mode(false);

        let Some(item) = selection.selected_item() else {
            self.clear_selection_state();
            return;
        };

        let Ok(unit) = item.downcast::<UnitObject>() else {
            self.clear_selection_state();
            return;
        };

        imp.bottom_bar.set_revealed(true);
        imp.selected_unit.replace(Some(unit.clone()));
        self.update_view_button(&unit);
        self.update_action_buttons(&unit);
    }

    fn clear_selection_state(&self) {
        {
            let imp = self.imp();
            imp.bottom_bar.set_revealed(false);
            imp.selected_unit.replace(None);
            imp.view_unit_button.set_sensitive(false);
        }
        self.clear_text_view();
        self.hide_action_buttons();
    }

    fn clear_text_view(&self) {
        self.imp().text_view.buffer().set_text("");
    }

    fn update_view_button(&self, unit: &UnitObject) {
        match systemd::cat(unit.clone()) {
            Ok(content) => {
                let imp = self.imp();
                imp.text_view.buffer().set_text(&content);
                let title = Self::extract_unit_file_path(&content)
                    .unwrap_or_else(|| unit.unit_name().to_string());
                imp.dialog.set_title(&title);
                imp.view_unit_button.set_sensitive(true);
                imp.text_view.set_vexpand(true);
                imp.text_view.set_hexpand(true);
            }
            Err(err) => {
                {
                    let imp = self.imp();
                    imp.view_unit_button.set_sensitive(false);
                    imp.dialog.set_title(unit.unit_name().as_str());
                    let message = format!("Failed to load {}: {}", unit.unit_name(), err);
                    imp.overlay.add_toast(Toast::new(&message));
                }
                self.clear_text_view();
            }
        }
    }

    fn extract_unit_file_path(content: &str) -> Option<String> {
        content
            .lines()
            .find(|line| line.starts_with('#'))
            .and_then(|line| line.split_whitespace().nth(1))
            .map(str::to_string)
    }

    fn update_action_buttons(&self, unit: &UnitObject) {
        let available: HashSet<_> = SystemCtrlAction::available_actions(unit)
            .into_iter()
            .collect();
        self.for_each_action_button(|action, button| {
            button.set_visible(available.contains(&action));
        });
    }

    fn hide_action_buttons(&self) {
        self.for_each_action_button(|_, button| button.set_visible(false));
    }

    fn for_each_action_button<F>(&self, mut f: F)
    where
        F: FnMut(SystemCtrlAction, &Button),
    {
        let imp = self.imp();
        f(SystemCtrlAction::Start, &imp.start_button);
        f(SystemCtrlAction::Stop, &imp.stop_button);
        f(SystemCtrlAction::Restart, &imp.restart_button);
        f(SystemCtrlAction::Enable, &imp.enable_button);
        f(SystemCtrlAction::Disable, &imp.disable_button);
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
        self.setup_button_handlers();
    }

    fn setup_button_handlers(&self) {
        let dialog = self.imp().dialog.clone();
        let view_unit_button = self.imp().view_unit_button.clone();
        view_unit_button.connect_clicked(clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                dialog.present(Some(&window));
            }
        ));

        let start_button = self.imp().start_button.clone();
        self.connect_action_button(SystemCtrlAction::Start, &start_button);
        let stop_button = self.imp().stop_button.clone();
        self.connect_action_button(SystemCtrlAction::Stop, &stop_button);
        let restart_button = self.imp().restart_button.clone();
        self.connect_action_button(SystemCtrlAction::Restart, &restart_button);
        let enable_button = self.imp().enable_button.clone();
        self.connect_action_button(SystemCtrlAction::Enable, &enable_button);
        let disable_button = self.imp().disable_button.clone();
        self.connect_action_button(SystemCtrlAction::Disable, &disable_button);

        self.hide_action_buttons();
        self.imp().view_unit_button.set_sensitive(false);
    }

    fn connect_action_button(&self, action: SystemCtrlAction, button: &Button) {
        button.connect_clicked(clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.handle_action_click(action);
            }
        ));
    }

    fn handle_action_click(&self, action: SystemCtrlAction) {
        let Some(unit) = self.imp().selected_unit.borrow().clone() else {
            return;
        };

        match action {
            SystemCtrlAction::Start => systemd::start(unit.clone()),
            SystemCtrlAction::Stop => systemd::stop(unit.clone()),
            SystemCtrlAction::Restart => systemd::restart(unit.clone()),
            SystemCtrlAction::Enable => systemd::enable(unit.clone()),
            SystemCtrlAction::Disable => systemd::disable(unit.clone()),
        }

        self.refresh_units();
    }

    fn refresh_units(&self) {
        Self::start_await_update(self.list_store(), self.imp().overlay.clone());
    }
}
