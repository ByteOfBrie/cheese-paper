use crate::ui::prelude::*;

#[derive(Debug)]
pub struct Measurements {
    pub measured: bool,
    pub text_box_height: f32,
    pub collapsible_header_extra_height: f32,
}

impl Default for Measurements {
    /// default measurements are based on measured values for a font size of 18
    fn default() -> Self {
        Self {
            measured: false,
            text_box_height: 24.6875,
            collapsible_header_extra_height: 29.5,
        }
    }
}

impl Measurements {
    /// Render a bunch of elements in a Ui to measure them
    /// The ui should be vertical with infinite height
    pub fn measure(&mut self, ui: &mut Ui) {
        let mut text = "Text".to_string();

        let response = ui.add(egui::TextEdit::singleline(&mut text).id_salt("measurement"));

        self.text_box_height = response.rect.height().abs();

        let min_height = 128.0;

        let before = ui.label("before");

        egui::CollapsingHeader::new("Summary")
            .default_open(true)
            .show(ui, |ui| {
                ui.add_sized(
                    egui::vec2(ui.available_width(), min_height),
                    egui::TextEdit::multiline(&mut text),
                );
            });

        let after = ui.label("after");

        let collapsing_header_height = after.rect.top() - before.rect.bottom();

        self.collapsible_header_extra_height = collapsing_header_height - min_height;

        self.measured = true;
        ui.ctx().request_repaint();
    }
}
