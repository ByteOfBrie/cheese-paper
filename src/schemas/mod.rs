mod default;
mod overthinker;

#[cfg(test)]
mod test;

#[cfg(test)]
pub use default::export_file_types;

pub use default::DEFAULT_SCHEMA;

use std::hash::{Hash, Hasher};

use crate::{cheese_error, components::Schema, util::CheeseError};

pub const SCHEMA_LIST: [&'static dyn Schema; 2] =
    [&DEFAULT_SCHEMA, &overthinker::OVERTHINKER_SCHEMA];

pub fn resolve_schema(identifier: &str) -> Result<&'static dyn Schema, CheeseError> {
    for schema in SCHEMA_LIST {
        if schema.get_schema_identifier() == identifier {
            return Ok(schema);
        }
    }

    Err(cheese_error!(
        "No schema found with identifier '{identifier}'"
    ))
}

/// A struct which can be used by any schema to represent any of it's available file types
pub struct FileTypeInfo {
    /// identifier used by the schema to indicate a file type
    identifier: &'static str,

    is_folder: bool,

    has_body: bool,

    exportable: bool,

    type_name: &'static str,

    empty_string_name: &'static str,

    extension: &'static str,

    description: &'static str,
}

pub type FileType = &'static FileTypeInfo;

impl PartialEq for FileTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Eq for FileTypeInfo {}

impl Hash for FileTypeInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state)
    }
}

impl std::fmt::Debug for FileTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[FileType: {}]", self.identifier)
    }
}
impl std::fmt::Display for FileTypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.type_name)
    }
}

impl FileTypeInfo {
    pub fn get_identifier(&self) -> &'static str {
        self.identifier
    }

    pub fn is_folder(&self) -> bool {
        self.is_folder
    }

    pub fn has_body(&self) -> bool {
        self.has_body
    }

    pub fn exportable(&self) -> bool {
        self.exportable
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn empty_string_name(&self) -> &'static str {
        self.empty_string_name
    }

    pub fn extension(&self) -> &'static str {
        self.extension
    }

    pub fn description(&self) -> &'static str {
        self.description
    }
}
