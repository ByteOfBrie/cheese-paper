mod base_file_object;
mod reference;
mod tools;
pub mod utils;

use crate::components::Schema;
use crate::components::schema::FileType;

use crate::components::project::ExportOptions;
use crate::ui::FileObjectEditor;
use crate::ui::prelude::EditorContext;

pub use tools::{FileID, FileObjectStore};
pub use tools::{ProjectStatistics, WordCountInfo};

use crate::util::CheeseError;
use std::fmt::Debug;
use std::rc::Rc;

pub use utils::{FILENAME_MAX_LENGTH, FOLDER_METADATA_FILE_NAME, HEADER_SPLIT};

pub use base_file_object::{
    BaseFileObject, CompileStatus, FileInfo, FileObjectMetadata, IncludeOptions,
};

pub use reference::ObjectReference;

pub trait FileObject: Debug {
    fn get_type(&self) -> FileType;

    fn get_schema(&self) -> &'static dyn Schema;

    fn get_base(&self) -> &BaseFileObject;
    fn get_base_mut(&mut self) -> &mut BaseFileObject;

    /// Load the body when loading this file object
    fn load_body(&mut self, body: String);
    /// Gets the contents of the body to be written when saving
    fn get_body(&self) -> String;

    /// Display the outline, writing all relevant non-prose information we have to a single
    /// markdown file that can be scanned/shared easily. We don't (currently) have any selections
    /// on export, everything gets included
    fn generate_outline(
        &self,
        _depth: u64,
        _export_string: &mut String,
        _objects: &FileObjectStore,
    ) {
        // we don't do anything by default
    }

    /// Generate an export of story text, will be overridden by objects that actually generate
    /// (folder and scene)
    ///
    /// `include_break` adds a break at the beginning if appropriate, and this function returns
    /// `true` if the next function should include a break
    fn generate_export(
        &self,
        _current_depth: u64,
        _export_string: &mut String,
        _objects: &FileObjectStore,
        _export_options: &ExportOptions,
        include_break: bool,
    ) -> bool {
        // we don't do anything by default, but we want to pass on the include
        include_break
    }

    fn id(&self) -> &Rc<String> {
        &self.get_base().metadata.id
    }

    /// Gets the word count of what will go in the export. Scenes should provide their
    /// own implemntation
    fn text_word_count(&self, _ctx: &mut EditorContext) -> usize {
        0
    }

    /// Whether this object will be exported, scenes and folders should implement
    /// TODO: can this be used for making generate_export more generic?
    fn include_in_export(&self) -> bool {
        false
    }

    fn resolve_references(&mut self, _objects: &FileObjectStore) {}

    /// Loads the file-specific metadata from the toml document
    ///
    /// pulls from the file object instead of an argument (otherwise it's slightly tricky to do ownership)
    fn load_metadata(&mut self) -> Result<bool, CheeseError>;

    /// Writes the current type-specific metadata to the BaseFileObjects toml_header
    fn write_metadata(&mut self, objects: &FileObjectStore);

    fn as_editor(&self) -> &dyn FileObjectEditor;

    fn as_editor_mut(&mut self) -> &mut dyn FileObjectEditor;

    /// a way to write and read directly to the metadata, for use in tests
    #[cfg(test)]
    fn get_test_field(&mut self) -> &mut String;
}
