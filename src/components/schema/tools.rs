use crate::components::file_objects::utils::{get_index_from_name, read_file_contents};
use crate::components::file_objects::{FileInfo, FileObjectMetadata};
use crate::components::schema::{FileType, Schema};

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::create_dir;
use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use crate::cheese_error;

use crate::components::file_objects::{
    BaseFileObject, FOLDER_METADATA_FILE_NAME, FileID, FileObject, FileObjectStore,
};
use crate::util::CheeseError;

impl PartialEq for dyn Schema {
    fn eq(&self, other: &Self) -> bool {
        self.get_schema_identifier() == other.get_schema_identifier()
    }
}

impl Eq for dyn Schema {}

#[allow(clippy::only_used_in_recursion)]
impl dyn Schema {
    fn parent_contains(
        &self,
        parent_id: &FileID,
        checking_id: &FileID,
        objects: &FileObjectStore,
    ) -> bool {
        let parent = objects.get(parent_id).unwrap();

        for child_id in &parent.borrow().get_base().children {
            // directly check if this is object we're looking for
            if child_id == checking_id {
                return true;
            }

            // check all of the children
            if self.parent_contains(child_id, checking_id, objects) {
                return true;
            }
        }

        false
    }

    /// Move a child between two folders, `source_file_id` and `dest_file_id`
    ///
    /// This can't be part of the FileObject trait because ownership is complicated between
    /// the
    pub fn move_child(
        &self,
        moving_file_id: &FileID,
        source_file_id: &FileID,
        dest_file_id: &FileID,
        new_index: usize,
        objects: &FileObjectStore,
    ) -> Result<(), CheeseError> {
        // Check for it being a valid move:
        // * can't move to one of your own children
        if self.parent_contains(moving_file_id, dest_file_id, objects) {
            return Err(cheese_error!(
                "attempted to move {moving_file_id} into itself"
            ));
        }

        // * can't move something without an index
        let moving = objects
            .get(moving_file_id)
            .expect("objects should contain moving file id");

        let moving_index = match moving.borrow().get_base().index {
            Some(index) => index,
            None => {
                return Err(cheese_error!(
                    "attempted to move {moving_file_id:} into itself"
                ));
            }
        };
        // * shouldn't move something where it already is
        if source_file_id == dest_file_id && moving_index == new_index {
            log::debug!("attempted to move {moving_file_id} to itself, skipping");
            return Ok(());
        }

        // We know it's a valid move (or at least think we do), go ahead with the move

        // From this point until the call to fix indexing, we have state that we can't safely recover
        // from with an error, so we should always panic instead
        self.create_index_and_move_on_disk(
            moving_file_id,
            source_file_id,
            dest_file_id,
            new_index,
            objects,
        );

        // if we're moving within an object, we already fixed indexing a few lines above
        if source_file_id != dest_file_id {
            // We just need to clean up and re-index the source to fill in the gap we left
            objects
                .get(source_file_id)
                .unwrap()
                .borrow_mut()
                .fix_indexing(objects);
        }

        Ok(())
    }

    /// Helper function called by move_child for the parts that are not safe to return early (including
    /// errors). If something goes wrong, it will panic
    fn create_index_and_move_on_disk(
        &self,
        moving_file_id: &FileID,
        source_file_id: &FileID,
        dest_file_id: &FileID,
        new_index: usize,
        objects: &FileObjectStore,
    ) {
        // Create index "gap" in destination (helpful to do first in case we're moving "up" and this
        // changes the path of the object being moved)
        objects
            .get(dest_file_id)
            .unwrap()
            .borrow_mut()
            .create_index_gap(new_index, objects)
            .unwrap();

        // Remove the moving object from it's current parent
        let source = objects
            .get(source_file_id)
            .expect("objects should contain source file id");

        let child_id_position = source
            .borrow()
            .get_base()
            .children
            .iter()
            .position(|val| moving_file_id == val)
            .unwrap_or_else(|| {
                panic!(
                    "Children should only be removed from their parents. \
                    child id: {moving_file_id}, parent: {source_file_id}"
                )
            });

        let child_id_string = source
            .borrow_mut()
            .get_base_mut()
            .children
            .remove(child_id_position);

        // Object is now removed from it's current parent, although still actually there on disk
        // We should also stop using `source` or the (scary) borrow checker will get mad at us
        // update: we have added some refcells everywhere and the borrow checker is nicer now

        let dest = objects.get(dest_file_id).unwrap();

        let insertion_index = std::cmp::min(new_index, dest.borrow().get_base().children.len());
        // Move the object into the children of dest (at the proper place)
        dest.borrow_mut()
            .get_base_mut()
            .children
            .insert(insertion_index, child_id_string);

        let child = objects.get(moving_file_id).unwrap();

        // Move the actual child on disk
        if let Err(err) =
            child
                .borrow_mut()
                .move_object(new_index, dest.borrow().get_path(), objects)
        {
            // We don't pass enough information around to meaninfully recover here
            log::error!("Encountered error while trying to move {moving_file_id}");
            panic!("Encountered unrecoverable error while trying to move {moving_file_id}: {err}");
        }

        // Fix indexing in the destination (now that it has the child)
        dest.borrow_mut().fix_indexing(objects);
    }

    /// Load an arbitrary file object from a file on disk into objects
    pub fn load_file(
        &self,
        filename: &Path,
        objects: &mut FileObjectStore,
    ) -> Result<FileID, CheeseError> {
        if !filename.exists() {
            return Err(cheese_error!(
                "from_file cannot load file that does not exist: {filename:?}"
            ));
        }

        // We process every dir, but only `.toml` or `.md` files
        if !filename.is_dir()
            && filename
                .extension()
                .is_none_or(|extension| extension != "toml" && extension != "md")
        {
            return Err(cheese_error!(
                "from_file cannot load file {filename:?} with unknown extension"
            ));
        }

        // Create the file info components right at the start
        let dirname = filename
            .parent()
            .ok_or(cheese_error!(
                "filename supplied to from_file should have a dirname component",
            ))?
            .to_path_buf();

        let basename = filename
            .file_name()
            .ok_or(cheese_error!(
                "filename supplied to from_file should have a basename component",
            ))?
            .to_owned();

        let mut modified = false;

        // If the filename is a directory, we need to look for the underlying file
        let underlying_file = match filename.is_dir() {
            true => filename.join(FOLDER_METADATA_FILE_NAME),
            false => filename.to_path_buf(),
        };

        let (metadata_str, file_body) = read_file_contents(&underlying_file).or_else(|err| {
            if filename.is_dir() {
                Ok(("".to_string(), None))
            } else {
                Err(cheese_error!(
                    "Failed to read file {underlying_file:?}: {err}"
                ))
            }
        })?;

        let mut metadata = FileObjectMetadata::default();

        let toml_header = metadata_str
            .parse::<DocumentMut>()
            .map_err(|err| cheese_error!("Error parsing {underlying_file:?}: {err}"))?;

        if !toml_header.contains_key("name") {
            let file_name = PathBuf::from(&basename)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let name_to_parse = if let Some((prefix, suffix)) = file_name.split_once('-') {
                match prefix.parse::<i64>() {
                    Ok(_) => suffix,
                    Err(_) => file_name.as_str(),
                }
            } else {
                file_name.as_str()
            };

            metadata.name = name_to_parse.replace("_", " ").trim().to_string();
            if !metadata.name.is_empty() {
                modified = true;
            }
        }

        let file_type_identifier = match toml_header.get("file_type") {
            Some(file_type_toml_item) => match file_type_toml_item.as_str() {
                Some(file_type_str) => Some(file_type_str),
                None => {
                    return Err(cheese_error!(
                        "file header contained non-string value for file_type: {filename:?}"
                    ));
                }
            },
            None => None,
        };

        let file_type: FileType = self.resolve_type(filename, file_type_identifier)?;

        let mut children = Vec::new();

        // Load children of this file object
        if file_type.is_folder() {
            if !filename.is_dir() {
                return Err(cheese_error!(
                    "{filename:?} has a folder-based file_type, but isn't actually a directory",
                ));
            }

            // We rescan and fix the indexing at the end when returning a folder or place, so we can read
            // the files in any order here. The only files that won't ever be affected by this are the
            // roots, which don't have indexing anyway
            for file in std::fs::read_dir(filename).map_err(|err| {
                cheese_error!("Error while attempt to read folder {filename:?}: {err}")
            })? {
                match file {
                    Ok(file) => {
                        // We've already read this file, nothing to do
                        if file.file_name() == FOLDER_METADATA_FILE_NAME {
                            continue;
                        }

                        let file_path = file.path();

                        // Just read the children in any order, we'll clean it up later
                        match self.load_file(&file_path, objects) {
                            Ok(child_id) => children.push(child_id.clone()),
                            Err(err) => log::debug!("Could not load child {file:?}: {err}"),
                        }
                    }
                    Err(err) => log::warn!("Could not read file {filename:?}: {err}"),
                }
            }
        }

        let index = get_index_from_name(&basename.to_string_lossy());

        // Check if we're loading a file object that we already know about
        if let Some(existing_file_id) = toml_header
            .get("id")
            .and_then(|id_item| id_item.as_str())
            .map(|id_str| FileID::new(id_str.to_owned()))
            && objects.contains_key(&existing_file_id)
        {
            // we just update the object in place
            let mut file_object = objects.get(&existing_file_id).unwrap().borrow_mut();
            file_object.get_base_mut().children = children;

            file_object.get_base_mut().file.dirname = dirname;
            file_object.get_base_mut().file.basename = basename;

            file_object.get_base_mut().index = index;

            file_object.reload_file()?;

            Ok(existing_file_id)
        } else {
            // we need to create a new object

            let mut file_info = FileInfo {
                dirname,
                basename,
                modtime: None,
                modified,
            };

            metadata
                .load_base_metadata(toml_header.as_table(), &mut file_info)
                .map_err(|err| {
                    cheese_error!("Error while parsing metadata for {filename:?}: {err}")
                })?;

            let base = BaseFileObject {
                metadata,
                index,
                file: file_info,
                toml_header,
                children,
            };

            let file_id = base.metadata.id.clone();

            let mut file_object = self.load_file_object(file_type, base, file_body)?;

            file_object.rescan_indexing(objects, false);

            objects.insert(file_id.clone(), RefCell::new(file_object));

            Ok(file_id)
        }
    }

    pub fn create_file(
        &self,
        file_type: FileType,
        dirname: PathBuf,
        index: usize,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        let base = BaseFileObject::new(dirname, Some(index));

        let mut file_object = self.init_file_object(file_type, base)?;

        file_object.get_base_mut().file.basename = file_object.calculate_filename();

        if file_type.is_folder() {
            create_dir(file_object.get_path())?;
        }

        file_object.save(&HashMap::new())?;

        Ok(file_object)
    }

    /// Creates a top level folder (one that doesn't have an index) based on the name. The name will
    /// be used directly in the metadata, but convereted to lowercase for the version on disk
    pub fn create_top_level_folder(
        &self,
        dirname: PathBuf,
        name: &str,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        let file_type = self.get_top_level_folder_type();
        assert!(file_type.is_folder());

        let mut base = BaseFileObject::new(dirname, None);

        base.metadata.name = name.to_string();
        base.file.basename = OsString::from(name.to_lowercase());

        let mut file_object = self.init_file_object(file_type, base)?;

        create_dir(file_object.get_path()).map_err(|err| {
            cheese_error!("Failed to create top-level directory: {}: {err}", name)
        })?;

        file_object.save(&HashMap::new()).map_err(|err| {
            cheese_error!(
                "Failed to save newly created top level directory: {}: {err}",
                name
            )
        })?;

        Ok(file_object)
    }
}
