use crate::{backend::SystemdBackend, model::UnitId};
use adw::prelude::*;
use relm4::{adw, gtk, prelude::*};
use std::sync::Arc;

pub struct FileViewer {
    backend: Arc<dyn SystemdBackend>,
    id: UnitId,
    pub loading: bool,
    pub text: String,
    pub error: String,
}

#[derive(Debug)]
pub enum FileMsg {
    Load,
    Closed,
}

#[relm4::component(pub)]
impl Component for FileViewer {
    type Init = (Arc<dyn SystemdBackend>, UnitId);
    type Input = FileMsg;
    type Output = ();
    type CommandOutput = crate::backend::Result<crate::model::UnitFileContent>;

    view! {
        adw::Dialog {
            set_title: &format!("{} — {}", model.id.name, model.id.scope),
            set_content_width: 800,
            set_content_height: 600,
            connect_closed => FileMsg::Closed,
            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {},
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    gtk::Spinner {
                        #[watch] set_spinning: model.loading,
                        #[watch] set_visible: model.loading,
                        set_margin_all: 12,
                    },
                    gtk::Label {
                        #[watch] set_label: &model.error,
                        #[watch] set_visible: !model.error.is_empty(),
                        set_wrap: true,
                        set_selectable: true,
                        set_margin_all: 12,
                    },
                    gtk::Button {
                        set_label: "Retry",
                        #[watch] set_visible: !model.error.is_empty() && !model.loading,
                        set_halign: gtk::Align::Center,
                        connect_clicked => FileMsg::Load,
                    },
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_hexpand: true,
                        #[name = "text_view"]
                        gtk::TextView {
                            set_editable: false,
                            set_cursor_visible: false,
                            set_monospace: true,
                            set_left_margin: 12,
                            set_right_margin: 12,
                            set_top_margin: 12,
                            set_bottom_margin: 12,
                        },
                    },
                },
            },
        }
    }

    fn init(
        (backend, id): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            backend,
            id,
            loading: true,
            text: String::new(),
            error: String::new(),
        };
        let widgets = view_output!();
        let backend = model.backend.clone();
        let id = model.id.clone();
        sender.oneshot_command(async move { backend.unit_files(id).await });
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: FileMsg, sender: ComponentSender<Self>, _: &Self::Root) {
        match msg {
            FileMsg::Load => {
                if self.loading {
                    return;
                }
                self.loading = true;
                self.error.clear();
                let backend = self.backend.clone();
                let id = self.id.clone();
                sender.oneshot_command(async move { backend.unit_files(id).await });
            }
            FileMsg::Closed => {
                self.text.clear();
                let _ = sender.output(());
            }
        }
    }

    fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        result: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        self.loading = false;
        match result {
            Ok(content) => {
                self.text = content.text;
                self.error = content.warnings.join("\n");
            }
            Err(error) => {
                self.text.clear();
                self.error = error.to_string();
            }
        }
        widgets.text_view.buffer().set_text(&self.text);
        self.update_view(widgets, sender);
    }
}
