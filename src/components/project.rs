use crate::cheese_error;
use crate::components::file_objects::{FileInfo, FileObject, FileObjectMetadata, FileObjectStore};
use crate::components::schema::Schema;
use crate::components::text::Text;
use crate::schemas::{DEFAULT_SCHEMA, resolve_schema};
use crate::util::CheeseError;

use notify::event::RenameMode;
use notify::{EventKind, event::ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebouncedEvent, Debouncer, RecommendedCache, new_debouncer};

use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use toml_edit::DocumentMut;

use crate::components::file_objects::{FOLDER_METADATA_FILE_NAME, FileID};

use crate::components::file_objects::utils::{
    metadata_extract_bool, metadata_extract_string, metadata_extract_u64,
    process_name_for_filename, write_outline_property, write_with_temp_file,
};

type RecommendedDebouncer = Debouncer<RecommendedWatcher, RecommendedCache>;
type WatcherReceiver = std::sync::mpsc::Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>;

/// An entire project. This is somewhat file_object like, but we don't implement everything,
/// so it's separate (for now)
#[derive(Debug)]
pub struct Project {
    pub schema: &'static dyn Schema,
    pub metadata: ProjectMetadata,
    pub base_metadata: FileObjectMetadata,
    pub file: FileInfo,
    /// The list of top level folders. The order is hardcoded for now but this can be relaxed later
    pub top_level_folders: Vec<FileID>,
    pub objects: FileObjectStore,
    toml_header: DocumentMut,

    last_added_event: Option<Instant>,
    event_queue: VecDeque<DebouncedEvent>,
    file_event_rx: WatcherReceiver,

    /// We don't need to do anything to the watcher, but we stop getting events if it's dropped
    _watcher: RecommendedDebouncer,
}

#[derive(Debug, Default)]
pub struct ProjectMetadata {
    pub summary: Text,
    pub notes: Text,
    pub genre: String,
    pub author: String,
    pub email: String,

    pub export: ProjectExportSettings,
}

#[derive(Debug)]
pub struct ProjectExportSettings {
    pub include_all_folder_titles: bool,
    /// how many levels deep to include folder titles, ignored if include_all_folder_titles is set
    pub include_folder_title_depth: u64,

    pub include_all_scene_titles: bool,
    /// how many levels deep to include scene titles, ignored if include_all_scene_titles is set
    pub include_scene_title_depth: u64,

    pub insert_break_at_end: bool,
}

impl Default for ProjectExportSettings {
    fn default() -> Self {
        Self {
            include_all_folder_titles: false,
            include_folder_title_depth: 1,
            include_all_scene_titles: false,
            include_scene_title_depth: 1,
            insert_break_at_end: true,
        }
    }
}

impl ProjectMetadata {
    pub fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str)) {
        f(&self.summary, "summary");
        f(&self.notes, "notes");
    }

    #[allow(dead_code)] // included for the sake of completeness
    pub fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str)) {
        f(&mut self.summary, "summary");
        f(&mut self.notes, "notes");
    }
}

const PROJECT_INFO_NAME: &str = "project.toml";

/// Loads a special top level folder (e.g., "project/text/", "project/worldbuilding"), creating it if
/// it doesn't already exist.
///
/// Name will be used directly in the metadata name, but will be converted to lowercase for the filename
fn load_top_level_folder(
    schema: &'static dyn Schema,
    project_path: &Path,
    name: &str,
    objects: &mut FileObjectStore,
) -> Result<FileID, CheeseError> {
    log::debug!("loading top level folder: {name}");

    let folder_path = &Path::join(project_path, name.to_lowercase());
    if folder_path.exists() {
        let created_object = schema
            .load_file(folder_path, objects)
            .map_err(|err| cheese_error!("failed to load top level folder {name}\n{}", err))?;

        let created_object_box = objects.get(&created_object).unwrap();
        let is_folder = created_object_box.borrow().is_folder();
        if is_folder {
            // A whole bunch of code to ensure that we get a capital letter if this object didn't already
            // have a name
            let modified = created_object_box.borrow().get_base().file.modified;
            if modified {
                let update_name = created_object_box.borrow().get_base().metadata.name != name
                    && created_object_box
                        .borrow()
                        .get_base()
                        .metadata
                        .name
                        .eq_ignore_ascii_case(name);

                if update_name {
                    created_object_box.borrow_mut().get_base_mut().metadata.name = name.to_string();
                }
            }

            Ok(created_object)
        } else {
            Err(cheese_error!(
                "somehow loaded a non-folder as a top level folder",
            ))
        }
    } else {
        log::debug!("top level folder {name} does not exist, creating...");
        let top_level_folder = schema
            .create_top_level_folder(project_path.to_owned(), name)
            .map_err(|err| {
                cheese_error!(
                    "An error occured while creating the top level folder\n{}",
                    err
                )
            })?;
        let folder_id = top_level_folder.id().clone();
        objects.insert(folder_id.clone(), RefCell::new(top_level_folder));
        Ok(folder_id)
    }
}

#[cfg(not(test))]
const WATCHER_MSEC_DURATION: u64 = 1000;

#[cfg(test)]
const WATCHER_MSEC_DURATION: u64 = 50;

fn create_watcher() -> notify::Result<(RecommendedDebouncer, WatcherReceiver)> {
    let (tx, rx) = std::sync::mpsc::channel();

    let watcher = new_debouncer(
        std::time::Duration::from_millis(WATCHER_MSEC_DURATION),
        None,
        tx,
    )?;

    Ok((watcher, rx))
}

// We hardcode the path here, might get replaced when schema can change file objects
const TEXT_FOLDER_POSITION: usize = 0;

impl Project {
    /// Create a new project
    pub fn new(
        schema: &'static dyn Schema,
        dirname: PathBuf,
        project_name: String,
    ) -> Result<Self, CheeseError> {
        let canonical_dirname = dirname.canonicalize().unwrap();
        // Not truncating here (for now)
        let file_safe_name = process_name_for_filename(&project_name);
        let project_path = canonical_dirname.join(&file_safe_name);

        if project_path.exists() {
            return Err(cheese_error!(
                "attempted to initialize {project_path:?}, which already exists"
            ));
        } else {
            std::fs::create_dir(&project_path)?;
        }

        let text = schema.create_top_level_folder(project_path.clone(), "Text")?;
        let characters = schema.create_top_level_folder(project_path.clone(), "Characters")?;
        let worldbuilding =
            schema.create_top_level_folder(project_path.clone(), "Worldbuilding")?;

        let file = FileInfo {
            dirname: canonical_dirname,
            basename: OsString::from(file_safe_name),
            modtime: None,
            modified: true, // Newly added files are modified (they don't exist on disk)
        };

        // Create the watcher path by hand since we can't call get_path() yet
        let watcher_path = file.dirname.join(&file.basename);

        // this might later get wrapped in an optional block or something but not worth it right now
        let (mut watcher, file_event_rx) =
            create_watcher().expect("Should always be able to create a watcher");

        watcher
            .watch(watcher_path, RecursiveMode::Recursive)
            .unwrap();

        let mut toml_header = DocumentMut::new();
        toml_header["schema"] = toml_edit::value(schema.get_schema_identifier());

        let top_level_folders = vec![
            text.id().clone(),
            characters.id().clone(),
            worldbuilding.id().clone(),
        ];

        let mut project = Self {
            schema,
            base_metadata: FileObjectMetadata {
                name: project_name,
                ..Default::default()
            },
            metadata: ProjectMetadata::default(),
            top_level_folders,
            file,
            toml_header,
            objects: HashMap::new(),
            last_added_event: None,
            event_queue: VecDeque::new(),
            file_event_rx,
            _watcher: watcher,
        };

        project.add_object(text);
        project.add_object(characters);
        project.add_object(worldbuilding);

        project.save()?;

        Ok(project)
    }

    /// Load an existing project from disk
    pub fn load(path: PathBuf) -> Result<Self, CheeseError> {
        if !path.exists() {
            return Err(cheese_error!(
                "attempted to load {path:?}, was not a directory"
            ));
        }

        let mut file_info = FileInfo {
            dirname: match path.parent() {
                Some(dirname) => dirname,
                None => {
                    return Err(cheese_error!("no directory component in {path:?}"));
                }
            }
            .to_path_buf(),
            basename: match path.file_name() {
                Some(basename) => basename,
                None => {
                    return Err(cheese_error!("no filename component in {path:?}"));
                }
            }
            .to_owned(),
            modtime: None,
            modified: false,
        };

        let mut base_metadata = FileObjectMetadata::default();
        let metadata = ProjectMetadata::default();

        // Load project metadata
        let project_info_path = Path::join(&path, PROJECT_INFO_NAME);

        let mut toml_header = if project_info_path.exists() {
            log::debug!("Found `project_info.toml`, loading project");

            let project_info_data =
                std::fs::read_to_string(project_info_path).expect("could not read file");

            project_info_data
                .parse::<DocumentMut>()
                .expect("invalid file metadata header")
        } else {
            // If the `project.toml` doesn't exist, check for a `text/` folder so we don't accidentally
            // load and hijack another folder
            if !Path::join(&path, "text").is_dir() {
                log::error!(
                    "Attempted to load a folder without `project_info.toml` or a `text/` folder. \
                     Consider creating a new project instead. If this was intended, please create \
                     one of the expected artifacts in that folder."
                );
                return Err(cheese_error!(
                    "attempted to load {path:?}, did not \
                         contain {PROJECT_INFO_NAME} or text folder"
                ));
            }
            log::warn!("Found `text/` but no project info file, creating it and continuing");
            DocumentMut::new()
        };

        let schema_identifier: String = match toml_header.get("schema") {
            Some(item) => match item.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return Err(cheese_error!(
                        "Invalid value found for 'schema' key: {item:?}"
                    ));
                }
            },
            None => {
                log::warn!("Project does not have a schema configured. Using default schema");
                toml_header["schema"] = toml_edit::value(DEFAULT_SCHEMA.get_schema_identifier());
                file_info.modified = true;
                DEFAULT_SCHEMA.get_schema_identifier().to_string()
            }
        };

        let schema = resolve_schema(&schema_identifier)?;

        // Load or create folders
        let mut objects = FileObjectStore::new();

        let top_level_folders = vec![
            load_top_level_folder(schema, &path, "Text", &mut objects)?,
            load_top_level_folder(schema, &path, "Characters", &mut objects)?,
            load_top_level_folder(schema, &path, "Worldbuilding", &mut objects)?,
        ];

        log::debug!("Finished loading all project file objects, continuing");

        base_metadata.load_base_metadata(toml_header.as_table(), &mut file_info)?;

        // Create the watcher path by hand since we can't call get_path() yet
        let watcher_path = file_info.dirname.join(&file_info.basename);

        // this might later get wrapped in an optional block or something but not worth it right now
        let (mut watcher, file_event_rx) =
            create_watcher().expect("Should always be able to create a watcher");

        watcher
            .watch(watcher_path, RecursiveMode::Recursive)
            .unwrap();

        let mut project = Self {
            schema,
            metadata,
            base_metadata,
            file: file_info,
            top_level_folders,
            toml_header,
            objects,
            event_queue: VecDeque::new(),
            last_added_event: None,
            file_event_rx,
            _watcher: watcher,
        };

        let metadata_modified = project.load_metadata()?;
        if metadata_modified {
            project.file.modified = true
        }

        project.clean_up_orphaned_objects();

        project.resolve_references();
        project.save()?;

        Ok(project)
    }

    pub fn add_object(&mut self, new_object: Box<dyn FileObject>) {
        let id = new_object.id().clone();
        self.objects.insert(id, RefCell::new(new_object));
    }

    pub fn save(&mut self) -> Result<(), CheeseError> {
        // First, try saving the children

        let results: Vec<Result<(), CheeseError>> = self
            .top_level_folders
            .iter()
            .map(|folder_id| {
                self.objects
                    .get(folder_id)
                    .unwrap()
                    .borrow_mut()
                    .save(&self.objects)
            })
            .collect();

        // Now save the project itself
        // unlike other file objects, this one doesn't rename automatically. This might be something
        // I want to add later, but it's currently intentional

        if self.file.modified {
            self.write_metadata();

            let final_str = self.toml_header.to_string();

            write_with_temp_file(self.get_project_info_file(), final_str)?;

            let new_modtime = std::fs::metadata(self.get_project_info_file())
                .expect("attempted to load file that does not exist")
                .modified()
                .expect("Modtime not available");

            // Update modtime based on what we just wrote
            self.file.modtime = Some(new_modtime);
            self.file.modified = false;
        }

        for result in results {
            result?
        }

        Ok(())
    }

    fn write_metadata(&mut self) {
        assert_eq!(
            self.toml_header["schema"]
                .as_str()
                .expect("schema should be a string"),
            self.schema.get_schema_identifier()
        );

        self.toml_header["file_format_version"] =
            toml_edit::value(self.base_metadata.version as i64);
        self.toml_header["name"] = toml_edit::value(&self.base_metadata.name);
        self.toml_header["id"] = toml_edit::value(&*self.base_metadata.id);

        self.toml_header["summary"] = toml_edit::value(&*self.metadata.summary);
        self.toml_header["notes"] = toml_edit::value(&*self.metadata.notes);
        self.toml_header["genre"] = toml_edit::value(&self.metadata.genre);
        self.toml_header["author"] = toml_edit::value(&self.metadata.author);
        self.toml_header["email"] = toml_edit::value(&self.metadata.email);

        // If the table doesn't already exist, we create it so we can get it immediately after
        if !self.toml_header.contains_key("export") {
            self.toml_header["export"] = toml_edit::value(toml_edit::InlineTable::new());
        }

        let export_table = self
            .toml_header
            .get_mut("export")
            .unwrap()
            .as_inline_table_mut()
            .unwrap();

        export_table.insert(
            "include_all_folder_titles",
            self.metadata.export.include_all_folder_titles.into(),
        );

        export_table.insert(
            "include_folder_title_depth",
            u64_to_i64_drop_msb(self.metadata.export.include_folder_title_depth).into(),
        );

        export_table.insert(
            "include_all_scene_files",
            self.metadata.export.include_all_scene_titles.into(),
        );
        export_table.insert(
            "include_scene_title_depth",
            u64_to_i64_drop_msb(self.metadata.export.include_scene_title_depth).into(),
        );
        export_table.insert(
            "insert_break_at_end",
            self.metadata.export.insert_break_at_end.into(),
        );
    }

    pub fn get_path(&self) -> PathBuf {
        Path::join(&self.file.dirname, &self.file.basename)
    }

    pub fn get_project_info_file(&self) -> PathBuf {
        let mut path = self.get_path();
        path.push(PROJECT_INFO_NAME);

        path
    }

    fn load_metadata(&mut self) -> Result<bool, CheeseError> {
        let mut modified = false;

        match metadata_extract_string(self.toml_header.as_table(), "summary")? {
            Some(summary) => self.metadata.summary = summary.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.toml_header.as_table(), "notes")? {
            Some(notes) => self.metadata.notes = notes.into(),
            None => modified = true,
        }

        match metadata_extract_string(self.toml_header.as_table(), "genre")? {
            Some(genre) => self.metadata.genre = genre,
            None => modified = true,
        }

        match metadata_extract_string(self.toml_header.as_table(), "author")? {
            Some(author) => self.metadata.author = author,
            None => modified = true,
        }

        match metadata_extract_string(self.toml_header.as_table(), "email")? {
            Some(email) => self.metadata.email = email,
            None => modified = true,
        }

        match self.toml_header.get("export") {
            Some(export_item) => match export_item.as_table_like() {
                Some(export_table) => {
                    match metadata_extract_bool(export_table, "include_all_folder_titles")? {
                        Some(val) => self.metadata.export.include_all_folder_titles = val,
                        None => modified = true,
                    }

                    match metadata_extract_u64(export_table, "include_folder_title_depth", false)? {
                        Some(val) => self.metadata.export.include_folder_title_depth = val,
                        None => modified = true,
                    }

                    match metadata_extract_bool(export_table, "include_all_scene_files")? {
                        Some(val) => self.metadata.export.include_all_scene_titles = val,
                        None => modified = true,
                    }

                    match metadata_extract_u64(export_table, "include_scene_title_depth", false)? {
                        Some(val) => self.metadata.export.include_scene_title_depth = val,
                        None => modified = true,
                    }

                    match metadata_extract_bool(export_table, "insert_break_at_end")? {
                        Some(val) => self.metadata.export.insert_break_at_end = val,
                        None => modified = true,
                    }
                }
                None => {
                    return Err(cheese_error!(
                        "Project Metadata has non-table value for export"
                    ));
                }
            },
            None => modified = true,
        }

        Ok(modified)
    }

    pub fn is_top_level_folder(&self, file_id: &FileID) -> bool {
        self.top_level_folders.contains(file_id)
    }

    /// Determine if the file should be loaded
    fn should_load(&mut self) -> Result<bool, CheeseError> {
        let current_modtime = std::fs::metadata(self.get_project_info_file())
            .expect("attempted to load file that does not exist")
            .modified()
            .expect("Modtime not available");

        if let Some(old_modtime) = self.file.modtime
            && old_modtime == current_modtime
        {
            // We've already loaded the latest revision, nothing to do
            return Ok(false);
        }

        Ok(true)
    }

    pub fn reload_file(&mut self) -> Result<(), CheeseError> {
        if !self.should_load()? {
            return Ok(());
        }

        let project_info_data = std::fs::read_to_string(self.get_project_info_file())?;

        let new_toml_header = project_info_data
            .parse::<DocumentMut>()
            .expect("invalid file metadata header");

        self.toml_header = new_toml_header;

        self.base_metadata
            .load_base_metadata(self.toml_header.as_table(), &mut self.file)?;
        self.load_metadata()?;

        Ok(())
    }

    /// Given a path, find the file ID. Right now, this is a pretty dumb algorithm that
    /// just visits every file object, gets its path, and compares it. This means it's
    /// O(n) path allocations, but it should be reliable.
    pub fn find_object_by_path(&self, object_path: &Path) -> Option<Rc<String>> {
        // If we have the metadata path, we're trying to find the object with the
        // parent of it, so we compute that path instead
        let compare_path = if object_path.ends_with("metadata.toml") {
            object_path.parent().expect("path should have a parent")
        } else {
            object_path
        };

        for (id, file_object) in self.objects.iter() {
            if file_object.borrow().get_path() == compare_path {
                return Some(id.clone());
            }
        }

        None
    }

    /// Given a FileID, try to find the FileID of its parent
    pub fn find_object_parent(&self, needle: &FileID) -> Option<FileID> {
        for object in self.objects.values() {
            if object.borrow().get_base().children.contains(needle) {
                return Some(object.borrow().id().clone());
            }
        }

        None
    }

    pub fn remove_path_from_parent(&self, object_path: &Path) -> Option<FileID> {
        let object_id = self.find_object_by_path(object_path)?;

        let parent_path = get_parent_path(object_path);
        let parent_id = self.find_object_by_path(parent_path)?;
        let mut parent_object = self.objects.get(&parent_id).unwrap().borrow_mut();

        let child_position = parent_object
            .get_base()
            .children
            .iter()
            .position(|id| *id == object_id)?;

        parent_object.get_base_mut().children.remove(child_position);

        Some(parent_id)
    }

    /// Export an outline to a string (which can be written to a file)
    pub fn export_outline(&self) -> String {
        let mut export_string = String::new();

        // Property at the top
        export_string.push_str("# ");
        export_string.push_str(&self.base_metadata.name);
        export_string.push_str("\n\n");

        write_outline_property("Story Summary", &self.metadata.summary, &mut export_string);

        for top_level_folder_id in &self.top_level_folders {
            let folder = self.objects.get(top_level_folder_id).unwrap().borrow();

            if !folder.get_base().children.is_empty() {
                if folder.get_base().metadata.name == "Text" {
                    export_string.push_str("# Scenes\n\n");
                } else {
                    export_string.push_str(&format!("# {}\n\n", folder.get_base().metadata.name));
                }

                for child_id in folder.get_base().children.iter() {
                    self.objects
                        .get(child_id)
                        .unwrap()
                        .borrow()
                        .generate_outline(2, &mut export_string, &self.objects);
                }

                export_string.push_str("\n\n");
            }
        }

        export_string
    }

    /// Export the story to a string (which can be written to a file)
    pub fn export_text(&self, export_options: ExportOptions) -> String {
        let mut export_string = String::new();

        let mut include_break = false;

        for child_id in self
            .objects
            .get(&self.top_level_folders[TEXT_FOLDER_POSITION])
            .unwrap()
            .borrow()
            .get_base()
            .children
            .iter()
        {
            include_break = self
                .objects
                .get(child_id)
                .unwrap()
                .borrow()
                .generate_export(
                    1,
                    &mut export_string,
                    &self.objects,
                    &export_options,
                    include_break,
                );
        }

        export_string
    }

    pub fn resolve_references(&mut self) {
        for object in self.objects.values() {
            object.borrow_mut().resolve_references(&self.objects);
        }
    }

    /// Get updates from the file watcher, can be called very frequently. Will only store
    /// them until `process_updates` is called
    pub fn receive_updates(&mut self) {
        // check for file system events and process them
        if let Ok(response) = self.file_event_rx.try_recv() {
            match response {
                Ok(events) => {
                    for event in events {
                        let mut git_event = false;
                        for event_path in event.paths.iter() {
                            if event_path.iter().any(|component| component == ".git") {
                                git_event = true;
                            }
                        }
                        if git_event {
                            continue;
                        }
                        if let EventKind::Access(_) = event.kind {
                            continue;
                        }

                        // We now have an event that isn't noise from .git or file opens:
                        log::debug!("found event: {event:?}");

                        self.event_queue.push_back(event);
                        self.last_added_event = Some(Instant::now());
                    }
                }
                Err(err) => log::warn!("Error while trying to watch files: {err:?}"),
            }
        }
    }

    /// Counterpart to receive_updates, should only be called immediately before a save
    pub fn process_updates(&mut self) {
        // Once we stop getting updates, we can process the list of events
        if let Some(last_event_time) = self.last_added_event
            && last_event_time.elapsed().as_millis() > (WATCHER_MSEC_DURATION * 2).into()
        {
            // Any file objects that should be rescanned at the end. This might be "wasted" sometimes
            // when a load also calls a rescan, but this is a super cheap operation
            let mut file_objects_needing_rescan = HashSet::new();

            // Paths that get loaded by `load_file`, either for a modification or a new file
            let mut paths_to_load = HashSet::new();

            // 1. process the entire event list, removing children that have been modified and storing
            // any new elements in a list to scan
            let queued_events: Vec<DebouncedEvent> = self.event_queue.drain(..).collect();
            for event in queued_events {
                match event.kind {
                    EventKind::Create(_create_kind) => {
                        let modify_path = event.paths.first().unwrap().to_owned();
                        log::debug!("processing creation event: {event:?}");
                        paths_to_load.insert(modify_path);
                    }
                    EventKind::Modify(ModifyKind::Data(_data_change)) => {
                        let modify_path = event.paths.first().unwrap().to_owned();
                        log::debug!("processing modify event: {event:?}");
                        if let Some(parent_id) = self.remove_path_from_parent(&modify_path) {
                            file_objects_needing_rescan.insert(parent_id);
                        }
                        paths_to_load.insert(modify_path);
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                        let delete_path = event
                            .paths
                            .first()
                            .expect("From rename should have a source");

                        log::debug!("processing rename(from) event as delete: {event:?}");

                        if let Some(parent_id) = self.remove_path_from_parent(delete_path) {
                            file_objects_needing_rescan.insert(parent_id);
                        }
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                        let modify_path = event
                            .paths
                            .last()
                            .expect("to event should have a destination")
                            .to_owned();

                        log::debug!("processing rename(to) as modify event: {event:?}");
                        if let Some(parent_id) = self.remove_path_from_parent(&modify_path) {
                            file_objects_needing_rescan.insert(parent_id);
                        }
                        paths_to_load.insert(modify_path);
                    }
                    EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                        let source_path = event
                            .paths
                            .first()
                            .expect("Rename event should have source");

                        let dest_path = event
                            .paths
                            .last()
                            .expect("Rename event should have destination")
                            .to_owned();

                        log::debug!("processing rename event: {event:?}");
                        if let Some(parent_id) = self.remove_path_from_parent(source_path) {
                            file_objects_needing_rescan.insert(parent_id);
                        }
                        paths_to_load.insert(dest_path);
                    }
                    EventKind::Modify(_) => {
                        log::debug!(
                            "Found unknown modify event: {event:?}, trying to process anyway"
                        );

                        let source_path_option = event.paths.first();

                        let dest_path_option = event.paths.last();

                        if let Some(dest_path) = dest_path_option
                            && let Some(source_path) = source_path_option
                        {
                            if let Some(parent_id) = self.remove_path_from_parent(source_path) {
                                file_objects_needing_rescan.insert(parent_id);
                            }
                            paths_to_load.insert(dest_path.to_owned());
                        } else {
                            log::debug!("unable to process modify(any) event");
                        }
                    }
                    EventKind::Remove(_remove_kind) => {
                        let delete_path = event
                            .paths
                            .first()
                            .expect("Rename event should have source");

                        if let Some(parent_id) = self.remove_path_from_parent(delete_path) {
                            file_objects_needing_rescan.insert(parent_id);
                        }
                    }
                    _ => {}
                }
            }

            // 2. remove all duplicate paths from this list (because I like writing extra code to avoid
            // probably harmless disk usage apparently)
            let paths_to_load_clone = paths_to_load.clone();
            for path1 in &paths_to_load_clone {
                for path2 in &paths_to_load_clone {
                    if path1 == path2 {
                        continue;
                    }
                    if *path2 == self.get_path() {
                        // special case: if we get an event for the entire project, discard it
                        paths_to_load.remove(path2);
                    } else if path1.starts_with(path2) {
                        paths_to_load.remove(path1);
                    }
                }
            }

            // I already did step 3 but I'm writing it out here because it makes more sense that way:
            // 3. every element that needs to be rescanned is removed from their parents (the list of
            //    children), the parents are stored in the needs_reindex set

            let project_info_file = self.get_project_info_file();

            // 4. load all of the objects we wanted to rescan
            for path_to_load in paths_to_load {
                if path_to_load == project_info_file {
                    if let Err(err) = self.reload_file() {
                        log::warn!("Error while reloading project info file: {err}");
                    }
                    continue;
                }

                let event_path = if path_to_load.file_name().and_then(|name| name.to_str())
                    == Some(FOLDER_METADATA_FILE_NAME)
                {
                    path_to_load.parent().unwrap().to_owned()
                } else {
                    path_to_load
                };

                if !self.is_relevant_event_path(&event_path) {
                    continue;
                }

                match self.schema.load_file(&event_path, &mut self.objects) {
                    Ok(file_id) => {
                        // Check if we have a normal file path or a top level folder. We should
                        // possibly filter these out even higher up, but we're already here so
                        // we need to handle it correctly
                        if self.is_top_level_folder(&file_id) {
                            file_objects_needing_rescan.insert(file_id);
                        } else {
                            let parent_path = get_parent_path(&event_path);
                            let parent_id_option = self.find_object_by_path(parent_path);
                            if let Some(parent_id) = parent_id_option {
                                let parent_object = self.objects.get(&parent_id).unwrap();
                                let parent_has_child = parent_object
                                    .borrow()
                                    .get_base()
                                    .children
                                    .contains(&file_id);
                                if !parent_has_child {
                                    parent_object
                                        .borrow_mut()
                                        .get_base_mut()
                                        .children
                                        .push(file_id);
                                }

                                file_objects_needing_rescan.insert(parent_id);
                            } else {
                                log::debug!(
                                    "Could not find parent object: {parent_path:?} while processing updates. \
                                Ignoring for now, maybe it will appear later (or be cleaned up)"
                                );
                            }
                        }
                    }
                    Err(err) => log::debug!("Could not load {event_path:?}: {err}"),
                }
            }

            // 5. Rescan anything that needs it
            for object_needing_rescan in file_objects_needing_rescan {
                self.objects
                    .get(&object_needing_rescan)
                    .unwrap()
                    .borrow_mut()
                    .rescan_indexing(&self.objects, true);
            }

            // 6. Clean up any dangling objects
            self.clean_up_orphaned_objects();

            log::debug!(
                "finished processing event queue at {:?}",
                std::time::Instant::now()
            );
            self.last_added_event = None;

            // 7. Any other steps
            self.resolve_references();
        }
    }

    pub fn clean_up_orphaned_objects(&mut self) {
        // Start by getting a set of all objects
        let mut dangling: HashSet<Rc<String>> = HashSet::from_iter(
            self.objects
                .keys()
                .filter(|file_id| !self.is_top_level_folder(file_id))
                .cloned(),
        );

        // Visit every object and remove all children from the dangling list. This will
        // not find cycles, but if there are cycles in our tree we have bigger problems
        for file_object in self.objects.values() {
            let file_object = file_object.borrow();
            for child in file_object.get_base().children.iter() {
                if !dangling.remove(child) {
                    // I might regret making this a panic instead of a log, but it
                    // shouldn't be possible (and I'm not sure how to recover)
                    panic!("Found two occurances of child {child} in objects");
                }
            }
            if file_object.get_base().index.is_none() && !self.is_top_level_folder(file_object.id())
            {
                panic!("Found file object {file_object} with index None");
            }
        }

        // If there are any objects left, these are the roots of the trees of dangling
        // objects and can safely be removed (since they don't exist on disk anymore)
        if !dangling.is_empty() {
            log::debug!("Found dangling file objects, removing them now");

            let mut queue_to_remove = VecDeque::from_iter(dangling);

            while let Some(to_remove) = queue_to_remove.pop_front() {
                if let Some(removed_object) = self.objects.remove(&to_remove) {
                    log::debug!("Removed dangling file object: {removed_object:?}");

                    // There might be a better way to do this (since we're dropping it anyway), but
                    // this is easy
                    for child in removed_object.borrow().get_base().children.iter() {
                        queue_to_remove.push_back(child.clone());
                    }
                } else {
                    log::error!("Could not remove file object: {to_remove}");
                }
            }
        }
    }

    /// Determine if we care about an event happening at this path. This filters out things like events
    /// starting with `.git/`, hidden files (on linux), unknown extensions, or files not in one of the
    /// three top level folders
    ///
    /// It does not check for files existing, and does not do anything specific to modification types.
    /// The string argument is only to provide better log message output
    fn is_relevant_event_path(&self, modify_path: &Path) -> bool {
        // We assume that any files that don't have an extension are folders but this function
        // not checking disk means we can't verify that
        if modify_path
            .extension()
            .is_some_and(|extension| extension != "md" && extension != "toml")
        {
            // we write .tmp files and then immediately remove them and other editors can do the same
            // we also don't care about files that other programs generate
            return false;
        }

        if modify_path
            .file_name()
            .is_none_or(|filename| filename.to_string_lossy().starts_with('.'))
        {
            // modified files should have a name, and we don't want to look at hidden files
            return false;
        }

        let relative_path = match modify_path.strip_prefix(self.get_path()) {
            Ok(relative_path) => relative_path,
            Err(err) => {
                log::error!("invalid event path not in project: {err}");
                return false;
            }
        };

        if !(relative_path.starts_with("text")
            || relative_path.starts_with("characters")
            || relative_path.starts_with("worldbuilding"))
        {
            if !relative_path.starts_with(".git") {
                // We expect a bunch of git events, but other events are unexpected, so log it
                log::debug!("invalid event path not in project folders: {modify_path:?}");
            }
            return false;
        }

        true
    }

    #[cfg(test)]
    pub fn get_text_folder(&self) -> &RefCell<Box<dyn FileObject>> {
        self.objects.get(&self.top_level_folders[0]).unwrap()
    }
}

fn get_parent_path(object_path: &Path) -> &Path {
    let object_base = if object_path.ends_with("metadata.toml") {
        object_path.parent().expect("path should have a parent")
    } else {
        object_path
    };

    object_base
        .parent()
        .expect("file objects should have a parent")
}

fn u64_to_i64_drop_msb(val: u64) -> i64 {
    const MSB_MASK: u64 = u64::MAX >> 1;
    (val & MSB_MASK) as i64
}

pub struct ExportOptions {
    pub folder_title_depth: ExportDepth,
    pub scene_title_depth: ExportDepth,
    pub insert_breaks: bool,
}

pub enum ExportDepth {
    All,
    Some(u64),
    None,
}

impl ExportDepth {
    pub fn should_display(&self, depth: u64) -> bool {
        match self {
            ExportDepth::All => true,
            ExportDepth::Some(max_depth) => depth <= *max_depth,
            ExportDepth::None => false,
        }
    }
}
