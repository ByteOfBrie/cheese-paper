use regex::Regex;

use crate::components::file_objects::FileObjectStore;
use crate::components::file_objects::utils::{
    metadata_extract_string, metadata_extract_u64, write_outline_property,
};
use crate::components::file_objects::{BaseFileObject, FileObject};
use crate::components::file_objects::{CompileStatus, IncludeOptions, ObjectReference};
use crate::components::project::ExportOptions;
use crate::components::text::Text;
use crate::schemas::FileType;
use crate::util::CheeseError;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::FileObjectEditor;
use crate::ui::prelude::*;

use crate::ford_get;
use crate::schemas::FileTypeInfo;

use egui::ScrollArea;

#[derive(Debug, Default)]
pub struct SceneMetadata {
    pub summary: Text,
    pub notes: Text,
    pub pov: Rc<RefCell<ObjectReference>>,
    pub compile_status: CompileStatus,
}

#[derive(Debug)]
pub struct Scene {
    base: BaseFileObject,
    pub metadata: SceneMetadata,
    pub text: Text,
}

impl Scene {
    pub const IDENTIFIER: &'static str = "scene";

    pub const TYPE_INFO: FileTypeInfo = FileTypeInfo {
        identifier: Self::IDENTIFIER,
        is_folder: false,
        has_body: true,
        type_name: "Scene",
        empty_string_name: "New Scene",
        extension: "md",
        description: "The text of the story",
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

impl FileObject for Scene {
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

        match metadata_extract_string(self.base.toml_header.as_table(), "pov")? {
            Some(pov) => {
                self.metadata.pov = Rc::new(RefCell::new(ObjectReference::new(
                    pov,
                    Some(&super::Character::TYPE_INFO),
                )))
            }
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

    fn load_body(&mut self, data: String) {
        self.text = data.trim().to_string().into();
    }

    fn get_base(&self) -> &BaseFileObject {
        &self.base
    }

    fn get_base_mut(&mut self) -> &mut BaseFileObject {
        &mut self.base
    }

    fn resolve_references(&mut self, objects: &FileObjectStore) {
        let mut pov = self.metadata.pov.borrow_mut();
        if let ObjectReference::Unknown(pov_unknown_ref) = pov.clone()
            && let Some(known_pov) = pov_unknown_ref.resolve(objects)
        {
            *pov = ObjectReference::Known(known_pov);
        }
    }

    fn get_body(&self) -> String {
        let mut full_text = String::new();

        for line in self.text.split('\n') {
            full_text.push_str(line.trim());
            full_text.push('\n');
        }

        full_text
    }

    fn write_metadata(&mut self, objects: &FileObjectStore) {
        self.base.toml_header["summary"] = toml_edit::value(&*self.metadata.summary);
        self.base.toml_header["notes"] = toml_edit::value(&*self.metadata.notes);
        self.base.toml_header["pov"] =
            toml_edit::value(self.metadata.pov.borrow().to_string(objects));
        self.base.toml_header["compile_status"] =
            toml_edit::value(self.metadata.compile_status.bits() as i64);
    }

    fn generate_outline(&self, depth: u64, export_string: &mut String, objects: &FileObjectStore) {
        (self as &dyn FileObject).write_title(depth, export_string);

        write_outline_property("summary", &self.metadata.summary, export_string);
        let needs_pov = *self.metadata.pov.borrow() != ObjectReference::None;
        if needs_pov {
            write_outline_property(
                "pov",
                &self.metadata.pov.borrow().to_string(objects),
                export_string,
            );
        }
        write_outline_property("notes", &self.metadata.notes, export_string);
    }

    fn generate_export(
        &self,
        depth: u64,
        export_string: &mut String,
        _objects: &FileObjectStore,
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
                IncludeOptions::Default => export_options.scene_title_depth.should_display(depth),
                IncludeOptions::Never => false,
            };

            if display_title {
                (self as &dyn FileObject).write_title(depth, export_string);
            } else if include_break {
                // We only include a break if the previous scene/document requested it *and* we
                // didn't already include a heading (title)
                export_string.push_str("----\n\n");
            }

            let body_text_unprocessed = &self.get_body();

            // add in smart quotes, other platforms will insert some and it's easier to be consistent here
            // regexes from https://webapps.stackexchange.com/questions/166314/how-to-replace-dumb-quotes-with-smart-quotes-in-google-docs/169065#169065
            // quotes preceded by whitespace or at the start of a block are beginning quotes
            let opening_double_quote = Regex::new(r#"((^|\s)\*{0,3})""#).unwrap();
            let closing_double_quote = Regex::new("\"").unwrap();

            // same thing for opening quotes
            let opening_single_quote = Regex::new(r#"((^|\s)\*{0,3})'"#).unwrap();
            let closing_single_quote = Regex::new("'").unwrap();

            let body_text = opening_double_quote.replace_all(body_text_unprocessed, "$1“");
            let body_text = closing_double_quote.replace_all(&body_text, "”");

            let body_text = opening_single_quote.replace_all(&body_text, "$1‘");
            let body_text = closing_single_quote.replace_all(&body_text, "’");

            // This should probably eventually be split into a `get_body_export` and `get_body_save`
            // function once those are different (probably for in-text-notes)
            export_string.push_str(&body_text);

            while !export_string.ends_with("\n\n") {
                export_string.push('\n');
            }

            // Determine if there should be a break after this scene and return it
            match self.metadata.compile_status.break_at_end() {
                IncludeOptions::Always => true,
                IncludeOptions::Default => export_options.insert_breaks,
                IncludeOptions::Never => false,
            }
        } else {
            // We didn't do anything, keep the same state
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
        &mut self.metadata.summary
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum SidebarTab {
    #[default]
    Notes,
    Export,
}

#[derive(Debug, Default)]
struct RenderData {
    sidebar_tab: SidebarTab,
    name_box: NameBox,
}

impl FileObjectEditor for Scene {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let mut cheese_response = egui::SidePanel::right("metadata sidebar")
            .resizable(true)
            .default_width(200.0)
            .width_range(50.0..)
            .show_inside(ui, |ui| self.show_sidebar(ui, ctx))
            .inner;

        egui::CentralPanel::default()
            .show_inside(ui, |ui| self.show_text_editor(ui, ctx))
            .inner
            .append_to(&mut cheese_response);

        self.process_response(&cheese_response);
        cheese_response
    }

    fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str)) {
        f(&self.metadata.summary, "Summary");
        f(&self.metadata.notes, "Notes");
        f(&self.text, "text");
    }

    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.metadata.summary, "Summary");
        f(&mut self.metadata.notes, "Notes");
        f(&mut self.text, "text");
    }
}

impl Scene {
    fn show_text_editor(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        ScrollArea::vertical()
            .id_salt("text")
            .auto_shrink(egui::Vec2b { x: false, y: false })
            .show(ui, |ui| {
                ui.add_sized(ui.available_size(), |ui: &'_ mut Ui| self.text.ui(ui, ctx))
                    .into()
            })
            .inner
    }

    fn show_sidebar(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        ford_get!(RenderData, rdata, ctx.stores.file_objects, self.id());
        let mut cheese_response = CheeseResponse::default();

        egui::TopBottomPanel::bottom("word_count").show_inside(ui, |ui| {
            ui.add_space(4.0);
            let words = self.text.word_count(ctx);
            let text = format!("{words} Words");
            ui.vertical_centered(|ui| {
                ui.label(text);
            });
        });

        ScrollArea::vertical().id_salt("metadata").show(ui, |ui| {
            rdata
                .name_box
                .ui(
                    &mut self.get_base_mut().metadata.name,
                    "Unnamed Scene",
                    ui,
                    ctx,
                )
                .append_to(&mut cheese_response);

            // Tab selection
            ui.horizontal(|ui| {
                ui.selectable_value(&mut rdata.sidebar_tab, SidebarTab::Notes, "Summary/Notes");
                ui.selectable_value(&mut rdata.sidebar_tab, SidebarTab::Export, "Export");
            });

            ui.separator();

            match rdata.sidebar_tab {
                SidebarTab::Notes => self.show_sidebar_metadata(ui, ctx),
                SidebarTab::Export => self.show_sidebar_export(ui),
            }
            .append_to(&mut cheese_response);
        });
        cheese_response
    }

    fn show_sidebar_metadata(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut EditorContext,
    ) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();

        // I am doing horrible things here but the borrow checker must be satisifed
        let changed = {
            let mut object_pov = self.metadata.pov.borrow_mut();
            let mut pov = object_pov.clone();

            ui.horizontal(|ui| {
                ui.label("POV ");
                egui::ComboBox::from_id_salt("metadata pov")
                    .selected_text(match &pov {
                        ObjectReference::Known(known_current_pov) => {
                            if let Some(current_pov_name) = ctx
                                .references
                                .for_type(&super::Character::TYPE_INFO)
                                .get(known_current_pov)
                            {
                                current_pov_name.clone()
                            } else {
                                format!("Ref: {known_current_pov}")
                            }
                        }
                        ObjectReference::Unknown(unknown_reference) => {
                            if unknown_reference.name.is_empty() {
                                format!("Ref: {}", unknown_reference.id)
                            } else {
                                format!("Ref: {}|{}", unknown_reference.name, unknown_reference.id)
                            }
                        }
                        ObjectReference::None => "None".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut pov, ObjectReference::None, "None");
                        for (character_reference, name) in
                            ctx.references.for_type(&super::Character::TYPE_INFO).iter()
                        {
                            ui.selectable_value(
                                &mut pov,
                                ObjectReference::Known(character_reference.clone()),
                                name,
                            );
                        }
                    });
            });

            // We don't have an actual response here so we have to manually process
            if pov != *object_pov {
                *object_pov = pov;
                true
            } else {
                false
            }
        };

        if changed {
            self.get_base_mut().file.modified = true;
        }

        // half of the available height should go to each widget
        let widget_space = ui.available_height() / 2.0;

        // widget metadata itself will take up slightly more room than the text box
        let metadata_text_space = widget_space - ctx.measurements.collapsible_header_extra_height;

        // make sure we don't go smaller than one line (which would be meaningless)
        let min_height = metadata_text_space.max(ctx.measurements.text_box_height);

        egui::CollapsingHeader::new("Summary")
            .default_open(true)
            .show(ui, |ui| {
                let response = ui.add_sized(
                    egui::vec2(ui.available_width(), min_height),
                    |ui: &'_ mut Ui| self.metadata.summary.ui(ui, ctx),
                );
                cheese_response.process_response(&response, true);
            });

        egui::CollapsingHeader::new("Notes")
            .default_open(true)
            .show(ui, |ui| {
                let response = ui.add_sized(
                    egui::vec2(ui.available_width(), min_height),
                    |ui: &'_ mut Ui| self.metadata.notes.ui(ui, ctx),
                );
                cheese_response.process_response(&response, true);
            });
        cheese_response
    }

    fn show_sidebar_export(&mut self, ui: &mut egui::Ui) -> CheeseResponse {
        let mut cheese_response = CheeseResponse::default();
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
                const INCLUDE_TITLE_MESSAGE: &str =
                    "If the title of this folder/scene will be included
                default - this will come from the settings in the export tab
                always - include the title for this, even if the project export settings differ
                never - do not include the title for this, even if the export settings differ";

                ui.label("Include Title  ℹ")
                    .on_hover_text(INCLUDE_TITLE_MESSAGE);

                let title_combobox_response = egui::ComboBox::from_id_salt("Include Title")
                    .selected_text(format!("{include_title:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut include_title, IncludeOptions::Default, "Default");
                        ui.selectable_value(&mut include_title, IncludeOptions::Always, "Always");
                        ui.selectable_value(&mut include_title, IncludeOptions::Never, "Never");
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
                const INCLUDE_BREAK_MESSAGE: &str =
                    "If this is followed by a scene, should there be a divider?
                    default - this will come from the settings in the export tab
                    always - include a divider after this, even if the project export settings differ
                    never - do not include a divider after this, even if the export settings differ";

                ui.label("Break at End  ℹ")
                    .on_hover_text(INCLUDE_BREAK_MESSAGE);

                let break_combobox_response = egui::ComboBox::from_id_salt("Break at End")
                    .selected_text(format!("{break_at_end:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut break_at_end, IncludeOptions::Default, "Default");
                        ui.selectable_value(&mut break_at_end, IncludeOptions::Always, "Always");
                        ui.selectable_value(&mut break_at_end, IncludeOptions::Never, "Never");
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

        cheese_response
    }
}
