use crate::components::file_objects::FileObjectStore;
use crate::components::file_objects::utils::{
    metadata_extract_string, metadata_extract_u64, write_outline_property,
};
use crate::components::file_objects::{BaseFileObject, FileObject};
use crate::components::file_objects::{CompileStatus, IncludeOptions};
use crate::components::project::ExportOptions;
use crate::components::text::Text;
use crate::schemas::FileType;
use crate::util::CheeseError;

use crate::ui::FileObjectEditor;
use crate::ui::prelude::*;

use crate::ford_get;
use crate::schemas::FileTypeInfo;

use egui::ScrollArea;

#[derive(Debug, Default)]
pub struct FolderMetadata {
    pub summary: Text,
    pub notes: Text,
    pub compile_status: CompileStatus,
}

#[derive(Debug)]
pub struct Folder {
    pub base: BaseFileObject,
    pub metadata: FolderMetadata,
}

impl Folder {
    pub const IDENTIFIER: &'static str = "folder";

    pub const TYPE_INFO: FileTypeInfo = FileTypeInfo {
        identifier: Self::IDENTIFIER,
        is_folder: true,
        has_body: false,
        type_name: "Folder",
        empty_string_name: "New Folder",
        extension: "toml",
        description: "A generic folder",
    };

    pub fn from_base(base: BaseFileObject) -> Result<Self, CheeseError> {
        let mut folder = Self {
            base,
            metadata: Default::default(),
        };

        let modified = folder.load_metadata().map_err(|err| {
            cheese_error!(
                "Error while loading object-specific metadata for {:?}:\n{}",
                folder.base.file,
                err
            )
        })?;

        if modified {
            folder.base.file.modified = true;
        }

        Ok(folder)
    }
}

impl FileObject for Folder {
    fn get_type(&self) -> FileType {
        &Self::TYPE_INFO
    }

    fn get_schema(&self) -> &'static dyn crate::components::Schema {
        &super::DEFAULT_SCHEMA
    }

    fn load_metadata(&mut self) -> Result<bool, CheeseError> {
        let mut modified = false;

        match metadata_extract_string(self.base.toml_header.as_table(), "summary")? {
            Some(value) => self.metadata.summary = value.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "notes")? {
            Some(notes) => self.metadata.notes = notes.into(),
            None => modified = true,
        }

        match metadata_extract_u64(self.base.toml_header.as_table(), "compile_status", true)? {
            Some(compile_status) => {
                self.metadata.compile_status = CompileStatus::from_bits_retain(compile_status)
            }
            None => modified = true,
        }

        Ok(modified)
    }

    fn load_body(&mut self, _data: String) {}
    fn get_body(&self) -> String {
        String::new()
    }

    fn get_base(&self) -> &BaseFileObject {
        &self.base
    }

    fn get_base_mut(&mut self) -> &mut BaseFileObject {
        &mut self.base
    }

    fn write_metadata(&mut self, _objects: &FileObjectStore) {
        self.base.toml_header["summary"] = toml_edit::value(&*self.metadata.summary);
        self.base.toml_header["notes"] = toml_edit::value(&*self.metadata.notes);
        self.base.toml_header["compile_status"] =
            toml_edit::value(self.metadata.compile_status.bits() as i64);
    }

    fn generate_outline(&self, depth: u64, export_string: &mut String, objects: &FileObjectStore) {
        (self as &dyn FileObject).write_title(depth, export_string);

        write_outline_property("summary", &self.metadata.summary, export_string);
        write_outline_property("notes", &self.metadata.notes, export_string);

        for child_id in self.get_base().children.iter() {
            objects.get(child_id).unwrap().borrow().generate_outline(
                depth + 1,
                export_string,
                objects,
            );
        }
    }

    fn generate_export(
        &self,
        depth: u64,
        export_string: &mut String,
        objects: &FileObjectStore,
        export_options: &ExportOptions,
        include_break: bool,
    ) -> bool {
        if self
            .metadata
            .compile_status
            .contains(CompileStatus::INCLUDE)
        {
            let display_title = match self.metadata.compile_status.include_title() {
                IncludeOptions::Always => true,
                IncludeOptions::Default => export_options.folder_title_depth.should_display(depth),
                IncludeOptions::Never => false,
            };

            // Keep track of whether the next scene will start with a break, which only ever gets
            // rendered in scenes
            let mut include_break_next = include_break;

            if display_title {
                (self as &dyn FileObject).write_title(depth, export_string);
                // We've written a title, so the requested break has been taken care of
                include_break_next = false;
            }

            // We don't actually have enough information here to decide to include a break, even
            // though it seems like we should. For example, we might have `include_break` set here
            // and no title displayed, but the next scene could actually start with a title, in which
            // case we shouldn't include the break here. Since we don't have any information about
            // what comes next, we just have to wait for the title to be drawn

            for child_id in self.get_base().children.iter() {
                // Keep passing the include_break status forwards along with any updates to it
                include_break_next = objects.get(child_id).unwrap().borrow().generate_export(
                    depth + 1,
                    export_string,
                    objects,
                    export_options,
                    include_break_next,
                );
            }

            let folder_include_break_next = match self.metadata.compile_status.break_at_end() {
                IncludeOptions::Always => true,
                IncludeOptions::Default => export_options.insert_breaks,
                IncludeOptions::Never => false,
            };

            // Request a break if either the final child or this folder should have a break
            include_break_next || folder_include_break_next
        } else {
            include_break
        }
    }

    fn as_editor(&self) -> &dyn crate::ui::FileObjectEditor {
        self
    }

    fn as_editor_mut(&mut self) -> &mut dyn crate::ui::FileObjectEditor {
        self
    }

    #[cfg(test)]
    fn get_test_field(&mut self) -> &mut String {
        &mut self.metadata.notes
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum Tab {
    #[default]
    Notes,
    Export,
}

#[derive(Debug, Default)]
struct RenderData {
    tab: Tab,
    name_box: NameBox,
}

impl FileObjectEditor for Folder {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let cheese_response = egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_editor(ui, ctx))
            .inner;
        self.process_response(&cheese_response);
        cheese_response
    }

    fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str)) {
        f(&self.metadata.summary, "Summary");
        f(&self.metadata.notes, "Notes");
    }

    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.metadata.summary, "Summary");
        f(&mut self.metadata.notes, "Notes");
    }
}

impl Folder {
    fn show_editor(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        ford_get!(RenderData, rdata, ctx.stores.file_objects, self.id());

        let mut cheese_response = CheeseResponse::default();

        // Tab selection
        ui.horizontal(|ui| {
            ui.selectable_value(&mut rdata.tab, Tab::Notes, "Summary/Notes");
            ui.selectable_value(&mut rdata.tab, Tab::Export, "Export");
        });

        ui.separator();

        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {

            cheese_response.extend(rdata.name_box.ui(
                    &mut self.get_base_mut().metadata.name,
                    "Unnamed Folder", ui, ctx));

            match rdata.tab {
                Tab::Notes => {
                    egui::CollapsingHeader::new("Summary")
                        .default_open(true)
                        .show(ui, |ui| {
                            let response =
                                ui.add(|ui: &'_ mut Ui| self.metadata.summary.ui(ui, ctx));
                            cheese_response.process_response(&response, true);
                        });

                    egui::CollapsingHeader::new("Notes")
                        .default_open(true)
                        .show(ui, |ui| {
                            let response = ui.add(|ui: &'_ mut Ui| self.metadata.notes.ui(ui, ctx));
                            cheese_response.process_response(&response, true);
                        });
                }
                Tab::Export => {
                    // Check box for including this file entirely
                    let mut export_include = self
                        .metadata
                        .compile_status
                        .contains(CompileStatus::INCLUDE);

                    let response = ui.checkbox(&mut export_include, "Include in export");
                    if response.changed() {
                        self.metadata
                            .compile_status
                            .set(CompileStatus::INCLUDE, export_include);
                    }
                    cheese_response.process_response(&response, true);

                    // The rest of the checkboxes have no effect if export isn't included
                    ui.add_enabled_ui(export_include, |ui| {
                        let mut include_title = self.metadata.compile_status.include_title();
                        let include_title_before = include_title;

                        ui.horizontal(|ui| {
                            const INCLUDE_TITLE_MESSAGE: &str = "If the title of this folder/scene will be included
                            default - this will come from the settings in the export tab
                            always - include the title for this, even if the project export settings differ
                            never - do not include the title for this, even if the export settings differ";

                            ui.label("Include Title  ℹ").on_hover_text(INCLUDE_TITLE_MESSAGE);

                            let title_combobox_response =
                                egui::ComboBox::from_id_salt("Include Title")
                                    .selected_text(format!("{include_title:?}"))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut include_title,
                                            IncludeOptions::Default,
                                            "Default",
                                        );
                                        ui.selectable_value(
                                            &mut include_title,
                                            IncludeOptions::Always,
                                            "Always",
                                        );
                                        ui.selectable_value(
                                            &mut include_title,
                                            IncludeOptions::Never,
                                            "Never",
                                        );
                                    });

                            // We want to be able to tab to the box, but it doesn't get a process_response
                            // call because that needs to be handled below
                            cheese_response.tabable_ids.push(title_combobox_response.response.id);
                        });

                        // We don't have an actual response here so we have to manually process
                        if include_title != include_title_before {
                            self.metadata
                                .compile_status
                                .set_include_title(include_title);
                            self.get_base_mut().file.modified = true;
                        }

                        // same thing but for the break
                        let mut break_at_end = self.metadata.compile_status.break_at_end();
                        let break_at_end_before = break_at_end;

                        ui.horizontal(|ui| {
                            const INCLUDE_BREAK_MESSAGE: &str = "If this is followed by a scene, should there be a divider?
                            default - this will come from the settings in the export tab
                            always - include a divider after this, even if the project export settings differ
                            never - do not include a divider after this, even if the export settings differ";

                            ui.label("Break at End  ℹ").on_hover_text(INCLUDE_BREAK_MESSAGE);

                            let break_combobox_response =
                                egui::ComboBox::from_id_salt("Break at End")
                                    .selected_text(format!("{break_at_end:?}"))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut break_at_end,
                                            IncludeOptions::Default,
                                            "Default",
                                        );
                                        ui.selectable_value(
                                            &mut break_at_end,
                                            IncludeOptions::Always,
                                            "Always",
                                        );
                                        ui.selectable_value(
                                            &mut break_at_end,
                                            IncludeOptions::Never,
                                            "Never",
                                        );
                                    });

                            // We want to be able to tab to the box, but it doesn't get a process_response
                            // call because that needs to be handled below
                            cheese_response.tabable_ids.push(break_combobox_response.response.id);
                        });

                        // We don't have an actual response here so we have to manually process
                        if break_at_end != break_at_end_before {
                            self.metadata.compile_status.set_break_at_end(break_at_end);
                            self.get_base_mut().file.modified = true;
                        }
                    });
                }
            }
        });
        cheese_response
    }
}
