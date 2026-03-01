use crate::ui::prelude::*;
use crate::ui::settings::Setting;

use egui::{Color32, RichText};

use super::ThemeSelection;

#[derive(Debug)]
pub struct SettingsPage {
    random_theme_name: String,

    random_theme_save_error: Option<CheeseError>,
}

impl Setting<bool> {
    fn ui<'a>(&'a mut self, ui: &mut egui::Ui, atoms: impl egui::IntoAtoms<'a>) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        let response = ui.checkbox(&mut self.user_editable, atoms);
        if response.changed() {
            self.modified_entry = true;
        }
        cheese_response.process_response(&response, true);
        cheese_response
    }
}

impl SettingsPage {
    // TODO: maybe get rid of this
    pub fn load(_ctx: &mut EditorContext) -> Self {
        Self {
            random_theme_name: String::new(),
            random_theme_save_error: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();

        ui.heading("Settings");

        cheese_response.extend(self.settings_ui(ui, ctx));

        ui.separator();

        ui.heading("Themes");

        cheese_response.extend(self.themes_ui(ui, ctx));

        // TODO: maybe parse cheese response here? might need to make sure themes are good

        cheese_response
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut settings_data = ctx.settings.0.borrow_mut();

        let mut cheese_response = CheeseResponse::default();

        ui.label("Font Size");

        let response = ui.text_edit_singleline(&mut settings_data.font_size.user_entry);
        if response.changed() {
            settings_data.font_size.modified_entry = true;
        }
        cheese_response.process_response(&response, true);

        if let Some(err) = &settings_data.font_size.error_message {
            ui.label(RichText::new(err).color(Color32::RED));
        }

        let response = settings_data.indent_line_start.ui(ui, "Indent Line Start");
        cheese_response.extend(response);

        let response = settings_data
            .highlight_multiple_spaces
            .ui(ui, "Highlight Multiple Spaces (in a Row)");
        cheese_response.extend(response);

        let response = settings_data
            .highlight_spaces_before_punctuation
            .ui(ui, "Highlight Spaces Between a Word and Punctuation");
        cheese_response.extend(response);

        let response = settings_data.check_for_updates.ui(ui, "Check for Updates");
        cheese_response.extend(response);

        let response = settings_data
            .reopen_last
            .ui(ui, "Reopen Last Project on Launch");
        cheese_response.extend(response);

        ui.label("Dictionary Location");

        let response = ui.text_edit_singleline(&mut settings_data.dictionary_location.user_entry);
        if response.changed() {
            settings_data.dictionary_location.modified_entry = true;
        }
        cheese_response.process_response(&response, true);

        if let Some(err) = &settings_data.dictionary_location.error_message {
            ui.label(RichText::new(err).color(Color32::RED));
        }

        if cheese_response.modified {
            ctx.render_version += 1;
        }

        cheese_response
    }

    fn themes_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        let mut update = false;

        let selected = ctx.settings.selected_theme();

        ui.horizontal(|ui| {
            if matches!(selected, ThemeSelection::Default) {
                ui.label("->");
            } else {
                ui.label("  ");
            }
            let response = ui.button("Default");
            if response.clicked() {
                ctx.settings.select_theme(ThemeSelection::Default).unwrap();
                update = true;
            }
            cheese_response.tabable_ids.push(response.id);
        });

        ui.horizontal(|ui| {
            if matches!(selected, ThemeSelection::DefaultLight) {
                ui.label("->");
            } else {
                ui.label("  ");
            }
            let response = ui.button("Light");
            if response.clicked() {
                ctx.settings
                    .select_theme(ThemeSelection::DefaultLight)
                    .unwrap();
                update = true;
            }
            cheese_response.tabable_ids.push(response.id);
        });

        ui.horizontal(|ui| {
            if matches!(selected, ThemeSelection::Random) {
                ui.label("->");
            } else {
                ui.label("  ");
            }
            let response = ui.button("Random");
            if response.clicked() {
                ctx.settings.select_theme(ThemeSelection::Random).unwrap();
                update = true;
            }
            cheese_response.tabable_ids.push(response.id);
        });

        ui.horizontal(|ui| {
            ui.heading("Available Presets");
            if ui.button("reload").clicked() {
                ctx.actions.schedule(move |project_editor, ctx| {
                    project_editor
                        .editor_context
                        .settings
                        .load()
                        .unwrap_or_else(|err| {
                            log::error!("Error encountered while reloading settings: {err}");
                        });
                    project_editor
                        .editor_context
                        .settings
                        .select_theme(selected)
                        .unwrap_or_else(|err| {
                            log::error!("Error encountered while selecting settings: {err}");
                        });
                    project_editor.update_theme(ctx);
                });
            }
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (idx, (name, _)) in ctx.settings.available_themes().iter().enumerate() {
                ui.horizontal(|ui| {
                    if matches!(selected, ThemeSelection::Preset(i) if i == idx) {
                        ui.label("->");
                    } else {
                        ui.label("  ");
                    }
                    let response = ui.button(name);
                    if response.clicked() {
                        ctx.settings
                            .select_theme(ThemeSelection::Preset(idx))
                            .unwrap();
                        update = true;
                    }
                    cheese_response.tabable_ids.push(response.id);
                });
            }
        });

        ui.separator();

        if matches!(selected, ThemeSelection::Random) {
            ui.label("Save random theme as preset ?");
            ui.horizontal(|ui| {
                ui.label("name : ");
                let response = ui.text_edit_singleline(&mut self.random_theme_name);
                cheese_response.tabable_ids.push(response.id);
                let response = ui.button("Save");
                if response.clicked() {
                    self.random_theme_save_error = ctx
                        .settings
                        .save_current_theme(&self.random_theme_name)
                        .err();
                    if self.random_theme_save_error.is_none() {
                        ctx.actions.schedule(|project_editor, _| {
                            if let Err(err) = project_editor.editor_context.settings.load() {
                                log::error!("Error encountered while reloading settings: {err}");
                            }
                        });
                    }
                }
                cheese_response.tabable_ids.push(response.id);
            });
            if let Some(err) = &self.random_theme_save_error {
                ui.label(RichText::new(err.to_string()).color(Color32::RED));
            }
        }

        if update {
            ctx.actions.schedule(|project_editor, ctx| {
                project_editor.update_theme(ctx);
            });
        }

        cheese_response
    }
}
