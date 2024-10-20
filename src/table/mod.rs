use crate::systemd::UnitObject;
use adw::glib::Object;
use adw::prelude::{Cast, CastNone};
use gtk::prelude::{ListItemExt, WidgetExt};
use gtk::{
    Align, ColumnView, ColumnViewColumn, CustomSorter, Label, ListItem, ListItemFactory, Ordering,
    SignalListItemFactory,
};

pub(crate) fn setup_columns(column_view: &ColumnView) {
    let name_factory = SignalListItemFactory::new();
    name_factory.connect_setup(move |a_, list_item| setup_factory(list_item));
    name_factory.connect_bind(move |_, list_item| {
        build_label(
            list_item,
            |unit_object| unit_object.unit_file().to_string(),
            30,
        )
    });

    let load_factory = SignalListItemFactory::new();
    load_factory.connect_setup(move |a_, list_item| setup_factory(list_item));
    load_factory.connect_bind(move |_, list_item| {
        build_label(
            list_item,
            |unit_object| unit_object.load().unwrap_or_default().to_string(),
            1_000,
        )
    });

    let active_factory = SignalListItemFactory::new();
    active_factory.connect_setup(move |a_, list_item| setup_factory(list_item));
    active_factory.connect_bind(move |_, list_item| {
        build_label(
            list_item,
            |unit_object| unit_object.active().unwrap_or_default().to_string(),
            1_000,
        )
    });

    let description_factory = SignalListItemFactory::new();
    description_factory.connect_setup(move |a_, list_item| setup_factory(list_item));
    description_factory.connect_bind(move |_, list_item| {
        build_label(
            list_item,
            |unit_object| unit_object.description().unwrap_or_default().to_string(),
            1_000,
        )
    });

    column_view.append_column(&with_expand("UNIT", name_factory));
    column_view.append_column(&with_expand("LOAD", load_factory));
    column_view.append_column(&with_expand("ACTIVE", active_factory));
    column_view.append_column(&with_expand("DESCRIPTION", description_factory));
}

fn setup_factory(list_item: &Object) {
    let label = Label::new(None);
    list_item
        .downcast_ref::<ListItem>()
        .expect("Needs to be ListItem")
        .set_child(Some(&label));
}

fn build_label<F>(list_item: &Object, transform_fn: F, max_len: usize)
where
    F: Fn(&UnitObject) -> String + 'static,
{
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
    let label_text_short = shorten_string(label_text, max_len);
    // Set the label text
    label.set_label(&label_text_short);
}

fn with_expand(unit_name: &str, name_factory: SignalListItemFactory) -> ColumnViewColumn {
    let column = ColumnViewColumn::new(
        Some(unit_name),
        Some(name_factory.upcast::<ListItemFactory>()),
    );
    column.set_expand(true);
    let sorter = CustomSorter::new(|one, two| Ordering::Equal);
    column.set_sorter(Some(&sorter));
    column.connect_sorter_notify(move |column| {
        let _ = column.clone();
    });
    column
}

fn shorten_string(s: String, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}
