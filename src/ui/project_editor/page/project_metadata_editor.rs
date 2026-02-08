use crate::ui::{prelude::*, project_editor::update_title};

use egui::ScrollArea;

impl Project {
    pub fn metadata_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let cheese_response = egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_project_metadata_editor(ui, ctx))
            .inner;
        if cheese_response.modified {
            self.file.modified = true;
        }
        cheese_response
    }

    fn show_project_metadata_editor(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.base_metadata.name)
                    .id_salt("name")
                    .hint_text("Story Title")
                    .lock_focus(true)
                    .desired_width(f32::INFINITY),
            );
            cheese_response.process_response(&response, true);

            // Special case: update the title if we've changed it:
            if response.changed() {
                update_title(&self.base_metadata.name, ui.ctx());
            }

            let response = ui.add(
                egui::TextEdit::singleline(&mut self.metadata.genre)
                    .id_salt("genre")
                    .hint_text("Genre")
                    .lock_focus(true)
                    .desired_width(f32::INFINITY),
            );
            cheese_response.process_response(&response, true);

            ui.horizontal(|ui| {
                let half_width = ui.available_width() / 2.0;

                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.metadata.author)
                        .id_salt("author")
                        .hint_text("Author Name")
                        .lock_focus(true)
                        .desired_width(half_width),
                );
                cheese_response.process_response(&response, true);

                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.metadata.email)
                        .id_salt("email")
                        .hint_text("Author Email")
                        .lock_focus(true)
                        .desired_width(half_width),
                );
                cheese_response.process_response(&response, true);
            });

            // extract the height from some arbitrary text box, it shouldn't matter much
            let text_box_height = response.rect.height().abs();

            // Two widgets, each take up half the space
            let widget_height_total = ui.available_height() / 2.0;

            // We want to guess at the space that the collapsingheader will take up, it doesn't matter
            // if we're not completely right
            let widget_height = widget_height_total - text_box_height;

            egui::CollapsingHeader::new("Story Description/Summary")
                .default_open(true)
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        egui::vec2(ui.available_width(), widget_height),
                        |ui: &'_ mut Ui| self.metadata.summary.ui(ui, ctx),
                    );
                    cheese_response.process_response(&response, true);
                });

            egui::CollapsingHeader::new("Notes")
                .default_open(true)
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        egui::vec2(ui.available_width(), widget_height),
                        |ui: &'_ mut Ui| self.metadata.notes.ui(ui, ctx),
                    );

                    cheese_response.process_response(&response, true);
                });
        });
        cheese_response
    }
}
