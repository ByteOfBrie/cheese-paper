use std::cell::OnceCell;

use crate::{ui::prelude::*, util::version::CodebergRelease};

/// Any sort of message that the user needs to be informed of
#[derive(Debug, Clone)]
pub enum Message {
    Generic(GenericMessage),
    Update(UpdateMessage),
}

/// Closable message with a message. Can be used for errors that the user needs to be informed of
#[derive(Debug, Clone)]
pub struct GenericMessage {
    pub message: String,
}

impl GenericMessage {
    pub fn ui(&self, ui: &mut Ui, _project: &Project, _ctx: &mut EditorContext) -> bool {
        ui.horizontal(|ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| ui.label(&self.message));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.button("dismiss").clicked()
            })
            .inner
        })
        .inner
    }
}

#[derive(Debug, Clone)]
pub struct UpdateMessage {
    pub release: CodebergRelease,
    message: OnceCell<String>,
}
impl UpdateMessage {
    pub fn new(release: CodebergRelease) -> Self {
        Self {
            release,
            message: OnceCell::new(),
        }
    }

    pub fn ui(&self, ui: &mut Ui, _project: &Project, ctx: &mut EditorContext) -> bool {
        let update_text = self.message.get_or_init(|| {
            if let Some((_, update_message)) =
                self.release.body.split_once("UPDATE_MESSAGE_OVERRIDE")
            {
                update_message.trim().to_owned()
            } else {
                format!(
                    "A new version of cheese-paper is available: {}",
                    self.release.tag_name
                )
            }
        });
        ui.horizontal(|ui| {
            ui.label(update_text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dismiss_button = ui.button("Dismiss").clicked();
                let skip_dismiss = if ui.button("Skip this version").clicked() {
                    if let Err(err) = ctx.ignore_version.set(self.release.tag_name.clone()) {
                        log::warn!(
                            "Tried to set ignore version twice, should not be possible: {err}"
                        )
                    }
                    true
                } else {
                    false
                };

                if ui.button("Open").clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(&self.release.html_url));
                }

                dismiss_button || skip_dismiss
            })
            .inner
        })
        .inner
    }
}

impl Message {
    /// Unified message UI. Messages output a bool if they should be dismissed (popped from the queue)
    pub fn ui(&self, ui: &mut Ui, project: &Project, ctx: &mut EditorContext) -> bool {
        match self {
            Message::Generic(message) => message.ui(ui, project, ctx),
            Message::Update(update_message) => update_message.ui(ui, project, ctx),
        }
    }
}
