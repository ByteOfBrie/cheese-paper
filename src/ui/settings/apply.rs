use crate::components::file_objects::utils::{create_dir_if_missing, write_with_temp_file};

use crate::ui::editor_base::configure_text_styles;
use crate::ui::{CheesePaperApp, prelude::*, settings::SettingsData};

impl CheesePaperApp {
    pub fn process_settings_changes(&mut self, ctx: &egui::Context) -> Result<(), CheeseError> {
        let (needs_processing, project_local) = self.state.settings.need_processing(ctx);

        if !needs_processing {
            return Ok(());
        }

        let mut settings = self.state.settings.clone();
        let mut data = settings.data.borrow_mut();

        data.update_values(project_local);

        self.apply_settings(&mut data, ctx);

        if project_local {
            if let Some(project_editor) = &mut self.project_editor {
                // Save functions will not touch the TOML keys that they don't know.
                // This lets us share a TOML file with the project save function without worrying about it

                if data.save(&mut project_editor.project.toml_header, project_local) {
                    write_with_temp_file(
                        project_editor.project.get_project_info_file(),
                        project_editor.project.toml_header.to_string(),
                    )
                    .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;
                }
            }
        } else {
            // Only write if we have changes
            if data.save(&mut settings.toml, project_local) {
                write_with_temp_file(
                    create_dir_if_missing(&data.config_file_path())?,
                    settings.toml.to_string(),
                )
                .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;
            }
        }

        Ok(())
    }

    fn apply_settings(&mut self, data: &mut SettingsData, ctx: &egui::Context) {
        if data.font_size.modified {
            configure_text_styles(ctx, *data.font_size.get_value());
        }

        if let Some(project_editor) = &mut self.project_editor {
            project_editor.apply_settings(data);
        }
    }
}

impl ProjectEditor {
    fn apply_settings(&mut self, data: &mut SettingsData) {
        if data.selected_dictionary.modified {
            self.editor_context.dictionary_state.dictionary = data.load_dictionary();
        }

        self.editor_context.render_version += 1;
    }
}
