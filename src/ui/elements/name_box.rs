//! A box for editing the name of a file object

use crate::ui::prelude::*;

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
        ctx: &mut EditorContext,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();

        ui.horizontal(|ui| {
            if self.editing {
                let edit_response = ui.add(
                    egui::TextEdit::singleline(&mut self.edit_content)
                        .id_salt("name")
                        .hint_text(empty_text)
                        .lock_focus(true),
                );
                if ctx.focus_jumper.recieve(&"name_edit_field") {
                    edit_response.request_focus();
                }
                cheese_response.tabable_ids.push(edit_response.id);

                let response = ui.button("✅");
                if response.clicked()
                    || (edit_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    cheese_response.modified = *text != self.edit_content;
                    *text = std::mem::take(&mut self.edit_content);
                    self.editing = false;
                }
                let response = ui.button("❌");
                if response.clicked()
                    || (edit_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Escape)))
                {
                    self.editing = false;
                }
            } else {
                ui.label(if !text.is_empty() {
                    text.as_str()
                } else {
                    empty_text
                });
                let response = ui.button("🖊");
                if response.clicked() || ui.input(|i| i.key_pressed(egui::Key::F2)) {
                    self.edit_content = text.clone();
                    self.editing = true;
                    ctx.focus_jumper.send("name_edit_field");
                }
            }
        });

        cheese_response
    }
}
