use adw::prelude::*;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use relm4::{adw, gtk, prelude::*};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use systemd_gtk::{
    backend::{BackendError, BackendEvent, Result, SystemdBackend},
    model::{Scope, UnitAction, UnitDetails, UnitFileContent, UnitId, UnitSummary},
    ui::{App, AppMsg, browser::BrowserMsg},
};
use tokio::sync::{Semaphore, mpsc};

struct FakeBackend {
    units: Mutex<HashMap<Scope, Vec<UnitSummary>>>,
    events: Mutex<Vec<(Scope, mpsc::UnboundedSender<BackendEvent>)>>,
    list_calls: AtomicUsize,
    action_calls: AtomicUsize,
    file_calls: AtomicUsize,
    hold_system: AtomicBool,
    fail_files: AtomicBool,
    fail_actions: AtomicBool,
    list_gate: Semaphore,
    action_gate: Semaphore,
}

impl FakeBackend {
    fn new() -> Self {
        let units = [Scope::System, Scope::User]
            .into_iter()
            .map(|scope| {
                (
                    scope,
                    ["z.service", "a.timer", "b.service"]
                        .into_iter()
                        .map(|name| UnitSummary {
                            id: UnitId {
                                scope,
                                name: name.into(),
                            },
                            description: format!("{scope} {name}"),
                            load: "loaded".into(),
                            active: match name {
                                "z.service" => "failed",
                                "a.timer" => "active",
                                _ => "inactive",
                            }
                            .into(),
                            sub: match name {
                                "z.service" => "failed",
                                "a.timer" => "waiting",
                                _ => "dead",
                            }
                            .into(),
                        })
                        .collect(),
                )
            })
            .collect();
        Self {
            units: Mutex::new(units),
            events: Mutex::default(),
            list_calls: AtomicUsize::new(0),
            action_calls: AtomicUsize::new(0),
            file_calls: AtomicUsize::new(0),
            hold_system: AtomicBool::new(false),
            fail_files: AtomicBool::new(false),
            fail_actions: AtomicBool::new(false),
            list_gate: Semaphore::new(0),
            action_gate: Semaphore::new(0),
        }
    }

    fn event(&self, scope: Scope, event: BackendEvent) {
        for (target, sender) in self.events.lock().unwrap().iter() {
            if *target == scope {
                let _ = sender.send(event.clone());
            }
        }
    }
}

impl SystemdBackend for FakeBackend {
    fn list_units(&self, scope: Scope) -> BoxFuture<'_, Result<Vec<UnitSummary>>> {
        async move {
            self.list_calls.fetch_add(1, Ordering::SeqCst);
            if scope == Scope::System && self.hold_system.load(Ordering::SeqCst) {
                self.list_gate.acquire().await.unwrap().forget();
            }
            Ok(self.units.lock().unwrap()[&scope].clone())
        }
        .boxed()
    }
    fn details(&self, id: UnitId) -> BoxFuture<'_, Result<UnitDetails>> {
        async move {
            let unit = self.units.lock().unwrap()[&id.scope]
                .iter()
                .find(|u| u.id == id)
                .cloned()
                .ok_or_else(|| BackendError::Unavailable("Unit disappeared".into()))?;
            Ok(UnitDetails {
                unit,
                can_start: true,
                can_stop: true,
                refuse_start: false,
                refuse_stop: false,
                job_pending: false,
                file_state: "disabled".into(),
            })
        }
        .boxed()
    }
    fn unit_files(&self, id: UnitId) -> BoxFuture<'_, Result<UnitFileContent>> {
        async move {
            self.file_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_files.load(Ordering::SeqCst) {
                return Err(BackendError::Unavailable(
                    "No permission to read the file".into(),
                ));
            }
            Ok(UnitFileContent {
                title: id.name,
                text: "[Service]\nExecStart=/usr/bin/true".into(),
                warnings: vec![],
            })
        }
        .boxed()
    }
    fn execute(&self, id: UnitId, action: UnitAction) -> BoxFuture<'_, Result<()>> {
        async move {
            self.action_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_actions.load(Ordering::SeqCst) {
                return Err(BackendError::OutcomeUnknown("test timeout".into()));
            }
            self.action_gate.acquire().await.unwrap().forget();
            self.units
                .lock()
                .unwrap()
                .get_mut(&id.scope)
                .unwrap()
                .iter_mut()
                .find(|u| u.id == id)
                .unwrap()
                .active = if action == UnitAction::Stop {
                "inactive"
            } else {
                "active"
            }
            .into();
            Ok(())
        }
        .boxed()
    }
    fn subscribe(&self, scope: Scope) -> BoxFuture<'_, Result<BoxStream<'static, BackendEvent>>> {
        async move {
            let (sender, receiver) = mpsc::unbounded_channel();
            self.events.lock().unwrap().push((scope, sender));
            Ok(
                futures_util::stream::unfold(receiver, |mut receiver| async move {
                    receiver.recv().await.map(|event| (event, receiver))
                })
                .boxed(),
            )
        }
        .boxed()
    }
    fn reset(&self, _: Scope) -> BoxFuture<'_, ()> {
        async {}.boxed()
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let context = gtk::glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(8);
    // A short GLib timer guarantees a wakeup so the deadline works even if no
    // component emits another message. Completion is driven by state, not sleeps.
    let wakeup = gtk::glib::timeout_add_local(Duration::from_millis(10), || {
        gtk::glib::ControlFlow::Continue
    });
    while !condition() {
        assert!(Instant::now() < deadline, "UI condition timed out");
        context.iteration(true);
    }
    wakeup.remove();
}

fn screenshot(window: &adw::ApplicationWindow, name: &str) {
    let Ok(directory) = std::env::var("SYSTEMD_GTK_SCREENSHOTS") else {
        return;
    };
    let frames = std::rc::Rc::new(std::cell::Cell::new(0));
    let ticks = frames.clone();
    window.add_tick_callback(move |_, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() >= 3 {
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
    until(|| frames.get() >= 3);
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, window.width().into(), window.height().into());
    let node = snapshot.to_node().expect("The window must have rendered");
    let texture = window.renderer().unwrap().render_texture(&node, None);
    std::fs::create_dir_all(&directory).unwrap();
    texture
        .save_to_png(std::path::Path::new(&directory).join(name))
        .unwrap();
}

fn find_button(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    if let Some(button) = root.downcast_ref::<gtk::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(button) = find_button(&widget, label) {
            return Some(button);
        }
        child = widget.next_sibling();
    }
    None
}

#[test]
fn component_workflows_remain_responsive_and_scope_safe() {
    gtk::init().expect("UI tests require a display (run under Xvfb)");
    adw::init().unwrap();
    systemd_gtk::ui::init_assets();
    assert!(
        gtk::IconTheme::for_display(&gtk::gdk::Display::default().unwrap())
            .has_icon(systemd_gtk::APP_ID)
    );
    let application = adw::Application::builder()
        .application_id("com.journeycorner.systemd-gtk.test")
        .build();
    application
        .register(None::<&gtk::gio::Cancellable>)
        .unwrap();
    let backend = Arc::new(FakeBackend::new());
    let controller = App::builder().launch(backend.clone()).detach();
    controller.widget().set_application(Some(&application));
    controller.widget().present();
    until(|| {
        controller.model().connected
            && !controller.model().refreshing
            && controller.model().browser.model().table.len() == 3
    });
    assert_eq!(
        backend.file_calls.load(Ordering::SeqCst),
        0,
        "listing must not read unit files"
    );

    // The visible first row is b.service, not the backing store's z.service.
    {
        let app = controller.model();
        let browser = app.browser.model();
        assert!(browser.table.get_columns().contains_key("TYPE"));
        browser.table.view.sort_by_column(
            browser.table.get_columns().get("TYPE"),
            gtk::SortType::Descending,
        );
    }
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .get_visible(0)
            .is_some_and(|row| row.borrow().unit_type() == "timer")
    });
    {
        let app = controller.model();
        let browser = app.browser.model();
        browser.table.view.sort_by_column(
            browser.table.get_columns().get("UNIT"),
            gtk::SortType::Ascending,
        );
    }
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .get_visible(0)
            .is_some_and(|row| row.borrow().id.name == "b.service")
    });
    assert_eq!(
        controller
            .model()
            .browser
            .model()
            .table
            .get_visible(0)
            .unwrap()
            .borrow()
            .id
            .name,
        "b.service"
    );
    controller
        .model()
        .browser
        .model()
        .table
        .selection_model
        .set_selected(0);
    until(|| controller.model().allows(UnitAction::Start));
    if std::env::var_os("SYSTEMD_GTK_SCREENSHOTS").is_some() {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceLight);
        screenshot(controller.widget(), "light.png");
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        screenshot(controller.widget(), "dark.png");
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::Default);
    }
    assert_eq!(
        controller.model().session.selected.as_ref().unwrap().name,
        "b.service"
    );

    // Reconnecting must clear old capabilities before the new snapshot arrives.
    let before = backend.list_calls.load(Ordering::SeqCst);
    backend.hold_system.store(true, Ordering::SeqCst);
    controller.emit(AppMsg::Retry);
    until(|| backend.list_calls.load(Ordering::SeqCst) > before);
    assert!(controller.model().connected);
    assert!(controller.model().details.is_none());
    assert!(!controller.model().allows(UnitAction::Start));
    backend.hold_system.store(false, Ordering::SeqCst);
    backend.list_gate.add_permits(1);
    until(|| !controller.model().refreshing && controller.model().allows(UnitAction::Start));
    assert_eq!(
        backend.file_calls.load(Ordering::SeqCst),
        0,
        "selection must not read files"
    );
    controller
        .model()
        .browser
        .emit(BrowserMsg::Search("B.SERVICE".into()));
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .selection_model
            .n_items()
            == 1
    });
    assert_eq!(
        controller
            .model()
            .browser
            .model()
            .selected()
            .unwrap()
            .id
            .name,
        "b.service"
    );
    controller
        .model()
        .browser
        .emit(BrowserMsg::Search(String::new()));
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .selection_model
            .n_items()
            == 3
    });

    // A list replacement must preserve selection by identity in visible order.
    let before = backend.list_calls.load(Ordering::SeqCst);
    backend
        .units
        .lock()
        .unwrap()
        .get_mut(&Scope::System)
        .unwrap()[0]
        .description = "Updated".into();
    for _ in 0..10 {
        backend.event(Scope::System, BackendEvent::Changed);
    }
    until(|| backend.list_calls.load(Ordering::SeqCst) > before && !controller.model().refreshing);
    assert_eq!(
        backend.list_calls.load(Ordering::SeqCst),
        before + 1,
        "signal bursts should coalesce"
    );
    until(|| controller.model().allows(UnitAction::Start));
    assert_eq!(
        controller.model().session.selected.as_ref().unwrap().name,
        "b.service"
    );

    controller.emit(AppMsg::OpenFile);
    until(|| {
        controller
            .model()
            .viewer
            .as_ref()
            .is_some_and(|v| !v.model().loading)
    });
    assert!(
        controller
            .model()
            .viewer
            .as_ref()
            .unwrap()
            .model()
            .text
            .contains("ExecStart")
    );
    let weak_dialog = controller
        .model()
        .viewer
        .as_ref()
        .unwrap()
        .widget()
        .downgrade();
    controller
        .model()
        .viewer
        .as_ref()
        .unwrap()
        .widget()
        .force_close();
    until(|| controller.model().viewer.is_none());
    until(|| weak_dialog.upgrade().is_none());

    backend.fail_files.store(true, Ordering::SeqCst);
    controller.emit(AppMsg::OpenFile);
    until(|| {
        controller
            .model()
            .viewer
            .as_ref()
            .is_some_and(|v| !v.model().error.is_empty())
    });
    backend.fail_files.store(false, Ordering::SeqCst);
    controller
        .model()
        .viewer
        .as_ref()
        .unwrap()
        .emit(systemd_gtk::ui::viewer::FileMsg::Load);
    until(|| {
        controller
            .model()
            .viewer
            .as_ref()
            .is_some_and(|v| !v.model().loading && v.model().error.is_empty())
    });
    controller
        .model()
        .viewer
        .as_ref()
        .unwrap()
        .widget()
        .force_close();
    until(|| controller.model().viewer.is_none());

    // A held mutation leaves search usable but blocks duplicate dispatch and scope changes.
    controller.emit(AppMsg::Action(UnitAction::Start));
    until(|| backend.action_calls.load(Ordering::SeqCst) == 1);
    controller.emit(AppMsg::Action(UnitAction::Start));
    controller.emit(AppMsg::Scope(Scope::User));
    controller
        .model()
        .browser
        .emit(BrowserMsg::Search("not present".into()));
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .selection_model
            .n_items()
            == 0
    });
    assert_eq!(controller.model().session.scope, Scope::System);
    assert_eq!(backend.action_calls.load(Ordering::SeqCst), 1);
    backend.action_gate.add_permits(1);
    until(|| controller.model().session.pending.is_none());
    controller
        .model()
        .browser
        .emit(BrowserMsg::Search(String::new()));
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .selection_model
            .n_items()
            == 3
    });

    // Cancelling a disruptive action sends no backend request; confirming sends one.
    until(|| !controller.model().refreshing);
    controller
        .model()
        .browser
        .model()
        .table
        .selection_model
        .set_selected(0);
    until(|| controller.model().allows(UnitAction::Stop));
    controller.emit(AppMsg::Action(UnitAction::Stop));
    until(|| controller.widget().visible_dialog().is_some());
    let confirmation = controller
        .widget()
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    assert!(confirmation.heading().unwrap().contains("b.service"));
    assert!(confirmation.body().contains("System"));
    find_button(confirmation.upcast_ref(), "Cancel")
        .unwrap()
        .emit_clicked();
    drop(confirmation);
    until(|| controller.widget().visible_dialog().is_none());
    assert_eq!(backend.action_calls.load(Ordering::SeqCst), 1);
    controller.emit(AppMsg::Action(UnitAction::Stop));
    until(|| controller.widget().visible_dialog().is_some());
    let confirmation = controller
        .widget()
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    find_button(confirmation.upcast_ref(), "Stop")
        .unwrap()
        .emit_clicked();
    drop(confirmation);
    until(|| backend.action_calls.load(Ordering::SeqCst) == 2);
    backend.action_gate.add_permits(1);
    until(|| {
        controller.model().session.pending.is_none() && controller.model().allows(UnitAction::Start)
    });
    backend.fail_actions.store(true, Ordering::SeqCst);
    controller.emit(AppMsg::Action(UnitAction::Start));
    until(|| controller.model().action_error.contains("unknown") && !controller.model().refreshing);
    assert_eq!(
        backend.action_calls.load(Ordering::SeqCst),
        3,
        "unknown outcomes must not be replayed"
    );
    backend.fail_actions.store(false, Ordering::SeqCst);

    // A delayed system snapshot cannot overwrite the user manager after switching.
    let before = backend.list_calls.load(Ordering::SeqCst);
    backend.hold_system.store(true, Ordering::SeqCst);
    controller.emit(AppMsg::Refresh);
    until(|| backend.list_calls.load(Ordering::SeqCst) > before);
    controller.emit(AppMsg::Scope(Scope::User));
    until(|| {
        controller.model().session.scope == Scope::User
            && controller.model().connected
            && !controller.model().refreshing
    });
    backend.hold_system.store(false, Ordering::SeqCst);
    backend.list_gate.add_permits(1);
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .get_visible(0)
            .is_some_and(|u| u.borrow().id.scope == Scope::User)
    });
    assert!(controller.model().session.selected.is_none());

    backend.event(
        Scope::User,
        BackendEvent::Disconnected("Test connection lost".into()),
    );
    until(|| !controller.model().connected && controller.model().error.contains("lost"));
    assert!(!controller.model().allows(UnitAction::Start));
    controller.emit(AppMsg::Retry);
    until(|| {
        controller.model().connected
            && !controller.model().refreshing
            && controller.model().error.is_empty()
    });

    // A stream ending without an explicit error must not leave the UI "connected".
    backend.events.lock().unwrap().clear();
    until(|| !controller.model().connected && controller.model().error.contains("stream ended"));
    assert!(controller.model().details.is_none());
    controller.emit(AppMsg::Retry);
    until(|| controller.model().connected && !controller.model().refreshing);

    // A realistic large snapshot remains a virtualized table and accepts filtering.
    let many = (0..3000)
        .map(|i| UnitSummary {
            id: UnitId {
                scope: Scope::User,
                name: format!("test-{i:04}.service"),
            },
            description: "Large snapshot".into(),
            load: "loaded".into(),
            active: "inactive".into(),
            sub: "dead".into(),
        })
        .collect();
    backend.units.lock().unwrap().insert(Scope::User, many);
    backend.event(Scope::User, BackendEvent::Changed);
    until(|| controller.model().browser.model().table.len() == 3000);
    controller
        .model()
        .browser
        .emit(BrowserMsg::Search("test-2999".into()));
    until(|| {
        controller
            .model()
            .browser
            .model()
            .table
            .selection_model
            .n_items()
            == 1
    });
    controller.emit(AppMsg::Shortcuts);
    until(|| controller.widget().visible_dialog().is_some());
    assert!(
        controller
            .widget()
            .visible_dialog()
            .unwrap()
            .is::<adw::ShortcutsDialog>()
    );
    controller.widget().visible_dialog().unwrap().force_close();
    until(|| controller.widget().visible_dialog().is_none());

    controller.widget().close();
    drop(controller);
    until(|| {
        backend
            .events
            .lock()
            .unwrap()
            .iter()
            .all(|(_, sender)| sender.is_closed())
    });
}
