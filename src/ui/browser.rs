use crate::model::{UnitId, UnitSummary, compare_names};
use gtk::prelude::*;
use relm4::{
    gtk,
    prelude::*,
    typed_view::column::{RelmColumn, TypedColumnView},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct Browser {
    pub table: TypedColumnView<UnitSummary, gtk::SingleSelection>,
    query: Rc<RefCell<String>>,
    replacing: Rc<Cell<bool>>,
}

#[derive(Debug)]
pub enum BrowserMsg {
    Replace(Vec<UnitSummary>),
    Search(String),
    Selection,
    Activate,
    FocusSearch,
}

#[derive(Debug, Clone)]
pub enum BrowserOutput {
    Selected(Option<UnitSummary>),
    OpenFile,
}

#[relm4::component(pub)]
impl Component for Browser {
    type Init = ();
    type Input = BrowserMsg;
    type Output = BrowserOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
            #[name = "search"]
            gtk::SearchEntry {
                set_placeholder_text: Some("Search units and descriptions"),
                set_margin_all: 12,
                set_tooltip_text: Some("Filter units (Ctrl+F)"),
                connect_search_changed[sender] => move |entry| {
                    sender.input(BrowserMsg::Search(entry.text().to_string()));
                },
            },
            gtk::Overlay {
                set_vexpand: true,
                set_hexpand: true,
                gtk::ScrolledWindow {
                    #[local_ref]
                    column_view -> gtk::ColumnView {},
                },
                #[name = "empty"]
                add_overlay = &gtk::Label {
                    set_label: "No units match your search.",
                    set_margin_all: 24,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    set_wrap: true,
                    set_can_target: false,
                    set_visible: false,
                },
            },
        }
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let mut table = TypedColumnView::<UnitSummary, gtk::SingleSelection>::new();
        table.view.add_css_class("data-table");
        table.view.add_css_class("unit-table");
        table.selection_model.set_autoselect(false);
        table.selection_model.set_can_unselect(true);
        table.append_column::<NameColumn>();
        table.append_column::<TypeColumn>();
        table.append_column::<LoadColumn>();
        table.append_column::<ActiveColumn>();
        table.append_column::<SubColumn>();
        table.append_column::<DescriptionColumn>();
        table
            .view
            .sort_by_column(table.get_columns().get("UNIT"), gtk::SortType::Ascending);
        let query = Rc::new(RefCell::new(String::new()));
        let filter_query = query.clone();
        table.add_filter(move |unit| unit.matches_normalized(&filter_query.borrow()));
        let replacing = Rc::new(Cell::new(false));
        let guard = replacing.clone();
        let selection_sender = sender.input_sender().clone();
        table
            .selection_model
            .connect_selection_changed(move |_, _, _| {
                if !guard.get() {
                    let _ = selection_sender.send(BrowserMsg::Selection);
                }
            });
        let activation_sender = sender.input_sender().clone();
        table.view.connect_activate(move |_, _| {
            let _ = activation_sender.send(BrowserMsg::Activate);
        });
        let model = Self {
            table,
            query,
            replacing,
        };
        let column_view = &model.table.view;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: BrowserMsg,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        match msg {
            BrowserMsg::Replace(units) => {
                let selected = self.selected().map(|unit| unit.id);
                self.replacing.set(true);
                self.table.clear();
                self.table.extend_from_iter(units);
                self.restore_selection(selected.as_ref());
                self.replacing.set(false);
                let _ = sender.output(BrowserOutput::Selected(self.selected()));
            }
            BrowserMsg::Search(query) => {
                let selected = self.selected().map(|unit| unit.id);
                self.replacing.set(true);
                *self.query.borrow_mut() = query.to_lowercase();
                self.table.notify_filter_changed(0);
                self.restore_selection(selected.as_ref());
                self.replacing.set(false);
                let _ = sender.output(BrowserOutput::Selected(self.selected()));
            }
            BrowserMsg::Selection => {
                let _ = sender.output(BrowserOutput::Selected(self.selected()));
            }
            BrowserMsg::Activate => {
                let _ = sender.output(BrowserOutput::OpenFile);
            }
            BrowserMsg::FocusSearch => {
                widgets.search.grab_focus();
            }
        }
        let visible = self.table.selection_model.n_items();
        widgets.empty.set_visible(visible == 0);
        widgets.empty.set_label(if self.query.borrow().is_empty() {
            "No loaded units."
        } else {
            "No units match your search."
        });
    }
}

impl Browser {
    pub fn selected(&self) -> Option<UnitSummary> {
        self.table
            .get_visible(self.table.selection_model.selected())
            .map(|item| item.borrow().clone())
    }

    fn restore_selection(&self, id: Option<&UnitId>) {
        // The selection uses visible positions; backing-store indices differ after sort/filter.
        let index = id.and_then(|id| {
            (0..self.table.selection_model.n_items()).find(|&i| {
                self.table
                    .get_visible(i)
                    .is_some_and(|item| &item.borrow().id == id)
            })
        });
        self.table
            .selection_model
            .set_selected(index.unwrap_or(gtk::INVALID_LIST_POSITION));
    }
}

macro_rules! column {
    ($name:ident, $title:literal, $get:expr, $expand:expr, $compare:expr) => {
        struct $name;
        impl RelmColumn for $name {
            type Root = gtk::Label;
            type Widgets = ();
            type Item = UnitSummary;
            const COLUMN_NAME: &'static str = $title;
            const ENABLE_RESIZE: bool = true;
            const ENABLE_EXPAND: bool = $expand;
            fn setup(_: &gtk::ListItem) -> (gtk::Label, ()) {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .margin_start(8)
                    .margin_end(8)
                    .margin_top(6)
                    .margin_bottom(6)
                    .build();
                (label, ())
            }
            fn bind(item: &mut UnitSummary, _: &mut (), label: &mut gtk::Label) {
                let get: fn(&UnitSummary) -> &str = $get;
                let value = get(item);
                label.set_label(value);
                label.set_tooltip_text(Some(value));
                // Factories recycle labels: always reset semantic styling on bind.
                label.remove_css_class("error");
                label.remove_css_class("success");
                if $title == "ACTIVE" {
                    match item.active.as_str() {
                        "failed" => label.add_css_class("error"),
                        "active" => label.add_css_class("success"),
                        _ => {}
                    }
                }
            }
            fn sort_fn() -> Option<Box<dyn Fn(&UnitSummary, &UnitSummary) -> std::cmp::Ordering>> {
                Some(Box::new($compare))
            }
        }
    };
}

column!(
    NameColumn,
    "UNIT",
    |u: &UnitSummary| &u.id.name,
    false,
    |a: &UnitSummary, b: &UnitSummary| compare_names(&a.id.name, &b.id.name)
);
column!(
    TypeColumn,
    "TYPE",
    UnitSummary::unit_type,
    false,
    |a: &UnitSummary, b: &UnitSummary| a
        .unit_type()
        .cmp(b.unit_type())
        .then_with(|| compare_names(&a.id.name, &b.id.name))
);
column!(
    LoadColumn,
    "LOAD",
    |u: &UnitSummary| &u.load,
    false,
    |a: &UnitSummary, b: &UnitSummary| a.load.cmp(&b.load)
);
column!(
    ActiveColumn,
    "ACTIVE",
    |u: &UnitSummary| &u.active,
    false,
    |a: &UnitSummary, b: &UnitSummary| a.active.cmp(&b.active)
);
column!(
    SubColumn,
    "SUB",
    |u: &UnitSummary| &u.sub,
    false,
    |a: &UnitSummary, b: &UnitSummary| a.sub.cmp(&b.sub)
);
column!(
    DescriptionColumn,
    "DESCRIPTION",
    |u: &UnitSummary| &u.description,
    true,
    |a: &UnitSummary, b: &UnitSummary| a
        .description
        .to_lowercase()
        .cmp(&b.description.to_lowercase())
);
