mod character;
mod folder;
mod place;
mod scene;

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
use place::Place;
use scene::Scene;

const FILE_TYPE_LIST: [FileType; 4] = [
    &Scene::TYPE_INFO,
    &Folder::TYPE_INFO,
    &Character::TYPE_INFO,
    &Place::TYPE_INFO,
];

pub struct DefaultSchema {}

impl Schema for DefaultSchema {
    fn get_schema_identifier(&self) -> &'static str {
        "default"
    }

    fn get_schema_name(&self) -> &'static str {
        "Default"
    }

    fn resolve_type(
        &self,
        filename: &Path,
        file_type_identifier: Option<&str>,
    ) -> Result<FileType, CheeseError> {
        match file_type_identifier {
            None => match filename.is_dir() {
                true => Ok(&Folder::TYPE_INFO),
                false => match filename.extension().and_then(|ext| ext.to_str()) {
                    Some("md") => Ok(&Scene::TYPE_INFO),
                    _ => Err(cheese_error!(
                        "Unspecified file type file type while attempting to read {filename:?}"
                    )),
                },
            },
            Some(file_type_str) => {
                match file_type_str {
                    Scene::IDENTIFIER => Ok(&Scene::TYPE_INFO),
                    Folder::IDENTIFIER => Ok(&Folder::TYPE_INFO),
                    Character::IDENTIFIER => Ok(&Character::TYPE_INFO),
                    Place::IDENTIFIER => Ok(&Place::TYPE_INFO),

                    // "worldbuilding" is the proper string, but also accept "place"
                    "place" => Ok(&Place::TYPE_INFO),

                    _ => Err(cheese_error!("Unknown file type: {file_type_str}")),
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
            Character::IDENTIFIER => Ok(Box::new(character::Character::from_base(base)?)),
            Folder::IDENTIFIER => Ok(Box::new(folder::Folder::from_base(base)?)),
            Place::IDENTIFIER => Ok(Box::new(place::Place::from_base(base)?)),
            Scene::IDENTIFIER => Ok(Box::new(scene::Scene::from_base(base, None)?)),
            _ => unreachable!(),
        }
    }

    fn load_file_object(
        &self,
        file_type: FileType,
        base: BaseFileObject,
        body: Option<String>,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        assert!(body.is_some() == file_type.has_body());

        match file_type.identifier {
            Character::IDENTIFIER => Ok(Box::new(character::Character::from_base(base)?)),
            Folder::IDENTIFIER => Ok(Box::new(folder::Folder::from_base(base)?)),
            Place::IDENTIFIER => Ok(Box::new(place::Place::from_base(base)?)),
            Scene::IDENTIFIER => Ok(Box::new(scene::Scene::from_base(base, body)?)),
            _ => unreachable!(),
        }
    }
}

pub const DEFAULT_SCHEMA: DefaultSchema = DefaultSchema {};

#[cfg(test)]
pub mod export_file_types {
    use crate::schemas::{FileType, default};

    pub const CHARACTER: FileType = &default::character::Character::TYPE_INFO;
    pub const FOLDER: FileType = &default::folder::Folder::TYPE_INFO;
    pub const PLACE: FileType = &default::place::Place::TYPE_INFO;
    pub const SCENE: FileType = &default::scene::Scene::TYPE_INFO;
}
