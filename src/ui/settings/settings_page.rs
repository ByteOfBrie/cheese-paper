use std::{
    collections::VecDeque,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use crate::ui::settings::Setting;
use crate::ui::{prelude::*, settings::dictionaries};

use egui::{Color32, RichText};

use super::ThemeSelection;

#[derive(Debug)]
pub struct SettingsPage {
    random_theme_name: String,

    random_theme_save_error: Option<CheeseError>,
}

impl Setting<bool> {
    fn ui<'a>(
        &'a mut self,
        ui: &mut Ui,
        atoms: impl egui::IntoAtoms<'a>,
        project_local: bool,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();

        ui.horizontal(|ui| {
            let response = ui.button("⟲");
            if response.clicked() {
                self.reset_value(project_local);
            }
            cheese_response.process_response(&response, true);

            if self.value(project_local).is_none() {
                ui.set_opacity(0.5);
            } else {
                ui.set_opacity(1.0);
            }

            let response = ui.checkbox(self.interface_value(project_local), atoms);
            cheese_response.process_response(&response, true);
        });
        cheese_response
    }
}

impl<T: PartialEq + Clone + AsRef<str> + std::fmt::Debug> Setting<T> {
    fn dropdown(
        &mut self,
        ui: &mut Ui,
        id_salt: &'static str,
        options: impl Iterator<Item = T>,
        project_local: bool,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(self.interface_value(project_local).as_ref())
            .show_ui(ui, |ui| {
                for option in options {
                    let response = ui.selectable_value(
                        self.interface_value(project_local),
                        option.clone(),
                        option.as_ref(),
                    );
                    cheese_response.process_response(&response, false);
                }
            });

        cheese_response
    }
}

static GLOBAL_SETTINGS_LOCATION: OnceLock<String> = OnceLock::new();

impl SettingsPage {
    // TODO: maybe get rid of this
    pub fn load(_ctx: &mut EditorContext) -> Self {
        Self {
            random_theme_name: String::new(),
            random_theme_save_error: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        layout::margin_box(ui, |ui| {
            let mut cheese_response = CheeseResponse::default();

            layout::heading(ui, "Settings");

            cheese_response.extend(self.settings_ui(ui, ctx));

            ui.separator();

            layout::heading(ui, "Themes");

            cheese_response.extend(self.themes_ui(ui, ctx));

            // TODO: maybe parse cheese response here? might need to make sure themes are good

            cheese_response
        })
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut settings_data = ctx.settings.0.borrow_mut();

        let settings_location = GLOBAL_SETTINGS_LOCATION.get_or_init(|| {
            format!(
                "global settings file: {:?}",
                settings_data.config_file_path()
            )
        });

        ui.small(settings_location);

        let mut cheese_response = CheeseResponse::default();

        ui.label("Font Size");

        let response = ui.text_edit_singleline(&mut settings_data.font_size.interface_value);
        cheese_response.process_response(&response, true);

        if let Some(err) = &settings_data.font_size.error_message {
            ui.label(RichText::new(err).color(Color32::RED));
        }

        let response = settings_data
            .indent_line_start
            .ui(ui, "Indent Line Start", false);
        cheese_response.extend(response);

        let response = settings_data.highlight_multiple_spaces.ui(
            ui,
            "Highlight Multiple Spaces (in a Row)",
            false,
        );
        cheese_response.extend(response);

        let response = settings_data.highlight_spaces_before_punctuation.ui(
            ui,
            "Highlight Spaces Between a Word and Punctuation",
            false,
        );
        cheese_response.extend(response);

        let response = settings_data
            .check_for_updates
            .ui(ui, "Check for Updates", false);
        cheese_response.extend(response);

        let response = settings_data
            .reopen_last
            .ui(ui, "Reopen Last Project on Launch", false);
        cheese_response.extend(response);

        ui.label("Dictionary");

        let mut options: VecDeque<_> = settings_data
            .available_dict
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        options.push_front(dictionaries::SELECTED_NONE.to_owned());

        cheese_response.extend(settings_data.selected_dictionary.dropdown(
            ui,
            "Dictionary Selection Dropdown",
            options.into_iter(),
            false,
        ));

        if let Some(err_msg) = &settings_data.selected_dictionary.error_message {
            ui.label(RichText::new(err_msg).color(Color32::RED));
        }

        if cheese_response.modified {
            // I'm changing this duration back to 400
            // you can change it back to 250 if you want
            // your move
            const APPLY_DELAY: Duration = Duration::from_millis(400);

            settings_data.next_apply = Some(SystemTime::now() + APPLY_DELAY);
            ui.ctx().request_repaint_after(APPLY_DELAY);
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

        layout::horizontal_heading(ui, |ui| {
            layout::heading(ui, "Available Presets");
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
