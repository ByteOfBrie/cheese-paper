use bitflags::bitflags;
use uuid::Uuid;

use super::*;
use crate::components::file_objects::utils::*;
// use crate::components::file_objects::{Character, Folder, Place, Scene};
use crate::components::schema::FileType;
use crate::util::CheeseError;
use std::ffi::OsString;
use std::fmt::Debug;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;
use toml_edit::{DocumentMut, TableLike};

/// Loading a file:
/// 1. Parse filename as a name -> metadata.name
/// 2. Load file, storing the metadata in some intermediate place
/// 3. Store the rest of the file into the metadata automatically (as present)
/// 4. Check for a meaningful name in the metadata (present and not the default), write if meaningful
///
/// Baseline metadata for all file objects
#[derive(Debug)]
pub struct FileObjectMetadata {
    /// Version of the object, can eventually be used to detect compatibility changes
    pub version: u64,
    /// Name of the object (e.g., title of a scene, character name)
    pub name: String,
    /// ID unique across all objects. The reference implementations use UUIDv4, but any string
    /// is acceptable
    pub id: Rc<String>,
}

#[derive(Debug)]
pub struct BaseFileObject {
    pub metadata: FileObjectMetadata,
    /// Index (ordering within parent)
    pub index: Option<usize>,
    pub file: FileInfo,
    pub toml_header: DocumentMut,
    pub children: Vec<FileID>,
}

impl Default for FileObjectMetadata {
    fn default() -> Self {
        Self {
            version: 1u64,
            name: String::new(),
            id: Rc::new(Uuid::new_v4().as_hyphenated().to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Path of the directory containing this file
    /// `/foo/bar/` -> `/foo`
    pub dirname: PathBuf,
    /// Path of the file within the dirname
    /// `/foo/bar/` -> `bar`
    pub basename: OsString,
    /// Modified time if the file exists
    pub modtime: Option<SystemTime>,
    pub modified: bool,
}

bitflags! {

    /// The presence of this particular object in the export.
    ///
    /// The `INCLUDE` will (likely) represent an "include in story" checkbox,
    /// if it's not set, none of the other options have any meaningful value.
    /// We (currently) accept bools as well, since that's most of the normal
    /// configuration.
    ///
    /// At a data level, it would probably make more sense to have (at least)
    /// three separate variables here (include, include_title, and break_at_end),
    /// where the latter two will have an enum with (default, always, and never),
    /// which is how it'll likely be presented in the UI, but doing it this way is
    /// more compact in the file.
    #[derive(Debug)]
    pub struct CompileStatus: u64 {
        const INCLUDE                = 0b0000_0000_0000_0001;
        const OVERRIDE_INCLUDE_TITLE = 0b0000_0000_0000_0010;
        const INCLUDE_TITLE          = 0b0000_0000_0000_0100;
        const OVERRIDE_BREAK_AT_END  = 0b0000_0000_0000_1000;
        const BREAK_AT_END           = 0b0000_0000_0001_0000;

        // allow for any bits, in case a future version of cheese-paper sets more
        const _ = !0;
    }
}

impl Default for CompileStatus {
    fn default() -> Self {
        CompileStatus::INCLUDE
    }
}

impl CompileStatus {
    pub fn include_title(&self) -> IncludeOptions {
        if self.contains(CompileStatus::INCLUDE_TITLE | CompileStatus::OVERRIDE_INCLUDE_TITLE) {
            IncludeOptions::Always
        } else if self.contains(CompileStatus::OVERRIDE_INCLUDE_TITLE) {
            IncludeOptions::Never
        } else {
            IncludeOptions::Default
        }
    }

    pub fn set_include_title(&mut self, options: IncludeOptions) {
        match options {
            IncludeOptions::Default => self.set(CompileStatus::OVERRIDE_INCLUDE_TITLE, false),
            IncludeOptions::Always => {
                self.set(CompileStatus::OVERRIDE_INCLUDE_TITLE, true);
                self.set(CompileStatus::INCLUDE_TITLE, true);
            }
            IncludeOptions::Never => {
                self.set(CompileStatus::OVERRIDE_INCLUDE_TITLE, true);
                self.set(CompileStatus::INCLUDE_TITLE, false);
            }
        }
    }

    pub fn break_at_end(&self) -> IncludeOptions {
        if self.contains(CompileStatus::BREAK_AT_END | CompileStatus::OVERRIDE_BREAK_AT_END) {
            IncludeOptions::Always
        } else if self.contains(CompileStatus::OVERRIDE_BREAK_AT_END) {
            IncludeOptions::Never
        } else {
            IncludeOptions::Default
        }
    }

    pub fn set_break_at_end(&mut self, options: IncludeOptions) {
        match options {
            IncludeOptions::Default => self.set(CompileStatus::OVERRIDE_BREAK_AT_END, false),
            IncludeOptions::Always => {
                self.set(CompileStatus::OVERRIDE_BREAK_AT_END, true);
                self.set(CompileStatus::BREAK_AT_END, true);
            }
            IncludeOptions::Never => {
                self.set(CompileStatus::OVERRIDE_BREAK_AT_END, true);
                self.set(CompileStatus::BREAK_AT_END, false);
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum IncludeOptions {
    #[default]
    Default,
    Always,
    Never,
}

impl FileObjectMetadata {
    /// Given a freshly read metadata dictionary, read it into the file objects, setting modified as
    /// appropriate
    pub fn load_base_metadata(
        &mut self,
        metadata_table: &dyn TableLike,
        file_info: &mut FileInfo,
    ) -> Result<(), CheeseError> {
        match metadata_extract_u64(metadata_table, "file_format_version", false)? {
            Some(version) => self.version = version,
            None => file_info.modified = true,
        }

        match metadata_extract_string(metadata_table, "name")? {
            Some(name) => self.name = name,
            None => file_info.modified = true,
        }

        match metadata_extract_string(metadata_table, "id")? {
            Some(id) => self.id = Rc::new(id),
            None => file_info.modified = true,
        }

        Ok(())
    }
}

impl BaseFileObject {
    /// Calculates the filename for a particular object
    pub fn calculate_filename(&self, file_type: FileType) -> OsString {
        let base_name: &str = if self.metadata.name.is_empty() {
            file_type.empty_string_name()
        } else {
            &self.metadata.name
        };

        let mut basename = match self.index {
            Some(index) => {
                let truncated_name = truncate_name(base_name, super::FILENAME_MAX_LENGTH);
                let file_safe_name = process_name_for_filename(truncated_name);
                let final_name = add_index_to_name(&file_safe_name, index);

                OsString::from(final_name)
            }
            None => OsString::from(process_name_for_filename(base_name)),
        };

        if !file_type.is_folder() {
            basename.push(".");
            basename.push(file_type.extension());
        }

        basename
    }

    /// Create a new file object in a folder
    pub fn new(dirname: PathBuf, index: Option<usize>) -> Self {
        Self {
            metadata: FileObjectMetadata::default(),
            index,
            file: FileInfo {
                dirname,
                basename: OsString::new(),
                modtime: None,
                modified: true, // Newly added files are modified (they don't exist on disk)
            },
            toml_header: DocumentMut::new(),
            children: Vec::new(),
        }
    }

    pub fn write_metadata(&mut self) {
        self.toml_header["file_format_version"] = toml_edit::value(self.metadata.version as i64);
        self.toml_header["name"] = toml_edit::value(&self.metadata.name);
        self.toml_header["id"] = toml_edit::value(&*self.metadata.id);
    }
}
impl std::fmt::Display for dyn FileObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[File Object | name=\"{}\" | id={}]",
            self.get_title(),
            self.id()
        )
    }
}
