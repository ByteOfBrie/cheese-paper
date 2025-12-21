use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::cheese_error;
use crate::components::file_objects::utils::{get_index_from_name, write_with_temp_file};
// use crate::components::file_objects::{Character, Folder, Place, Scene};
use crate::util::CheeseError;
use egui_ltreeview::DirPosition;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

use super::*;
use crate::components::file_objects::utils::read_file_contents;

pub type FileID = Rc<String>;

pub type FileObjectStore = HashMap<FileID, RefCell<Box<dyn FileObject>>>;

impl dyn FileObject {
    pub fn is_folder(&self) -> bool {
        self.get_type().is_folder()
    }

    pub fn has_body(&self) -> bool {
        self.get_type().has_body()
    }

    pub fn type_name(&self) -> &'static str {
        self.get_type().type_name()
    }

    pub fn empty_string_name(&self) -> &'static str {
        self.get_type().empty_string_name()
    }

    pub fn extension(&self) -> &'static str {
        self.get_type().extension()
    }

    pub fn calculate_filename(&self) -> OsString {
        self.get_base().calculate_filename(self.get_type())
    }

    /// Calculates the object's current path. For objects in a single file, this is their path
    /// (including the extension), for folder-based objects (i.e., Folder, Place), this is the
    /// path to the folder.
    ///
    /// Also see `get_file`
    pub fn get_path(&self) -> PathBuf {
        Path::join(
            &self.get_base().file.dirname,
            &self.get_base().file.basename,
        )
    }

    /// The path to an object's underlying file, the equivalent of `get_path` when doing file
    /// operations on this object
    pub fn get_file(&self) -> PathBuf {
        let base_path = self.get_path();
        if self.is_folder() {
            Path::join(&base_path, FOLDER_METADATA_FILE_NAME)
        } else {
            base_path
        }
    }

    /// Determine if the file should be loaded
    fn should_load(&mut self, file_to_read: &Path) -> Result<bool, CheeseError> {
        let current_modtime = match std::fs::metadata(file_to_read) {
            Ok(file_metadata) => file_metadata.modified()?,
            Err(err) => {
                log::warn!(
                    "attempted to load file that does not exist: {:?}",
                    file_to_read
                );
                return Err(err.into());
            }
        };

        if let Some(old_modtime) = self.get_base().file.modtime
            && old_modtime == current_modtime
        {
            // We've already loaded the latest revision, nothing to do
            return Ok(false);
        }

        Ok(true)
    }

    /// Reloads the contents of this file object from disk. Assumes that the file has been properly
    /// initialized already
    pub fn reload_file(&mut self) -> Result<(), CheeseError> {
        let file_to_read = self.get_file();

        if !self.should_load(&file_to_read)? {
            return Ok(());
        }

        let (metadata_str, file_body) = read_file_contents(&file_to_read)?;

        let new_toml_header = metadata_str
            .parse::<DocumentMut>()
            .expect("invalid file metadata header");

        let base_file_object = self.get_base_mut();

        base_file_object
            .metadata
            .load_base_metadata(new_toml_header.as_table(), &mut base_file_object.file)?;

        base_file_object.toml_header = new_toml_header;

        self.load_metadata()?;

        if let Some(file_body) = file_body {
            self.load_body(file_body);
        }

        Ok(())
    }

    pub fn children<'a>(
        &self,
        objects: &'a FileObjectStore,
    ) -> impl Iterator<Item = &'a RefCell<Box<dyn FileObject>>> {
        self.get_base()
            .children
            .iter()
            .filter_map(|child_id| objects.get(child_id))
    }

    pub fn new<O: FileObject + 'static>(o: O) -> Box<RefCell<dyn FileObject>> {
        Box::new(RefCell::new(o))
    }

    // Helper function to create a child at the end of a directory, which is much simpler
    #[cfg(test)]
    pub fn create_child_at_end(
        &mut self,
        file_type: FileType,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        assert!(self.is_folder());

        // We know it's at the end, and thus we know that there aren't any children
        self.create_child(file_type, DirPosition::Last, &HashMap::new())
    }

    /// Creates a child in this folder, returning it to be added to the list
    pub fn create_child(
        &mut self,
        file_type: FileType,
        position: DirPosition<FileID>,
        objects: &FileObjectStore,
    ) -> Result<Box<dyn FileObject>, CheeseError> {
        let new_index = match position {
            DirPosition::After(child) => {
                self.get_base()
                    .children
                    .iter()
                    .position(|id| *id == child)
                    .unwrap()
                    + 1
            }
            DirPosition::Before(child) => self
                .get_base()
                .children
                .iter()
                .position(|id| *id == child)
                .unwrap(),
            DirPosition::First => 0,
            DirPosition::Last => self.get_base().children.len(),
        };

        self.create_index_gap(new_index, objects)?;

        // It might not be the best behavior to recover from an error *after* a file is created on
        // disk, but that might not even be possible, and is kinda okay since we should only ever
        // overwrite that file by accident, even in the worst case
        let new_object: Box<dyn FileObject> =
            self.get_schema()
                .create_file(file_type, self.get_path(), new_index)?;

        self.get_base_mut()
            .children
            .insert(new_index, new_object.id().clone());

        Ok(new_object)
    }

    /// Creates a gap in the indexes, to be called immediately before a move
    pub fn create_index_gap(
        &mut self,
        index: usize,
        objects: &FileObjectStore,
    ) -> Result<(), CheeseError> {
        assert!(self.is_folder());

        let children = &self.get_base().children;

        // Ensure we have to do the work
        if index < children.len() {
            // Go backwards from the end of the list to the place where the gap is being created
            // to ensure that we don't have collisions with names
            for i in (index..children.len()).rev() {
                let child_id = &children[i];
                objects
                    .get(child_id)
                    .unwrap()
                    .borrow_mut()
                    .set_index(i + 1, objects)
                    .unwrap();
            }

            log::debug!("created indexing gap in {self} at {index}");
        } else {
            log::debug!("indexing gap requested at the end of {self}, nothing to do");
        }
        Ok(())
    }

    pub fn get_title(&self) -> String {
        if self.get_base().metadata.name.is_empty() {
            self.empty_string_name().to_string()
        } else {
            self.get_base().metadata.name.clone()
        }
    }

    /// Called by all of the generate_outline functions, keeps the formatting consistent
    pub fn write_title(&self, depth: u64, export_string: &mut String) {
        // file object title (at the appropriate header level)
        for _ in 0..depth {
            export_string.push('#');
        }
        export_string.push(' ');
        export_string.push_str(&self.get_title());
        export_string.push_str("\n\n");
    }

    /// For ease of calling, `objects` can contain arbitrary objects, only values contained
    /// in `children` will actually be sorted.
    pub fn fix_indexing(&mut self, objects: &FileObjectStore) {
        log::debug!(
            "Fixing indexing of {}: {:?}",
            self,
            self.get_base().children
        );
        for (count, child) in self.children(objects).enumerate() {
            let set_index_result = child.borrow_mut().set_index(count, objects);
            if let Err(err) = set_index_result {
                log::error!(
                    "Error while trying to fix indexing of child {}: {err}",
                    child.borrow()
                );
                panic!(
                    "Error during fix_indexing, cannot be sure if we have valid indexes anymore"
                );
            }
        }
    }

    /// Reorder the children based on their index (self reported in basename), followed by a call to
    /// fix_indexing. If `recursive` is true, do this for all descendents as well
    pub fn rescan_indexing(&mut self, objects: &FileObjectStore, recursive: bool) {
        if !self.is_folder() {
            // nothing to do
            return;
        }

        self.get_base_mut().children.sort_by_key(|child_id| {
            match get_index_from_name(
                &objects
                    .get(child_id)
                    .unwrap()
                    .borrow()
                    .get_base()
                    .file
                    .basename
                    .to_string_lossy(),
            ) {
                Some(index) => index,
                None => usize::MAX,
            }
        });

        self.fix_indexing(objects);

        if recursive {
            for child in self.children(objects) {
                child.borrow_mut().rescan_indexing(objects, true);
            }
        }
    }

    fn move_on_disk(
        &mut self,
        old_path: PathBuf,
        new_path: PathBuf,
        objects: &FileObjectStore,
    ) -> Result<(), CheeseError> {
        if new_path == old_path {
            // Nothing to do
            return Err(cheese_error!("attempted to rename {old_path:?} to itself"));
        }

        if new_path.exists() {
            return Err(cheese_error!(
                "attempted to rename {old_path:?}, but {new_path:?} already exists"
            ));
        }

        if old_path.exists() {
            std::fs::rename(old_path, new_path)?;
        }

        for child in self.children(objects) {
            child
                .borrow_mut()
                .process_path_update(self.get_path(), objects);
        }

        Ok(())
    }

    /// When the parent changes path, updates this dirname and any other children
    pub fn process_path_update(&mut self, new_directory: PathBuf, objects: &FileObjectStore) {
        self.get_base_mut().file.dirname = new_directory;

        // Propogate this to any children
        for child in self.children(objects) {
            child
                .borrow_mut()
                .process_path_update(self.get_path(), objects);
        }
    }

    /// Change the filename in the base object and on disk, processing any required updates
    pub fn set_filename(
        &mut self,
        new_filename: OsString,
        objects: &FileObjectStore,
    ) -> Result<(), CheeseError> {
        let old_path = self.get_path();
        let new_path = Path::join(&self.get_base().file.dirname, &new_filename);

        if new_path == old_path {
            // Nothing to do. this can happen as part of a move being processed by the tracker
            // (which is quite complex to untangle), so it's a debug instead of a warn now
            log::debug!("set_filename: tried to move {old_path:?} to itself, skipping");
            return Ok(());
        }

        self.get_base_mut().file.basename = new_filename;

        if let Err(err) = self.move_on_disk(old_path, new_path, objects) {
            log::error!(
                "failed to set filename of {self:?} to {:?}",
                self.get_base().file.basename
            );
            return Err(err);
        }

        Ok(())
    }

    /// Processes the actual move on disk of this file object. Does *not* handle any logic about
    /// parents or indexes, see `move_child`
    pub fn move_object(
        &mut self,
        new_index: usize,
        new_path: PathBuf,
        objects: &FileObjectStore,
    ) -> Result<(), CheeseError> {
        let old_path = self.get_path();

        self.get_base_mut().index = Some(new_index);
        self.get_base_mut().file.dirname = new_path;
        self.get_base_mut().file.basename = self.calculate_filename();
        let new_path = self.get_path();

        if new_path == old_path {
            // Nothing to do:
            log::warn!("tried to move {old_path:?} to itself, harmless but shouldn't happen");
            return Ok(());
        }

        log::debug!("moving {self} from {old_path:#?} to {new_path:?}");

        self.move_on_disk(old_path, new_path, objects)
    }

    pub fn save(&mut self, objects: &FileObjectStore) -> Result<(), CheeseError> {
        // First, try to save children, intentionally trying all of them
        let mut errors = vec![];
        for child in self.children(objects) {
            if let Err(err) = child.borrow_mut().save(objects) {
                errors.push(err);
            }
        }

        if !self.get_base().file.modified {
            // If we had *any* errors, return one of them
            return match errors.pop() {
                Some(err) => Err(err),
                None => Ok(()),
            };
        }

        // For everything that isn't a top level folder: check if the filename on disk matches
        // the name, updating the file on disk if necessary
        if self.get_base().index.is_some() {
            let calculated_filename = self.calculate_filename();
            if self.get_base().file.basename != calculated_filename {
                self.set_filename(calculated_filename, objects)?
            }
        }

        // Ensure `toml_header` has the up-to-date metadata
        self.get_base_mut().write_metadata();
        self.write_metadata(objects);
        self.get_base_mut().toml_header["file_type"] =
            toml_edit::value(self.get_type().get_identifier());

        let mut final_str = self.get_base().toml_header.to_string();

        // Add the scene body and the split (which we want to do even if there isn't any actual body)
        if self.has_body() {
            final_str.push_str(HEADER_SPLIT);
            final_str.push_str("\n\n");
            final_str.push_str(&self.get_body());
        }

        write_with_temp_file(self.get_file(), final_str)?;

        let new_modtime = std::fs::metadata(self.get_file())
            .expect("attempted to load file that does not exist")
            .modified()
            .expect("Modtime not available");

        // Update modtime based on what we just wrote
        self.get_base_mut().file.modtime = Some(new_modtime);
        self.get_base_mut().file.modified = false;

        // If we had *any* errors, return one of them
        match errors.pop() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn remove_child(
        child_id: &FileID,
        parent_id: &FileID,
        objects: &mut FileObjectStore,
    ) -> Result<(), CheeseError> {
        log::debug!("removing file: {child_id:?}");

        let removed_child = objects.remove(child_id).unwrap();

        removed_child.borrow_mut().remove_file_object(objects)?;

        let parent = objects.get(parent_id).unwrap();

        // Remove this from the list of children
        let child_index = parent
            .borrow()
            .get_base()
            .children
            .iter()
            .position(|id| id == child_id)
            .expect("child_id must be a child of this object");

        parent
            .borrow_mut()
            .get_base_mut()
            .children
            .remove(child_index);

        parent.borrow_mut().fix_indexing(objects);

        Ok(())
    }

    pub fn remove_file_object(&mut self, objects: &mut FileObjectStore) -> Result<(), CheeseError> {
        let mut errors = Vec::new();
        log::debug!("Removing file object {}", self.get_base().metadata.id);

        let children = self.get_base().children.clone();

        // Go through the list backwards, so calling `fix_indexing` at the end
        // isn't expensive (having to do a bunch of moves)
        for child in children.iter().rev() {
            let removed_child = objects.remove(child).unwrap();
            // save any errors for later
            if let Err(err) = removed_child.borrow_mut().remove_file_object(objects) {
                errors.push(err);
            }
        }

        // then, we need to take care of this file
        std::fs::remove_file(self.get_file())?;

        if self.is_folder() {
            std::fs::remove_dir(self.get_path())?;
        }

        // If we had any errors earlier, return them
        match errors.pop() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Sets the index to this file, doing the move if necessary
    pub fn set_index(
        &mut self,
        new_index: usize,
        objects: &FileObjectStore,
    ) -> Result<bool, CheeseError> {
        let object_index = self.get_base().index;
        let filename_index = get_index_from_name(&self.get_base().file.basename.to_string_lossy());

        match (
            Some(new_index) == object_index,
            Some(new_index) == filename_index,
        ) {
            (true, true) => Ok(false),
            (true, false) => {
                log::debug!(
                    "Updating index of object {self} in filename from {} (index: {:?}) to {new_index}",
                    self.get_base().file.basename.to_string_lossy(),
                    filename_index,
                );
                self.set_filename(self.calculate_filename(), objects)?;
                Ok(true)
            }
            (false, true) => {
                log::debug!(
                    "Updating index of object {self} in memory from {:?} to {new_index}",
                    self.get_base().index
                );
                self.get_base_mut().index = Some(new_index);
                Ok(true)
            }
            (false, false) => {
                log::debug!(
                    "Updating index of object {self} on disk and in memory from {:?} to {new_index}",
                    self.get_base().index
                );
                self.get_base_mut().index = Some(new_index);
                self.set_filename(self.calculate_filename(), objects)?;
                Ok(true)
            }
        }
    }
}
