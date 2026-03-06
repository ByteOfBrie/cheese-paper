use crate::components::file_objects::utils::{create_dir_if_missing, write_with_temp_file};

use crate::ui::editor_base::configure_text_styles;
use crate::ui::{CheesePaperApp, prelude::*, settings::SettingsData};

impl CheesePaperApp {
    pub fn process_settings_changes(&mut self, ctx: &egui::Context) -> Result<(), CheeseError> {
        if !self.state.settings.need_processing(ctx) {
            return Ok(());
        }

        let mut settings = self.state.settings.clone();
        let mut data = settings.0.borrow_mut();

        data.update_values();

        self.apply_settings(&mut data, ctx);

        // Only write if we have changes
        if data.save(&mut settings.1) {
            write_with_temp_file(
                create_dir_if_missing(&data.config_file_path())?,
                settings.1.to_string(),
            )
            .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;
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
