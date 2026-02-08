use egui::Vec2;
use rfd::FileDialog;

use crate::{
    components::{
        file_objects::utils::process_name_for_filename,
        project::{ExportDepth, ExportOptions},
    },
    ui::prelude::*,
};

//This probably shouldn't be a part of Project but it's easy enough right now
impl Project {
    pub fn export_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let cheese_response = egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_export_selection(ui, ctx))
            .inner;
        if cheese_response.modified {
            self.file.modified = true;
        }
        cheese_response
    }

    fn show_export_selection(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
        ui.label("Project Export Selection");

        egui::Grid::new("Export Options")
            .num_columns(2).spacing(Vec2{x: 5.0, y:10.0})
            .show(ui, |ui| {
                let response = ui.checkbox(
                    &mut self.metadata.export.include_all_folder_titles,
                    "Include All Folder Titles",
                )
                .on_hover_text(
                    "If this is checked, the title from every folder will be included \
                    in the export (as headings)",
                );
                cheese_response.process_response(&response, true);
                ui.end_row();

                const FOLDER_DEPTH_MESSAGE: &str = "If the previous checkbox is unset, this sets the \
                    max depth in the tree where folders will have their titles included (as headings).
                    0 means no folders will have their titles included as headings
                    1 means that only top level folders will have their titles included
                    2 means that folders at the top level or directly inside top level folders";

                ui.add_enabled_ui(!self.metadata.export.include_all_folder_titles, |ui| {
                    ui.label("Include Folder Title Depth  ℹ")
                        .on_disabled_hover_text(FOLDER_DEPTH_MESSAGE)
                        .on_hover_text(FOLDER_DEPTH_MESSAGE);
                });

                // Same enable conditions, but in a separate block so egui can do the grid properly
                ui.add_enabled_ui(!self.metadata.export.include_all_folder_titles, |ui| {
                    let response = ui.add(egui::DragValue::new(
                        &mut self.metadata.export.include_folder_title_depth,
                    ));
                    cheese_response.process_response(&response, true);
                });
                ui.end_row();


                let response = ui.checkbox(
                    &mut self.metadata.export.include_all_scene_titles,
                    "Include All Scene Titles",
                )
                .on_hover_text(
                    "If checked, the title of every scene will be included \
                    in the export (as headings)",
                );
                cheese_response.process_response(&response, true);
                ui.end_row();

                const SCENE_DEPTH_MESSAGE: &str = "If the previous checkbox is unset, this sets the \
                    max depth in the tree where scenes will have their titles included (as headings).
                    0 means no scenes will have their titles included as headings
                    1 means that only top level scenes will have their titles included
                    2 means that scenes at the top level or directly inside top level folders";

                ui.add_enabled_ui(!self.metadata.export.include_all_scene_titles, |ui| {
                    ui.label("Include Scene Title Depth  ℹ")
                        .on_disabled_hover_text(SCENE_DEPTH_MESSAGE)
                        .on_hover_text(SCENE_DEPTH_MESSAGE);
                });

                // Same enable conditions, but in a separate block so egui can do the grid properly
                ui.add_enabled_ui(!self.metadata.export.include_all_scene_titles, |ui| {
                    let response = ui.add(egui::DragValue::new(
                        &mut self.metadata.export.include_scene_title_depth,
                    ));
                    cheese_response.process_response(&response, true);
                });
                ui.end_row();

                let response = ui.checkbox(
                    &mut self.metadata.export.insert_break_at_end,
                    "Insert break between consecutive scenes",
                ).on_hover_text("If checked, insert break (horizontal line) between scenes. If this is \
                    not set, two consecutive scenes will only have a newline in the final export");
                cheese_response.process_response(&response, true);
            });

        ui.add_space(40.0);

        let export_story_button_response = ui.button("Export Story Text");

        if export_story_button_response.clicked() {
            let project_title = &self.base_metadata.name;
            let suggested_title = format!("{}.md", process_name_for_filename(project_title));
            let export_location_option = FileDialog::new()
                .set_title(format!("Export {project_title}"))
                .set_directory(&ctx.last_export_folder)
                .set_file_name(suggested_title)
                .save_file();

            let folder_title_depth = if self.metadata.export.include_all_folder_titles {
                ExportDepth::All
            } else if self.metadata.export.include_folder_title_depth == 0 {
                ExportDepth::None
            } else {
                ExportDepth::Some(self.metadata.export.include_folder_title_depth)
            };

            let scene_title_depth = if self.metadata.export.include_all_scene_titles {
                ExportDepth::All
            } else if self.metadata.export.include_scene_title_depth == 0 {
                ExportDepth::None
            } else {
                ExportDepth::Some(self.metadata.export.include_scene_title_depth)
            };

            let export_options = ExportOptions {
                folder_title_depth,
                scene_title_depth,
                insert_breaks: self.metadata.export.insert_break_at_end,
            };

            if let Some(export_location) = export_location_option {
                let export_contents = self.export_text(export_options);
                if let Err(err) = std::fs::write(&export_location, export_contents) {
                    log::error!("Error while attempting to write outline: {err}");
                }

                ctx.last_export_folder = export_location
                    .parent()
                    .map(|val| val.to_path_buf())
                    .unwrap_or_default();
            }
        }

        cheese_response
            .tabable_ids
            .push(export_story_button_response.id);

        cheese_response
    }
}
