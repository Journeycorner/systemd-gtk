use crate::systemd::unit::UnitObject;
use adw::prelude::{Cast, CastNone, ListItemExt, WidgetExt};
use gtk::glib::Object;
use gtk::prelude::BoxExt;
use gtk::{
    ColumnView, ColumnViewColumn, CustomSorter, Label, ListItem, ListItemFactory, Ordering,
    SignalListItemFactory, SortType,
};

/// Sets up the columns for the given `ColumnView` widget.
///
/// This function takes a `ColumnView` widget and adds multiple columns to it, each representing a different property of a `UnitObject`.
/// The columns include "UNIT", "LOAD", "ACTIVE", "SUB", and "DESCRIPTION". Each column is associated with a getter function that
/// extracts the appropriate property from a `UnitObject`. The "UNIT" column is sorted in ascending order by default.
///
/// # Arguments
/// * `column_view` - A reference to a `ColumnView` widget to which columns will be added.
///
/// # GTK-RS
/// This function uses GTK-RS to create columns for a `ColumnView` widget. It utilizes `SignalListItemFactory` to create list item factories,
/// `CustomSorter` to define custom sorting for columns, and `ColumnViewColumn` to represent individual columns in the `ColumnView`.
pub fn setup_columns(column_view: &ColumnView) {
    let properties: &[(
        &str,
        fn(&UnitObject) -> String,
        Option<fn(&str) -> (&str, &str)>,
    )] = &[
        ("UNIT", UnitObject::unit_name, Some(split_name_and_suffix)),
        ("LOAD", UnitObject::load, None),
        ("ACTIVE", UnitObject::state, None),
        ("SUB", UnitObject::sub_state, None),
        ("DESCRIPTION", UnitObject::description, None),
    ];

    for (title, getter, split_func) in properties {
        let factory = create_factory(*getter);
        let column = with_expand(title, factory, *getter, *split_func);
        // sort by unit column by default
        if "UNIT".eq(*title) {
            column_view.sort_by_column(Some(&column), SortType::Ascending);
        }
        column_view.append_column(&column);
    }
}

fn create_factory(getter: fn(&UnitObject) -> String) -> SignalListItemFactory {
    let factory = SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| setup_factory(list_item));
    factory.connect_bind(move |_, list_item| build_label(list_item, getter, 1_000));
    factory
}

fn setup_factory(list_item: &Object) {
    let list_item = list_item
        .downcast_ref::<ListItem>()
        .expect("Needs to be ListItem");

    let label = Label::new(None);
    let boxx = gtk::Box::default();
    boxx.append(&label);

    list_item.set_child(Some(&boxx));
}

fn build_label(list_item: &Object, transform_fn: fn(&UnitObject) -> String, max_len: usize) {
    let unit_object = list_item
        .downcast_ref::<ListItem>()
        .expect("Needs to be ListItem")
        .item()
        .and_downcast::<UnitObject>()
        .expect("The item has to be an `UnitObject`.");

    let boxx = list_item
        .downcast_ref::<ListItem>()
        .expect("Needs to be ListItem")
        .child()
        .and_downcast::<gtk::Box>()
        .expect("The child has to be a `Box`.");

    // TODO use file name
    boxx.set_tooltip_text(Some(unit_object.unit_name().as_str()));

    let _ = if !unit_object.state().eq("active") {
        WidgetExt::add_css_class
    } else {
        // removal is necessary because of widget reuse
        WidgetExt::remove_css_class
    }(&boxx, "inactive");

    let label = boxx
        .first_child()
        .unwrap()
        .downcast::<Label>()
        .expect("The child has to be a `Label`.");

    let label_text = transform_fn(&unit_object);
    label.set_label(&shorten_string(label_text, max_len));
}

fn with_expand(
    unit_name: &str,
    factory: SignalListItemFactory,
    getter: fn(&UnitObject) -> String,
    split_func: Option<fn(&str) -> (&str, &str)>,
) -> ColumnViewColumn {
    let column = ColumnViewColumn::new(Some(unit_name), Some(factory.upcast::<ListItemFactory>()));
    column.set_expand(true);
    let sorter = CustomSorter::new(move |one, two| {
        let unit_object_1 = one
            .downcast_ref::<UnitObject>()
            .expect("The object needs to be of type `UnitObject`.");
        let unit_object_2 = two
            .downcast_ref::<UnitObject>()
            .expect("The object needs to be of type `UnitObject`.");

        let value_1 = getter(unit_object_1);
        let value_2 = getter(unit_object_2);

        if let Some(split) = split_func {
            // special case: sort by type first and by name second
            let (name_a, suffix_a) = split(&value_1);
            let (name_b, suffix_b) = split(&value_2);
            return match suffix_a.cmp(&suffix_b) {
                std::cmp::Ordering::Equal => {
                    string_compare_sort((&name_a).parse().unwrap(), name_b.parse().unwrap())
                }
                other => other.into(),
            };
        } else {
            string_compare_sort(value_1, value_2)
        }
    });
    column.set_sorter(Some(&sorter));
    column
}

fn string_compare_sort(value_1: String, value_2: String) -> Ordering {
    value_1.to_lowercase().cmp(&value_2.to_lowercase()).into()
}

fn shorten_string(s: String, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}

/// Split unit name and suffix in order to enable two layer sorting
fn split_name_and_suffix(s: &str) -> (&str, &str) {
    if let Some(idx) = s.rfind('.') {
        (&s[..idx], &s[idx..])
    } else {
        (s, "")
    }
}
