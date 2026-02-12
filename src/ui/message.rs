use crate::ui::prelude::*;

/// Any sort of message that the user needs to be informed of
#[derive(Debug, Clone)]
pub enum Message {
    Generic(GenericMessage),
}

/// Closable message with a message. Can be used for errors that the user needs to be informed of
#[derive(Debug, Clone)]
pub struct GenericMessage {
    pub message: String,
}

impl GenericMessage {
    pub fn ui(&self, ui: &mut Ui, _project: &Project, _ctx: &mut EditorContext) -> bool {
        ui.horizontal(|ui| {
            // ui.label(&self.message);
            // ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            //     ui.button("dismiss").clicked()
            // })
            // .inner

            egui::ScrollArea::horizontal().show(ui, |ui| ui.label(&self.message));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.button("dismiss").clicked()
            })
            .inner
        })
        .inner
    }
}

impl Message {
    pub fn ui(&self, ui: &mut Ui, project: &Project, ctx: &mut EditorContext) -> bool {
        match self {
            Message::Generic(message) => message.ui(ui, project, ctx),
        }
    }
}
