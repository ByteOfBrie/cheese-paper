mod character;
mod folder;
mod note;
mod scene;
mod section;

use crate::cheese_error;
use crate::components::schema::Schema;
use crate::{
    components::file_objects::{BaseFileObject, FileObject},
    util::CheeseError,
};

use super::FileType;
use std::path::Path;

use character::Character;
use folder::Folder;
use note::Note;
use scene::Scene;
use section::Section;

const FILE_TYPE_LIST: [FileType; 5] = [
    &Scene::TYPE_INFO,
    &Section::TYPE_INFO,
    &Note::TYPE_INFO,
    &Character::TYPE_INFO,
    &Folder::TYPE_INFO,
];

pub struct OverthinkerSchema {}

impl Schema for OverthinkerSchema {
    fn get_schema_identifier(&self) -> &'static str {
        "overthinker"
    }

    fn get_schema_name(&self) -> &'static str {
        "Overthinker"
    }

    fn resolve_type(
        &self,
        filename: &Path,
        file_type_identifier: Option<&str>,
    ) -> Result<FileType, CheeseError> {
        match file_type_identifier {
            Some(file_type_str) => match file_type_str {
                Scene::IDENTIFIER => Ok(&Scene::TYPE_INFO),
                Folder::IDENTIFIER => Ok(&Folder::TYPE_INFO),
                Character::IDENTIFIER => Ok(&Character::TYPE_INFO),
                Note::IDENTIFIER => Ok(&Note::TYPE_INFO),
                Section::IDENTIFIER => Ok(&Section::TYPE_INFO),

                _ => Err(cheese_error!("Unknown file type: {file_type_str}")),
            },
            None => {
                if filename.is_dir() {
                    Ok(&Folder::TYPE_INFO)
                } else {
                    match filename.extension().and_then(|ext| ext.to_str()) {
                        Some("md") => Ok(&Scene::TYPE_INFO),
                        _ => Err(cheese_error!(
                            "Unspecified file type file type while attempting to read {filename:?}"
                        )),
                    }
                }
            }
        }
    }

    fn get_all_file_types(&self) -> &'static [FileType] {
        &FILE_TYPE_LIST
    }

    fn get_top_level_folder_type(&self) -> FileType {
        &Folder::TYPE_INFO
    }

    fn init_file_object(
        &self,
        file_type: FileType,
        base: BaseFileObject,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        match file_type.identifier {
            Character::IDENTIFIER => Ok(Box::new(Character::from_base(base)?)),
            Folder::IDENTIFIER => Ok(Box::new(Folder::from_base(base)?)),
            Scene::IDENTIFIER => Ok(Box::new(Scene::from_base(base, None)?)),
            Note::IDENTIFIER => Ok(Box::new(Note::from_base(base, None)?)),
            Section::IDENTIFIER => Ok(Box::new(Section::from_base(base)?)),
            _ => unreachable!(),
        }
    }

    fn load_file_object(
        &self,
        file_type: FileType,
        base: BaseFileObject,
        body: Option<String>,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        assert_eq!(body.is_some(), file_type.has_body());

        match file_type.identifier {
            Character::IDENTIFIER => Ok(Box::new(character::Character::from_base(base)?)),
            Folder::IDENTIFIER => Ok(Box::new(folder::Folder::from_base(base)?)),
            Scene::IDENTIFIER => Ok(Box::new(scene::Scene::from_base(base, body)?)),
            Note::IDENTIFIER => Ok(Box::new(note::Note::from_base(base, body)?)),
            Section::IDENTIFIER => Ok(Box::new(Section::from_base(base)?)),
            _ => unreachable!(),
        }
    }
}

pub const OVERTHINKER_SCHEMA: OverthinkerSchema = OverthinkerSchema {};
