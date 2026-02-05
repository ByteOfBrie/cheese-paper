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
pub struct CharacterMetadata {
    pub summary: Text,
    pub notes: Text,

    pub goal: Text,

    pub appearance: Text,

    // how the character's personality looks to the outside world
    pub persona: Text,

    // what's going on in the character's head
    pub interior: Text,

    // what the character does in the story
    pub role: Text,
}

#[derive(Debug)]
pub struct Character {
    pub base: BaseFileObject,
    pub metadata: CharacterMetadata,
}

#[derive(Debug, Default)]
struct RenderData {
    name_box: NameBox,
}

impl Character {
    pub const IDENTIFIER: &'static str = "character";

    pub const TYPE_INFO: FileTypeInfo = FileTypeInfo {
        identifier: Self::IDENTIFIER,
        is_folder: false,
        has_body: false,
        type_name: "Character",
        empty_string_name: "New Character",
        extension: "toml",
        description: "An info sheet for characters",
    };

    pub fn from_base(base: BaseFileObject) -> Result<Self, CheeseError> {
        let mut character = Self {
            base,
            metadata: Default::default(),
        };

        match character.load_metadata() {
            Ok(modified) => {
                if modified {
                    character.base.file.modified = true;
                }
            }
            Err(err) => {
                log::error!(
                    "Error while loading object-specific metadata for {:?}: {}",
                    character.base.file,
                    &err
                );
                return Err(err);
            }
        }

        Ok(character)
    }
}

impl FileObject for Character {
    fn get_type(&self) -> FileType {
        &Self::TYPE_INFO
    }

    fn get_schema(&self) -> &'static dyn crate::components::Schema {
        &super::OVERTHINKER_SCHEMA
    }

    fn load_metadata(&mut self) -> Result<bool, CheeseError> {
        let mut modified = false;

        match metadata_extract_string(self.base.toml_header.as_table(), "summary")? {
            Some(summary) => self.metadata.summary = summary.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "notes")? {
            Some(notes) => self.metadata.notes = notes.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "goal")? {
            Some(goal) => self.metadata.goal = goal.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "appearance")? {
            Some(appearance) => self.metadata.appearance = appearance.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "persona")? {
            Some(persona) => self.metadata.persona = persona.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "interior")? {
            Some(interior) => self.metadata.interior = interior.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "role")? {
            Some(role) => self.metadata.role = role.into(),
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
        self.base.toml_header["appearance"] = toml_edit::value(&*self.metadata.appearance);
        self.base.toml_header["persona"] = toml_edit::value(&*self.metadata.persona);
        self.base.toml_header["goal"] = toml_edit::value(&*self.metadata.goal);
        self.base.toml_header["interior"] = toml_edit::value(&*self.metadata.interior);
        self.base.toml_header["role"] = toml_edit::value(&*self.metadata.role);
    }

    fn generate_outline(&self, depth: u64, export_string: &mut String, _objects: &FileObjectStore) {
        (self as &dyn FileObject).write_title(depth, export_string);

        write_outline_property("summary", &self.metadata.summary, export_string);
        write_outline_property("appearance", &self.metadata.appearance, export_string);
        write_outline_property("persona", &self.metadata.persona, export_string);
        write_outline_property("goal", &self.metadata.goal, export_string);
        write_outline_property("interior", &self.metadata.interior, export_string);
        write_outline_property("role", &self.metadata.role, export_string);
        write_outline_property("notes", &self.metadata.notes, export_string);
    }

    fn as_editor(&self) -> &dyn crate::ui::FileObjectEditor {
        self
    }

    fn as_editor_mut(&mut self) -> &mut dyn crate::ui::FileObjectEditor {
        self
    }

    #[cfg(test)]
    fn get_test_field(&mut self) -> &mut String {
        &mut self.metadata.appearance
    }
}

// shortcuts for not having to cast every time

impl FileObjectEditor for Character {
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
        f(&self.metadata.summary, "summary");
        f(&self.metadata.notes, "notes");
        f(&self.metadata.appearance, "appearance");
        f(&self.metadata.persona, "persona");
        f(&self.metadata.goal, "goal");
        f(&self.metadata.interior, "interior");
        f(&self.metadata.role, "role");
    }

    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.metadata.summary, "summary");
        f(&mut self.metadata.notes, "notes");
        f(&mut self.metadata.appearance, "appearance");
        f(&mut self.metadata.persona, "persona");
        f(&mut self.metadata.goal, "goal");
        f(&mut self.metadata.interior, "interior");
        f(&mut self.metadata.role, "role");
    }

    fn provide_spellcheck_additions(&self) -> Vec<&str> {
        if !self.base.metadata.name.is_empty() {
            vec![&self.base.metadata.name]
        } else {
            vec![]
        }
    }
}

impl Character {
    fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
        rdata: &mut RenderData,
    ) -> Vec<Id> {
        let mut ids = Vec::new();
        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {
            let (modified, nb_ids) = rdata.name_box.ui(
                &mut self.get_base_mut().metadata.name,
                "Unnamed Character",
                ui,
                ctx,
            );
            self.get_base_mut().file.modified |= modified;
            ids.extend(nb_ids);

            let min_height =
                (ui.available_height() / 2.0) - ctx.measurements.collapsible_header_extra_height;

            egui::CollapsingHeader::new("Summary")
                .default_open(true)
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        egui::vec2(ui.available_width(), min_height),
                        |ui: &'_ mut Ui| self.metadata.summary.ui(ui, ctx),
                    );
                    self.process_response(&response);
                    ids.push(response.id);
                });

            egui::CollapsingHeader::new("Notes")
                .default_open(true)
                .show(ui, |ui| {
                    let response = ui.add_sized(
                        egui::vec2(ui.available_width(), min_height),
                        |ui: &'_ mut Ui| self.metadata.notes.ui(ui, ctx),
                    );
                    self.process_response(&response);
                    ids.push(response.id);
                });
        });

        ids
    }

    fn show_editor(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        let mut ids = Vec::new();
        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {
            ui.label("Goals");
            let response: egui::Response = ui.add(|ui: &'_ mut Ui| self.metadata.goal.ui(ui, ctx));
            self.process_response(&response);
            ids.push(response.id);

            ui.label("Appearance");
            let response: egui::Response =
                ui.add(|ui: &'_ mut Ui| self.metadata.appearance.ui(ui, ctx));
            self.process_response(&response);
            ids.push(response.id);

            ui.label("Persona");
            let response: egui::Response =
                ui.add(|ui: &'_ mut Ui| self.metadata.persona.ui(ui, ctx));
            self.process_response(&response);
            ids.push(response.id);

            ui.label("Interior");
            let response: egui::Response =
                ui.add(|ui: &'_ mut Ui| self.metadata.interior.ui(ui, ctx));
            self.process_response(&response);
            ids.push(response.id);

            ui.label("Narrative Role");
            let response: egui::Response = ui.add(|ui: &'_ mut Ui| self.metadata.role.ui(ui, ctx));
            self.process_response(&response);
            ids.push(response.id);
        });
        ids
    }
}
