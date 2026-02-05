use crate::components::file_objects::FileObjectStore;
use crate::components::file_objects::utils::metadata_extract_string;
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
pub struct NoteMetadata {
    pub subject: Text,
    pub commentary: Text,
}

#[derive(Debug)]
pub struct Note {
    base: BaseFileObject,
    pub metadata: NoteMetadata,
    pub text: Text,
}

#[derive(Debug, Default)]
struct RenderData {
    name_box: NameBox,
}

impl Note {
    pub const IDENTIFIER: &'static str = "note";

    pub const TYPE_INFO: FileTypeInfo = FileTypeInfo {
        identifier: Self::IDENTIFIER,
        is_folder: false,
        has_body: true,
        type_name: "Note",
        empty_string_name: "New Note",
        extension: "md",
        description: "A file with content for writing down notes",
    };

    pub fn from_base(base: BaseFileObject, body: Option<String>) -> Result<Self, CheeseError> {
        let mut scene = Self {
            base,
            metadata: Default::default(),
            text: body.map(|s| s.into()).unwrap_or_default(),
        };

        match scene.load_metadata() {
            Ok(modified) => {
                if modified {
                    scene.base.file.modified = true;
                }
            }
            Err(err) => {
                log::error!(
                    "Error while loading object-specific metadata for {:?}: {}",
                    scene.base.file,
                    &err
                );
                return Err(err);
            }
        }

        Ok(scene)
    }
}

impl FileObject for Note {
    fn get_type(&self) -> FileType {
        &Self::TYPE_INFO
    }

    fn get_schema(&self) -> &'static dyn crate::components::Schema {
        &super::OVERTHINKER_SCHEMA
    }

    fn load_metadata(&mut self) -> Result<bool, CheeseError> {
        let mut modified = false;

        match metadata_extract_string(self.base.toml_header.as_table(), "subject")? {
            Some(summary) => self.metadata.subject = summary.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.base.toml_header.as_table(), "commentary")? {
            Some(notes) => self.metadata.commentary = notes.into(),
            None => modified = true,
        }

        Ok(modified)
    }

    fn load_body(&mut self, data: String) {
        self.text = data.trim().to_string().into();
    }

    fn get_base(&self) -> &BaseFileObject {
        &self.base
    }

    fn get_base_mut(&mut self) -> &mut BaseFileObject {
        &mut self.base
    }

    fn get_body(&self) -> String {
        let mut full_text = String::new();

        for line in self.text.split('\n') {
            full_text.push_str(line.trim());
            full_text.push('\n');
        }

        full_text
    }

    fn write_metadata(&mut self, _objects: &FileObjectStore) {
        self.base.toml_header["subject"] = toml_edit::value(&*self.metadata.subject);
        self.base.toml_header["commentary"] = toml_edit::value(&*self.metadata.commentary);
    }

    fn as_editor(&self) -> &dyn crate::ui::FileObjectEditor {
        self
    }

    fn as_editor_mut(&mut self) -> &mut dyn crate::ui::FileObjectEditor {
        self
    }

    #[cfg(test)]
    fn get_test_field(&mut self) -> &mut String {
        &mut self.metadata.subject
    }
}

impl FileObjectEditor for Note {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        ford_get!(RenderData, rdata, ctx.stores.file_objects, self.id());

        let sidebar_ids = egui::SidePanel::right("metadata sidebar")
            .resizable(true)
            .default_width(200.0)
            .width_range(50.0..)
            .show_inside(ui, |ui| self.show_sidebar(ui, ctx, rdata))
            .inner;

        let mut ids = egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_text_editor(ui, ctx))
            .inner;

        ids.extend(sidebar_ids);
        ids
    }

    fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str)) {
        f(&self.metadata.subject, "subject");
        f(&self.metadata.commentary, "commentary");
        f(&self.text, "text");
    }

    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.metadata.subject, "Subject");
        f(&mut self.metadata.commentary, "Commentary");
        f(&mut self.text, "text");
    }
}

impl Note {
    fn show_text_editor(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        ScrollArea::vertical()
            .id_salt("text")
            .auto_shrink(egui::Vec2b { x: false, y: false })
            .show(ui, |ui| {
                let response =
                    ui.add_sized(ui.available_size(), |ui: &'_ mut Ui| self.text.ui(ui, ctx));

                self.process_response(&response);
                vec![response.id]
            })
            .inner
    }

    fn show_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
        rdata: &mut RenderData,
    ) -> Vec<Id> {
        let mut ids = Vec::new();

        egui::TopBottomPanel::bottom("word_count").show_inside(ui, |ui| {
            ui.add_space(4.0);
            let words = self.text.word_count(ctx);
            let text = format!("{words} Words");
            ui.vertical_centered(|ui| {
                ui.label(text);
            });
        });

        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {
            let (modified, nb_ids) = rdata.name_box.ui(
                &mut self.get_base_mut().metadata.name,
                "Unnamed Note",
                ui,
                ctx,
            );
            self.get_base_mut().file.modified |= modified;
            ids.extend(nb_ids);

            ui.separator();

            self.show_sidebar_metadata(ui, ctx);
        });
        ids
    }

    fn show_sidebar_metadata(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Vec<Id> {
        let mut ids = Vec::new();

        // half of the available height should go to each widget
        let widget_space = ui.available_height() / 2.0;

        // widget metadata itself will take up slightly more room than the text box
        let metadata_text_space = widget_space - ctx.measurements.collapsible_header_extra_height;

        // make sure we don't go smaller than one line (which would be meaningless)
        let min_height = metadata_text_space.max(ctx.measurements.text_box_height);

        egui::CollapsingHeader::new("Subject")
            .default_open(true)
            .show(ui, |ui| {
                let response = ui.add_sized(
                    egui::vec2(ui.available_width(), min_height),
                    |ui: &'_ mut Ui| self.metadata.subject.ui(ui, ctx),
                );
                self.process_response(&response);
                ids.push(response.id);
            });

        egui::CollapsingHeader::new("Commentary")
            .default_open(true)
            .show(ui, |ui| {
                let response = ui.add_sized(
                    egui::vec2(ui.available_width(), min_height),
                    |ui: &'_ mut Ui| self.metadata.commentary.ui(ui, ctx),
                );
                self.process_response(&response);
                ids.push(response.id);
            });
        ids
    }
}
