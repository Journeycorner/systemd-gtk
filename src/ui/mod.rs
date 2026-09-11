pub mod browser;
pub mod viewer;

use crate::{
    backend::{BackendEvent, Result, SystemdBackend},
    model::{Scope, SessionState, Ticket, UnitAction, UnitDetails, UnitId, UnitSummary},
};
use adw::prelude::*;
use browser::{Browser, BrowserMsg, BrowserOutput};
use futures_util::{
    StreamExt,
    future::{AbortHandle, Abortable},
};
use relm4::{adw, gtk, prelude::*};
use std::{sync::Arc, time::Duration};
use viewer::FileViewer;

pub struct App {
    backend: Arc<dyn SystemdBackend>,
    pub session: SessionState,
    pub browser: Controller<Browser>,
    pub viewer: Option<Controller<FileViewer>>,
    pub details: Option<UnitDetails>,
    pub connected: bool,
    pub refreshing: bool,
    pub error: String,
    pub action_error: String,
    pub status: String,
    last_units: Vec<UnitSummary>,
    has_snapshot: bool,
    refresh_again: bool,
    debounce_pending: bool,
    watch_generation: u64,
    watch: Option<AbortHandle>,
    viewer_generation: u64,
}

#[derive(Debug, Clone)]
pub enum AppMsg {
    Scope(Scope),
    Refresh,
    Retry,
    Browser(BrowserOutput),
    OpenFile,
    FileClosed(u64),
    Action(UnitAction),
    Confirm(UnitId, UnitAction),
    Search,
    Shortcuts,
}

#[derive(Debug)]
pub enum AppCommand {
    Connected(u64),
    Event(u64, BackendEvent),
    Debounce(u64),
    Units(Ticket, Result<Vec<UnitSummary>>),
    Details(Ticket, UnitId, Result<UnitDetails>),
    ActionDone(UnitId, UnitAction, Result<()>),
}

#[relm4::component(pub)]
impl Component for App {
    type Init = Arc<dyn SystemdBackend>;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = AppCommand;

    view! {
        adw::ApplicationWindow {
            set_title: Some("systemd GTK"),
            set_icon_name: Some(crate::APP_ID),
            set_default_size: (1100, 720),
            #[name = "overlay"]
            adw::ToastOverlay {
                adw::ToolbarView {
                    add_top_bar = &adw::HeaderBar {
                        pack_start = &gtk::Image {
                            set_icon_name: Some(crate::APP_ID),
                            set_pixel_size: 32,
                            set_tooltip_text: Some("systemd GTK"),
                        },
                        pack_start = &gtk::DropDown::from_strings(&["System", "User"]) {
                            set_tooltip_text: Some("Choose the system or current user's systemd manager"),
                            #[watch] set_sensitive: model.session.pending.is_none(),
                            #[watch] set_selected: if model.session.scope == Scope::System { 0 } else { 1 },
                            connect_selected_notify[sender] => move |dropdown| {
                                sender.input(AppMsg::Scope(if dropdown.selected() == 0 { Scope::System } else { Scope::User }));
                            },
                        },
                        pack_end = &gtk::Button {
                            set_icon_name: "view-refresh-symbolic",
                            set_tooltip_text: Some("Refresh units (Ctrl+R)"),
                            #[watch] set_sensitive: model.connected && !model.refreshing,
                            connect_clicked => AppMsg::Refresh,
                        },
                        pack_end = &gtk::Button {
                            set_icon_name: "help-browser-symbolic",
                            set_tooltip_text: Some("Keyboard shortcuts"),
                            connect_clicked => AppMsg::Shortcuts,
                        },
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        gtk::Box {
                            set_spacing: 8,
                            set_margin_all: 12,
                            gtk::Spinner {
                                #[watch] set_spinning: model.refreshing || model.session.pending.is_some(),
                                #[watch] set_visible: model.refreshing || model.session.pending.is_some(),
                            },
                            gtk::Label {
                                #[watch] set_label: &model.status,
                                set_xalign: 0.0,
                                set_wrap: true,
                            },
                        },
                        gtk::Box {
                            set_spacing: 12,
                            set_margin_start: 12,
                            set_margin_end: 12,
                            #[watch] set_visible: !model.error.is_empty() || !model.action_error.is_empty(),
                            gtk::Label {
                                #[watch] set_label: &model.error_text(),
                                set_wrap: true,
                                set_selectable: true,
                                set_hexpand: true,
                                set_xalign: 0.0,
                            },
                            gtk::Button {
                                set_label: "Retry",
                                #[watch] set_sensitive: model.session.pending.is_none(),
                                connect_clicked => AppMsg::Retry,
                            },
                        },
                        #[local_ref]
                        browser_widget -> gtk::Box {},
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_all: 12,
                            set_spacing: 8,
                            #[watch] set_visible: model.session.selected.is_some(),
                            gtk::Label {
                                #[watch] set_label: &model.selection_label(),
                                set_xalign: 0.0,
                                set_selectable: true,
                                set_ellipsize: gtk::pango::EllipsizeMode::Middle,
                            },
                            gtk::Box {
                                set_spacing: 6,
                                gtk::Button {
                                    set_label: "Start",
                                    #[watch] set_sensitive: model.allows(UnitAction::Start),
                                    connect_clicked => AppMsg::Action(UnitAction::Start),
                                },
                                gtk::Button {
                                    set_label: "Stop",
                                    #[watch] set_sensitive: model.allows(UnitAction::Stop),
                                    connect_clicked => AppMsg::Action(UnitAction::Stop),
                                },
                                gtk::Button {
                                    set_label: "Restart",
                                    #[watch] set_sensitive: model.allows(UnitAction::Restart),
                                    connect_clicked => AppMsg::Action(UnitAction::Restart),
                                },
                                gtk::Button {
                                    set_label: "Enable",
                                    set_margin_start: 12,
                                    set_tooltip_text: Some("Enable automatic activation; does not start the unit now"),
                                    #[watch] set_sensitive: model.allows(UnitAction::Enable),
                                    connect_clicked => AppMsg::Action(UnitAction::Enable),
                                },
                                gtk::Button {
                                    set_label: "Disable",
                                    set_tooltip_text: Some("Disable automatic activation; does not stop the unit now"),
                                    #[watch] set_sensitive: model.allows(UnitAction::Disable),
                                    connect_clicked => AppMsg::Action(UnitAction::Disable),
                                },
                                gtk::Button {
                                    set_label: "View unit file",
                                    set_margin_start: 12,
                                    #[watch] set_sensitive: model.connected && model.session.selected.is_some(),
                                    connect_clicked => AppMsg::OpenFile,
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        backend: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let browser = Browser::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::Browser);
        let mut model = Self {
            backend,
            session: SessionState::default(),
            browser,
            viewer: None,
            details: None,
            connected: false,
            refreshing: false,
            error: String::new(),
            action_error: String::new(),
            status: "Connecting…".into(),
            last_units: Vec::new(),
            has_snapshot: false,
            refresh_again: false,
            debounce_pending: false,
            watch_generation: 0,
            watch: None,
            viewer_generation: 0,
        };
        let browser_widget = model.browser.widget();
        let widgets = view_output!();
        let actions = gtk::gio::SimpleActionGroup::new();
        for (name, message) in [
            ("search", AppMsg::Search),
            ("refresh", AppMsg::Refresh),
            ("view-unit", AppMsg::OpenFile),
            ("shortcuts", AppMsg::Shortcuts),
        ] {
            let action = gtk::gio::SimpleAction::new(name, None);
            let input = sender.input_sender().clone();
            action.connect_activate(move |_, _| {
                let _ = input.send(message.clone());
            });
            actions.add_action(&action);
        }
        root.insert_action_group("browser", Some(&actions));
        model.start_watch(&sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AppMsg, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            AppMsg::Scope(scope) => {
                if self.session.switch_scope(scope) {
                    self.close_viewer();
                    self.details = None;
                    self.last_units.clear();
                    self.has_snapshot = false;
                    self.browser.emit(BrowserMsg::Replace(Vec::new()));
                    self.start_watch(&sender);
                }
            }
            AppMsg::Refresh => self.refresh(&sender),
            AppMsg::Retry => {
                if self.session.pending.is_none() {
                    self.action_error.clear();
                    self.session.invalidate();
                    self.start_watch(&sender);
                }
            }
            AppMsg::Browser(BrowserOutput::Selected(unit)) => {
                if unit
                    .as_ref()
                    .is_some_and(|u| u.id.scope != self.session.scope)
                {
                    return;
                }
                self.load_details(unit.map(|u| u.id), &sender);
            }
            AppMsg::Browser(BrowserOutput::OpenFile) | AppMsg::OpenFile => {
                if self.connected
                    && let Some(id) = self.session.selected.clone()
                {
                    self.close_viewer();
                    let generation = self.viewer_generation;
                    let viewer = FileViewer::builder()
                        .launch((self.backend.clone(), id))
                        .forward(sender.input_sender(), move |_| {
                            AppMsg::FileClosed(generation)
                        });
                    viewer.widget().present(Some(root));
                    self.viewer = Some(viewer);
                }
            }
            AppMsg::FileClosed(generation) => {
                if generation == self.viewer_generation {
                    self.viewer = None;
                }
            }
            AppMsg::Action(action) => {
                if self.allows(action)
                    && let Some(id) = self.session.selected.clone()
                {
                    if action.needs_confirmation() {
                        let dialog = adw::AlertDialog::builder()
                            .heading(format!("{action} {}?", id.name))
                            .body(format!("This changes the {} unit. {}", id.scope, match action {
                                UnitAction::Stop => "The unit will be stopped.",
                                UnitAction::Restart => "The unit will be stopped and started again.",
                                _ => "Automatic startup will be disabled; a running unit will remain running.",
                            })).build();
                        dialog.add_responses(&[
                            ("cancel", "Cancel"),
                            ("confirm", &action.to_string()),
                        ]);
                        dialog.set_default_response(Some("cancel"));
                        dialog.set_close_response("cancel");
                        dialog.set_response_appearance(
                            "confirm",
                            adw::ResponseAppearance::Destructive,
                        );
                        let input = sender.input_sender().clone();
                        dialog.connect_response(None, move |_, response| {
                            if response == "confirm" {
                                let _ = input.send(AppMsg::Confirm(id.clone(), action));
                            }
                        });
                        dialog.present(Some(root));
                    } else {
                        self.begin_action(id, action, &sender);
                    }
                }
            }
            AppMsg::Confirm(id, action) => self.begin_action(id, action, &sender),
            AppMsg::Search => self.browser.emit(BrowserMsg::FocusSearch),
            AppMsg::Shortcuts => {
                let dialog = adw::ShortcutsDialog::new();
                let section = adw::ShortcutsSection::new(Some("Units"));
                for (title, accelerator) in [
                    ("Search", "<Control>f"),
                    ("Refresh", "<Control>r"),
                    ("View unit file", "Return"),
                    ("Keyboard shortcuts", "<Control>question"),
                ] {
                    section.add(adw::ShortcutsItem::new(title, accelerator));
                }
                dialog.add(section);
                dialog.present(Some(root));
            }
        }
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: AppCommand,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        match msg {
            AppCommand::Connected(generation) if generation == self.watch_generation => {
                self.connected = true;
                self.refreshing = false;
                self.error.clear();
                self.refresh(&sender);
            }
            AppCommand::Event(generation, event) if generation == self.watch_generation => {
                match event {
                    BackendEvent::Changed => {
                        if !self.debounce_pending {
                            self.debounce_pending = true;
                            sender.oneshot_command(async move {
                                tokio::time::sleep(Duration::from_millis(250)).await;
                                AppCommand::Debounce(generation)
                            });
                        }
                    }
                    BackendEvent::Disconnected(error) => {
                        self.connected = false;
                        self.error = error;
                        self.status = "Disconnected — displayed units may be stale.".into();
                        self.refreshing = false;
                        self.session.invalidate();
                        self.details = None;
                    }
                }
            }
            AppCommand::Debounce(generation) if generation == self.watch_generation => {
                self.debounce_pending = false;
                self.refresh(&sender);
            }
            AppCommand::Units(ticket, result) if self.session.accepts_list(ticket) => {
                self.refreshing = false;
                match result {
                    Ok(units) => {
                        if self.session.pending.is_none() {
                            self.status =
                                format!("{} loaded {} units", units.len(), self.session.scope);
                        }
                        self.error.clear();
                        if !self.has_snapshot || units != self.last_units {
                            self.has_snapshot = true;
                            self.last_units = units.clone();
                            self.browser.emit(BrowserMsg::Replace(units));
                        } else {
                            self.load_details(self.session.selected.clone(), &sender);
                        }
                    }
                    Err(error) => {
                        self.error = error.to_string();
                        self.connected = false;
                        self.session.invalidate();
                        self.details = None;
                        self.status = "Refresh failed — displayed units may be stale.".into();
                    }
                }
                if self.refresh_again {
                    self.refresh_again = false;
                    self.refresh(&sender);
                }
            }
            AppCommand::Details(ticket, id, result)
                if self.session.accepts_details(ticket, &id) =>
            {
                match result {
                    Ok(details) => self.details = Some(details),
                    Err(error) => {
                        self.details = None;
                        self.error = format!("Could not read {}: {error}", id.name);
                    }
                }
            }
            AppCommand::ActionDone(id, action, result)
                if self.session.pending.as_ref() == Some(&(id.clone(), action)) =>
            {
                self.session.pending = None;
                let failed = result.is_err();
                let message = match result {
                    Ok(()) => format!("{action} completed for {} ({})", id.name, id.scope),
                    Err(error) => format!("{action} {} ({}): {error}", id.name, id.scope),
                };
                widgets.overlay.add_toast(adw::Toast::new(&message));
                if failed {
                    self.action_error = message.clone();
                }
                self.status = message;
                self.details = None;
                self.session.select(self.session.selected.clone());
                self.refresh(&sender);
            }
            _ => {}
        }
        self.update_view(widgets, sender);
    }
}

impl App {
    fn error_text(&self) -> String {
        [&self.error, &self.action_error]
            .into_iter()
            .filter(|message| !message.is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn allows(&self, action: UnitAction) -> bool {
        self.connected
            && self.session.pending.is_none()
            && self.details.as_ref().is_some_and(|details| {
                self.session.selected.as_ref() == Some(&details.unit.id) && details.allows(action)
            })
    }

    fn selection_label(&self) -> String {
        self.session.selected.as_ref().map_or(String::new(), |id| {
            let state = self.details.as_ref().map_or("Loading details…", |d| {
                if d.file_state.is_empty() {
                    "No install state"
                } else {
                    &d.file_state
                }
            });
            format!("{} · {} · {}", id.scope, id.name, state)
        })
    }

    fn load_details(&mut self, id: Option<UnitId>, sender: &ComponentSender<Self>) {
        self.details = None;
        let ticket = self.session.select(id.clone());
        if self.connected
            && let Some(id) = id
        {
            let backend = self.backend.clone();
            sender.oneshot_command(async move {
                let result = backend.details(id.clone()).await;
                AppCommand::Details(ticket, id, result)
            });
        }
    }

    fn begin_action(&mut self, id: UnitId, action: UnitAction, sender: &ComponentSender<Self>) {
        if self.session.selected.as_ref() != Some(&id) || !self.allows(action) {
            return;
        }
        if let Some(details) = &self.details
            && self.session.begin_action(details, action)
        {
            self.action_error.clear();
            self.status = format!("{action} {} ({})…", id.name, id.scope);
            let backend = self.backend.clone();
            sender.oneshot_command(async move {
                let result = backend.execute(id.clone(), action).await;
                AppCommand::ActionDone(id, action, result)
            });
        }
    }

    fn refresh(&mut self, sender: &ComponentSender<Self>) {
        if !self.connected {
            return;
        }
        if self.refreshing {
            self.refresh_again = true;
            return;
        }
        self.refreshing = true;
        let ticket = self.session.list_ticket();
        let scope = self.session.scope;
        let backend = self.backend.clone();
        sender.oneshot_command(async move {
            AppCommand::Units(ticket, backend.list_units(scope).await)
        });
    }

    fn start_watch(&mut self, sender: &ComponentSender<Self>) {
        if let Some(watch) = self.watch.take() {
            watch.abort();
        }
        self.watch_generation += 1;
        self.details = None;
        self.session.invalidate();
        self.connected = false;
        self.refreshing = true;
        self.refresh_again = false;
        self.debounce_pending = false;
        self.error.clear();
        self.status = format!("Connecting to {} systemd…", self.session.scope);
        let generation = self.watch_generation;
        let scope = self.session.scope;
        let backend = self.backend.clone();
        let (abort, registration) = AbortHandle::new_pair();
        self.watch = Some(abort);
        sender.command(move |out, shutdown| {
            let task = async move {
                backend.reset(scope).await;
                match backend.subscribe(scope).await {
                    Ok(mut events) => {
                        let _ = out.send(AppCommand::Connected(generation));
                        while let Some(event) = events.next().await {
                            let disconnected = matches!(event, BackendEvent::Disconnected(_));
                            if out.send(AppCommand::Event(generation, event)).is_err() {
                                return;
                            }
                            if disconnected {
                                return;
                            }
                        }
                        let _ = out.send(AppCommand::Event(
                            generation,
                            BackendEvent::Disconnected("The systemd event stream ended.".into()),
                        ));
                    }
                    Err(error) => {
                        let _ = out.send(AppCommand::Event(
                            generation,
                            BackendEvent::Disconnected(error.to_string()),
                        ));
                    }
                }
            };
            async move {
                let _ = shutdown
                    .register(Abortable::new(task, registration))
                    .drop_on_shutdown()
                    .await;
            }
        });
    }

    fn close_viewer(&mut self) {
        self.viewer_generation += 1;
        if let Some(viewer) = self.viewer.take() {
            viewer.widget().force_close();
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(watch) = self.watch.take() {
            watch.abort();
        }
    }
}

pub fn run() {
    let application = adw::Application::builder()
        .application_id(crate::APP_ID)
        .build();
    application.connect_startup(|_| {
        init_assets();
    });
    application.set_accels_for_action("browser.search", &["<Control>f"]);
    application.set_accels_for_action("browser.refresh", &["<Control>r"]);
    // Enter belongs to the focused table/button/dialog, not a global window action.
    application.set_accels_for_action("browser.shortcuts", &["<Control>question"]);
    let app = RelmApp::from_app(application);
    app.run::<App>(Arc::new(crate::backend::DbusBackend::new()));
}

/// Register embedded assets after GTK initialization, including during UI tests.
pub fn init_assets() {
    gtk::gio::resources_register_include!("assets.gresource")
        .expect("embedded application resources must be valid");
    let display = gtk::gdk::Display::default().expect("GTK display must be initialized");
    gtk::IconTheme::for_display(&display).add_resource_path("/com/journeycorner/systemd-gtk/icons");
    let css = gtk::CssProvider::new();
    css.load_from_string(include_str!("../../data/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
