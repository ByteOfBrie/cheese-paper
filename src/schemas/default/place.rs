use crate::components::file_objects::FileObjectStore;
use crate::components::file_objects::utils::{metadata_extract_string, write_outline_property};
use crate::components::file_objects::{BaseFileObject, FileObject};
use crate::components::text::Text;
use crate::schemas::FileType;
use crate::util::CheeseError;

use crate::ui::FileObjectEditor;
use crate::ui::prelude::*;

use crate::ford_get;
use crate::schemas::FileTypeInfo;

use egui::Id;
use egui::ScrollArea;

#[derive(Debug, Default)]
pub struct PlaceMetadata {
    pub connection: Text,
    pub description: Text,
    pub appearance: Text,
    pub other_senses: Text,
    pub notes: Text,
}

#[derive(Debug)]
pub struct Place {
    pub base: BaseFileObject,
    pub metadata: PlaceMetadata,
}

#[derive(Debug, Default)]
struct RenderData {
    name_box: NameBox,
}

impl Place {
    pub const IDENTIFIER: &'static str = "worldbuilding";

    pub const TYPE_INFO: FileTypeInfo = FileTypeInfo {
        identifier: Self::IDENTIFIER,
        is_folder: true,
        has_body: false,
        type_name: "Place",
        empty_string_name: "New Place",
        extension: "toml",
        description: "A folder corresponding to a specific place",
    };

    pub fn from_base(base: BaseFileObject) -> Result<Self, CheeseError> {
        let mut place = Self {
            base,
            metadata: Default::default(),
        };

        match place.load_metadata() {
            Ok(modified) => {
                if modified {
                    place.base.file.modified = true;
                }
            }
            Err(err) => {
                log::error!(
                    "Error while loading object-specific metadata for {:?}: {}",
                    place.base.file,
                    &err
                );
                return Err(err);
            }
        }

        Ok(place)
    }
}

impl FileObject for Place {
    fn get_type(&self) -> FileType {
        &Self::TYPE_INFO
    }

    fn get_schema(&self) -> &'static dyn crate::components::Schema {
        &super::DEFAULT_SCHEMA
    }

    fn load_metadata(&mut self) -> Result<bool, CheeseError> {
        let mut modified = false;

        match metadata_extract_string(self.base.toml_header.as_table(), "connection")? {
            Some(connection) => self.metadata.connection = connection.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "description")? {
            Some(description) => self.metadata.description = description.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "appearance")? {
            Some(appearance) => self.metadata.appearance = appearance.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "other_senses")? {
            Some(other_senses) => self.metadata.other_senses = other_senses.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "notes")? {
            Some(notes) => self.metadata.notes = notes.into(),
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
        self.base.toml_header["connection"] = toml_edit::value(&*self.metadata.connection);
        self.base.toml_header["description"] = toml_edit::value(&*self.metadata.description);
        self.base.toml_header["appearance"] = toml_edit::value(&*self.metadata.appearance);
        self.base.toml_header["other_senses"] = toml_edit::value(&*self.metadata.other_senses);
        self.base.toml_header["notes"] = toml_edit::value(&*self.metadata.notes);
    }

    fn generate_outline(&self, depth: u64, export_string: &mut String, objects: &FileObjectStore) {
        (self as &dyn FileObject).write_title(depth, export_string);

        write_outline_property("connection", &self.metadata.connection, export_string);
        write_outline_property("description", &self.metadata.description, export_string);
        write_outline_property("appearance", &self.metadata.appearance, export_string);
        write_outline_property("other_senses", &self.metadata.other_senses, export_string);
        write_outline_property("notes", &self.metadata.notes, export_string);

        for child_id in self.get_base().children.iter() {
            objects.get(child_id).unwrap().borrow().generate_outline(
                depth + 1,
                export_string,
                objects,
            );
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
        &mut self.metadata.description
    }
}

impl FileObjectEditor for Place {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        ford_get!(RenderData, rdata, ctx.stores.file_objects, self.id());

        let sidebar_ids = egui::SidePanel::right("metadata sidebar")
            .resizable(true)
            .default_width(200.0)
            .width_range(50.0..)
            .show_inside(ui, |ui| self.show_sidebar(ui, ctx, rdata))
            .inner;

        let mut ids = egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_editor(ui, ctx))
            .inner;

        ids.extend(sidebar_ids);
        ids
    }

    fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str)) {
        f(&self.metadata.connection, "Connection");
        f(&self.metadata.description, "Description");
        f(&self.metadata.appearance, "Appearance");
        f(&self.metadata.other_senses, "Other Senses");
        f(&self.metadata.notes, "notes");
    }

    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.metadata.connection, "Connection");
        f(&mut self.metadata.description, "Description");
        f(&mut self.metadata.appearance, "Appearance");
        f(&mut self.metadata.other_senses, "Other Senses");
        f(&mut self.metadata.notes, "Notes");
    }

    fn provide_spellcheck_additions(&self) -> Vec<&str> {
        if !self.base.metadata.name.is_empty() {
            vec![&self.base.metadata.name]
        } else {
            vec![]
        }
    }
}

impl Place {
    fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
        rdata: &mut RenderData,
    ) -> Vec<Id> {
        let mut ids = Vec::new();

        ScrollArea::vertical()
            .id_salt("main metadata")
            .show(ui, |ui| {
                let (modified, nb_ids) = rdata.name_box.ui(
                    &mut self.get_base_mut().metadata.name,
                    "Unnamed Place",
                    ui,
                    ctx,
                );
                self.get_base_mut().file.modified |= modified;
                ids.extend(nb_ids);

                ui.label("Notes");
                let response = ui.add_sized(ui.available_size(), |ui: &'_ mut Ui| {
                    self.metadata.notes.ui(ui, ctx)
                });
                self.process_response(&response);
                ids.push(response.id);
            });
        ids
    }

    fn show_editor(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        let mut ids = Vec::new();

        ScrollArea::vertical()
            .id_salt("main metadata")
            .show(ui, |ui| {
                ui.label("Connection To Story");
                let response = ui.add(|ui: &'_ mut Ui| self.metadata.connection.ui(ui, ctx));
                self.process_response(&response);
                ids.push(response.id);

                ui.label("Description");
                let response = ui.add(|ui: &'_ mut Ui| self.metadata.description.ui(ui, ctx));
                self.process_response(&response);
                ids.push(response.id);

                ui.label("Appearance");
                let response = ui.add(|ui: &'_ mut Ui| self.metadata.appearance.ui(ui, ctx));
                self.process_response(&response);
                ids.push(response.id);

                ui.label("Other Senses");
                let response = ui.add(|ui: &'_ mut Ui| self.metadata.other_senses.ui(ui, ctx));
                self.process_response(&response);
                ids.push(response.id);
            });
        ids
    }
}
