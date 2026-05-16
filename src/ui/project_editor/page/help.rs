use egui::ScrollArea;

use crate::ui::prelude::*;

/// This should probably have RenderedText, but it's currently broken. See #290
#[derive(Debug, Default)]
pub struct HelpPage {}

const HELP_TEXT: &str = include_str!("../../../../manual.md");

impl HelpPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorContext) -> CheeseResponse {
        layout::margin_box(ui, |ui| {
            ScrollArea::vertical().id_salt("manual").show(ui, |ui| {
                ui.label(HELP_TEXT);
            });

            CheeseResponse::default()
        })
    }
}
