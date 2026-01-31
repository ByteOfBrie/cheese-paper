//! A box for editing the name of a file object

use crate::ui::prelude::*;
use egui::Id;

#[derive(Debug, Default)]
pub struct NameBox {
    editing: bool,
    edit_content: String,
}

impl NameBox {
    pub fn ui(
        &mut self,
        text: &mut String,
        empty_text: &str,
        ui: &mut egui::Ui,
        _ctx: &mut EditorContext,
    ) -> (bool, Vec<Id>) {
        let mut tabable_ids = vec![];
        let mut modified = false;

        ui.horizontal(|ui| {
            if self.editing {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.edit_content)
                        .id_salt("name")
                        .hint_text(empty_text)
                        .lock_focus(true),
                );
                tabable_ids.push(response.id);
                let response = ui.button("✅");
                if response.clicked() {
                    *text = std::mem::take(&mut self.edit_content);
                    modified = true;
                    self.editing = false;
                }
                let response = ui.button("❌");
                if response.clicked() {
                    self.editing = false;
                }
            } else {
                ui.label(if !text.is_empty() {
                    text.as_str()
                } else {
                    empty_text
                });
                let response = ui.button("🖊");
                if response.clicked() {
                    self.edit_content = text.clone();
                    self.editing = true;
                }
            }
        });

        (modified, tabable_ids)
    }
}
