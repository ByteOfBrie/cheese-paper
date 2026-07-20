use std::{
    collections::VecDeque,
    sync::OnceLock,
    time::{Duration, SystemTime},
};

use crate::ui::settings::Setting;
use crate::ui::{prelude::*, settings::dictionaries};

use egui::{Color32, RichText};

use super::ThemeSelection;

/// The only two things that are still stored in the settings page
/// rather than the main settings option. We should eventually move these over,
/// but this is generally fine
#[derive(Debug, Default)]
pub struct SettingsPage {
    random_theme_name: String,

    random_theme_save_error: Option<CheeseError>,
}

impl Setting<bool> {
    fn ui<'a>(&'a mut self, ui: &mut Ui, atoms: &'a str, project_local: bool) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();

        ui.horizontal(|ui| {
            let response = ui.button("⟲");
            response.widget_info(|| {
                WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    format!("reset {atoms}"),
                )
            });
            if response.clicked() {
                self.reset_value(project_local);
                // The egui response doesn't show as "changed", so we set it manually
                cheese_response.modified = true;
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
        description: &'static str,
        options: impl Iterator<Item = T>,
        project_local: bool,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        let inner_response = egui::ComboBox::from_id_salt(description)
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

        inner_response.response.widget_info(|| {
            WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), description)
        });

        cheese_response.tabable_ids.push(inner_response.response.id);

        cheese_response
    }
}

static GLOBAL_SETTINGS_LOCATION: OnceLock<String> = OnceLock::new();

impl SettingsPage {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
        project_local: bool,
    ) -> CheeseResponse {
        layout::margin_box(ui, |ui| {
            let mut cheese_response = CheeseResponse::default();

            if project_local {
                layout::heading(ui, "Project Settings");

                cheese_response.extend(self.project_local_settings_ui(ui, ctx));
            } else {
                layout::heading(ui, "Settings");

                cheese_response.extend(self.settings_ui(ui, ctx));

                ui.separator();

                layout::heading(ui, "Themes");

                cheese_response.extend(self.themes_ui(ui, ctx));

                if cheese_response.modified {
                    let mut settings_data = ctx.settings.data.borrow_mut();

                    // the decision was made through a rigorous design process known as "coding crimes chicken".
                    // a reasonable compromise between 250 and 400 was not good enough. this
                    // had the unfortunate consequence of a particular binary being subpar
                    // for 50% of the current active users at all times
                    // thankfully, we can use basic heuristics to determine the optimal
                    // APPLY_DELAY for all (current) users of cheese-paper.
                    static APPLY_DELAY: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();

                    settings_data.next_apply = Some(
                        SystemTime::now()
                            + *APPLY_DELAY.get_or_init(|| {
                                if settings_data
                                    .available_dict
                                    .iter()
                                    .any(|dict| dict.name.contains("fr"))
                                {
                                    Duration::from_millis(400)
                                } else {
                                    Duration::from_millis(250)
                                }
                            }),
                    );

                    ui.ctx().request_repaint_after(*APPLY_DELAY.get().unwrap());
                }
            }

            cheese_response
        })
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut settings_data = ctx.settings.data.borrow_mut();

        let settings_location = GLOBAL_SETTINGS_LOCATION.get_or_init(|| {
            format!(
                "global settings file: {:?}",
                settings_data.config_file_path()
            )
        });

        ui.small(settings_location);

        let mut cheese_response = CheeseResponse::default();

        let font_size_label = ui.label("Font Size");

        let response = ui.text_edit_singleline(&mut settings_data.font_size.interface_value);
        cheese_response.process_response(&response, true);
        response.labelled_by(font_size_label.id);

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

        #[cfg(feature = "update_checking")]
        {
            let response = settings_data
                .check_for_updates
                .ui(ui, "Check for Updates", false);
            cheese_response.extend(response);
        }

        let response = settings_data
            .custom_tab_behavior
            .ui(ui, "Custom Tab Behavior", false);
        cheese_response.extend(response);

        let response = settings_data
            .reopen_last
            .ui(ui, "Reopen Last Project on Launch", false);
        cheese_response.extend(response);

        ui.horizontal(|ui| {
            ui.label("Dictionary: ");

            let mut options: VecDeque<_> = settings_data
                .available_dict
                .iter()
                .map(|entry| entry.name.clone())
                .collect();

            options.push_front(dictionaries::SELECTED_NONE.to_owned());

            cheese_response.extend(settings_data.selected_dictionary.dropdown(
                ui,
                "Dictionary",
                options.into_iter(),
                false,
            ));
        });

        if let Some(err_msg) = &settings_data.selected_dictionary.error_message {
            ui.label(RichText::new(err_msg).color(Color32::RED));
        }

        if ui.button("Open Dictionary Folder").clicked()
            && let Err(err) = open::that(settings_data.settings_path.join("dictionaries"))
        {
            log::warn!("Could not open dictionary folder: {err}");
        }

        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Open Log Folder").clicked()
                && let Err(err) = open::that(ctx.data.get_directory().join("logs"))
            {
                log::warn!("Could not open log folder: {err}");
            }

            if ui.button("Open Data Folder").clicked()
                && let Err(err) = open::that(ctx.data.get_directory())
            {
                log::warn!("Could not open data folder: {err}");
            }
        });

        cheese_response
    }

    fn project_local_settings_ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
    ) -> CheeseResponse {
        let mut settings_data = ctx.settings.data.borrow_mut();

        let mut cheese_response = CheeseResponse::default();

        let response = settings_data.highlight_multiple_spaces.ui(
            ui,
            "Highlight Multiple Spaces (in a Row)",
            true,
        );
        cheese_response.extend(response);

        let response = settings_data.highlight_spaces_before_punctuation.ui(
            ui,
            "Highlight Spaces Between a Word and Punctuation",
            true,
        );
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
            "Dictionary",
            options.into_iter(),
            true,
        ));

        if let Some(err_msg) = &settings_data.selected_dictionary.error_message {
            ui.label(RichText::new(err_msg).color(Color32::RED));
        }

        if cheese_response.modified {
            const APPLY_DELAY: Duration = Duration::from_millis(400);

            settings_data.pl_next_apply = Some(SystemTime::now() + APPLY_DELAY);
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
            response.widget_info(|| {
                WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    response.enabled(),
                    "Default theme",
                )
            });
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
            response.widget_info(|| {
                WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), "Light theme")
            });
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
            response.widget_info(|| {
                WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), "Random theme")
            });
            cheese_response.tabable_ids.push(response.id);
        });

        layout::horizontal_heading(ui, |ui| {
            layout::heading(ui, "Available Presets");
            let reload_button = ui.button("reload");
            reload_button.widget_info(|| {
                WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    reload_button.enabled(),
                    "reload themes",
                )
            });
            if reload_button.clicked() {
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
                    response.widget_info(|| {
                        WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            response.enabled(),
                            format!("{name} theme"),
                        )
                    });
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
            ui.label("Save random theme as preset?");
            ui.horizontal(|ui| {
                ui.label("name: ");
                let response = ui.text_edit_singleline(&mut self.random_theme_name);
                response.widget_info(|| {
                    WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        response.enabled(),
                        "random theme save name",
                    )
                });
                cheese_response.tabable_ids.push(response.id);
                let response = ui.button("Save");
                response.widget_info(|| {
                    WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        response.enabled(),
                        "Save random theme",
                    )
                });
                if response.clicked() {
                    self.random_theme_save_error = ctx
                        .settings
                        .save_current_theme(&self.random_theme_name)
                        .err();
                    // if the theme saved successfully, load the config and change the theme
                    if self.random_theme_save_error.is_none() {
                        let random_theme_name = self.random_theme_name.clone();
                        ctx.actions.schedule(move |project_editor, ctx| {
                            match project_editor.editor_context.settings.load() {
                                Ok(()) => {
                                    let mut theme_index_option = None;
                                    for (index, (theme_name, _)) in project_editor
                                        .editor_context
                                        .settings
                                        .data
                                        .borrow()
                                        .available_themes
                                        .iter()
                                        .enumerate()
                                    {
                                        if *theme_name == random_theme_name {
                                            theme_index_option = Some(index);
                                        }
                                    }
                                    if let Some(theme_index) = theme_index_option {
                                        project_editor
                                            .editor_context
                                            .settings
                                            .select_theme(ThemeSelection::Preset(theme_index))
                                            .unwrap();
                                    }
                                    project_editor.update_theme(ctx);
                                }
                                Err(err) => {
                                    log::error!("Error encountered while reloading settings: {err}")
                                }
                            }
                        });
                        update = true;
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
            cheese_response.modified = true;
        }

        cheese_response
    }
}
