use crate::components::Schema;
use crate::components::file_objects::FileObjectStore;

use crate::components::file_objects::{FileID, FileObject, utils::write_with_temp_file};

use crate::components::project::Project;
use crate::util::CheeseError;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::create_dir;
use std::fs::{read_dir, read_to_string};
use std::path::Path;

use std::cell::RefCell;
use std::rc::Rc;
use std::{fmt::Display, thread, time};

use crate::schemas::FileType;
use crate::schemas::SCHEMA_LIST;

/// These tests were not written to be agnostic to the kind of file types that exist.
/// Rather than re-write them immediately, it is best to make an iteration of the tests which
/// are not changed in functionality, and test if the post-refactor code still behaves the same way
/// For this purpose, here is a 'hack' to give us access to the file types which are private
/// to the schema::default module
use crate::schemas::export_file_types::{CHARACTER, FOLDER, PLACE, SCENE};

const SCHEMA: &'static dyn Schema = &crate::schemas::DEFAULT_SCHEMA;

fn file_id(s: &str) -> Rc<String> {
    Rc::new(s.to_string())
}

/// Call process_updates twice with more time than the WATCHER_MSEC_DURATION
/// to make sure it actually gets woken up and runs
#[cfg(test)]
fn process_updates(project: &mut Project) {
    for _ in 0..5 {
        thread::sleep(time::Duration::from_millis(60));
        project.process_updates();
    }
}

/// Sleep and call process_updates twice with more time than the WATCHER_MSEC_DURATION
/// to make sure it actually gets woken up and runs
#[cfg(test)]
fn save_and_process_updates(project: &mut Project) {
    process_updates(project);

    project.save().unwrap();

    process_updates(project);
}

/// Helper to get the file id from a path
fn get_id_from_file(filename: &Path) -> Option<FileID> {
    use toml_edit::DocumentMut;

    use crate::components::file_objects::FOLDER_METADATA_FILE_NAME;
    use crate::components::file_objects::utils::read_file_contents;

    if !filename.exists() {
        return None;
    }

    // If the filename is a directory, we need to look for the underlying file, otherwise
    // we already have it
    let underlying_file = match filename.is_dir() {
        true => Path::join(filename, FOLDER_METADATA_FILE_NAME),
        false => filename.to_path_buf(),
    };

    let (metadata_str, _file_body) = match read_file_contents(&underlying_file) {
        Ok((metadata_str, file_body)) => (metadata_str, file_body),
        Err(err) => {
            if !filename.is_dir() {
                log::error!("Failed to read file {:?}: {:?}", &underlying_file, err);
            }
            return None;
        }
    };

    let toml_header = match metadata_str.parse::<DocumentMut>() {
        Ok(toml_header) => toml_header,
        Err(err) => {
            log::error!("Error parsing {underlying_file:?}: {err}");
            return None;
        }
    };

    if let Some(id_item) = toml_header.get("id")
        && let Some(id_string) = id_item.as_str()
    {
        use crate::components::file_objects::FileID;

        return Some(FileID::new(id_string.to_string()));
    }

    None
}

#[test]
/// Ensure that projects are created properly
fn test_basic_create_project() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let project_name = "test project";
    let project_path = base_dir.path().join("test_project");

    assert!(!project_path.exists());
    assert_eq!(read_dir(base_dir.path()).unwrap().count(), 0);

    let project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        project_name.to_string(),
    )
    .unwrap();

    assert_eq!(project_path.canonicalize().unwrap(), project.get_path());

    assert_eq!(read_dir(base_dir.path()).unwrap().count(), 1);
    assert!(project_path.exists());
    assert_eq!(read_dir(&project_path).unwrap().count(), 4);

    // Ensure that the file is at least populated
    assert_ne!(
        read_to_string(project.get_project_info_file())
            .unwrap()
            .len(),
        0
    );
}

#[test]
/// Ensure that file_objects are created properly
fn test_basic_create_file_object() -> Result<(), CheeseError> {
    let base_dir = tempfile::TempDir::new()?;

    let scene = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 0)
        .unwrap();
    let character = SCHEMA
        .create_file(CHARACTER, base_dir.path().to_path_buf(), 0)
        .unwrap();
    let folder = SCHEMA
        .create_file(FOLDER, base_dir.path().to_path_buf(), 0)
        .unwrap();
    let place = SCHEMA
        .create_file(PLACE, base_dir.path().to_path_buf(), 0)
        .unwrap();

    // Ensure that all four of the files exist in the proper place
    assert_eq!(read_dir(base_dir.path()).unwrap().count(), 4);
    assert_eq!(
        scene.get_base().file.basename,
        OsString::from("000-New_Scene.md")
    );
    assert_eq!(
        character.get_base().file.basename,
        OsString::from("000-New_Character.toml")
    );
    assert_eq!(
        folder.get_base().file.basename,
        OsString::from("000-New_Folder")
    );
    assert_eq!(
        place.get_base().file.basename,
        OsString::from("000-New_Place")
    );

    // Ensure that folders are created with the metadata.toml file
    assert_eq!(read_dir(folder.get_path()).unwrap().count(), 1);
    assert_eq!(read_dir(place.get_path()).unwrap().count(), 1);

    // Ensure that the files contain stuff
    assert_ne!(read_to_string(scene.get_file()).unwrap().len(), 0);
    assert_ne!(read_to_string(character.get_file()).unwrap().len(), 0);
    assert_ne!(read_to_string(folder.get_file()).unwrap().len(), 0);
    assert_ne!(read_to_string(place.get_file()).unwrap().len(), 0);

    Ok(())
}

#[test]
/// Ensure that top level folders work the way we want
fn test_create_top_level_folder() -> Result<(), CheeseError> {
    let base_dir = tempfile::TempDir::new()?;

    let text = SCHEMA.create_top_level_folder(base_dir.path().to_path_buf(), "Text")?;

    assert_eq!(read_dir(base_dir.path())?.count(), 1);
    assert_eq!(read_dir(text.get_path())?.count(), 1);

    assert_eq!(text.get_path().file_name().unwrap(), "text");
    assert_eq!(text.get_base().index, None);
    assert_eq!(text.get_base().metadata.name, "Text");

    Ok(())
}

#[test]
/// Ensure names actually get truncated when saving (there are other tests that cover truncation
/// behavior in more depth), and that names get characters removed
fn test_complicated_file_object_names() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut scene = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 0)
        .unwrap();

    let scene1 = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 1)
        .unwrap();

    scene.get_base_mut().metadata.name =
        "This is a really long scene name that will have to be shortened".to_string();
    scene.get_base_mut().file.modified = true;

    scene.save(&HashMap::new()).unwrap();

    // This is probably getting too far into specifics of behavior for this test,
    // but I don't think it'll change very much, so I'm writing it like this now
    assert_eq!(
        scene.get_file().file_name().unwrap(),
        "000-This_is_a_really_long_scene.md"
    );

    assert_eq!(read_dir(base_dir.path()).unwrap().count(), 2);
    assert!(scene.get_file().exists());
    assert_ne!(read_to_string(scene.get_file()).unwrap().len(), 0);

    scene.get_base_mut().metadata.name = "Difficult(to)ParseName/Bad_ ".to_string();
    scene.get_base_mut().file.modified = true;
    scene.save(&HashMap::new()).unwrap();

    assert_eq!(read_dir(base_dir.path()).unwrap().count(), 2);
    assert!(scene.get_file().exists());
    assert_ne!(read_to_string(scene.get_file()).unwrap().len(), 0);

    assert_eq!(
        scene.get_file().file_name().unwrap(),
        "000-Difficult(to)ParseName-Bad_.md"
    );

    // At the end, ensure we didn't clobber the other scene somehow
    assert_eq!(
        scene1.get_base().file.basename,
        OsString::from("001-New_Scene.md")
    );
    assert!(scene1.get_file().exists());
    assert_ne!(read_to_string(scene1.get_file()).unwrap().len(), 0);
}

#[test]
fn test_change_index_scene() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut scene = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 0)
        .unwrap();

    let scene1 = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 1)
        .unwrap();

    scene.load_body("sample scene text".to_string());
    scene.get_base_mut().file.modified = true;
    scene.save(&HashMap::new()).unwrap();

    scene.set_index(2, &HashMap::new()).unwrap();

    // Make sure the untouched scene didn't change somehow
    assert_eq!(
        scene1.get_base().file.basename,
        OsString::from("001-New_Scene.md")
    );
    assert!(scene1.get_file().exists());
    assert_ne!(read_to_string(scene1.get_file()).unwrap().len(), 0);

    // Make sure the moved scene is at the expected path
    assert_eq!(scene.get_base().index, Some(2));
    assert_eq!(
        scene.get_base().file.basename,
        OsString::from("002-New_Scene.md")
    );
    assert!(scene.get_file().exists());

    // Make sure the file contents moved with it
    let scene_text_full = read_to_string(scene.get_file()).unwrap();
    assert_ne!(scene_text_full.len(), 0);
    assert!(scene_text_full.contains("sample scene text"));
}

#[test]
fn test_create_child() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    // create the scenes
    let scene = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();

    let character = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(CHARACTER)
        .unwrap();

    let folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();

    let place = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(PLACE)
        .unwrap();

    // Four file objects plus the metadata
    let path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();
    assert_eq!(read_dir(path).unwrap().count(), 5);
    assert!(scene.get_file().exists());
    assert_eq!(scene.get_base().index, Some(0));
    assert!(character.get_file().exists());
    assert_eq!(character.get_base().index, Some(1));
    assert!(folder.get_file().exists());
    assert_eq!(folder.get_base().index, Some(2));
    assert!(place.get_file().exists());
    assert_eq!(place.get_base().index, Some(3));
}

#[test]
fn test_set_index_folders() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut top_level_folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();

    let mut mid_level_folder = top_level_folder.create_child_at_end(FOLDER).unwrap();
    let child_scene = mid_level_folder.create_child_at_end(SCENE).unwrap();
    let child_scene_id = child_scene.get_base().metadata.id.clone();

    assert!(child_scene.get_file().exists());
    assert_eq!(child_scene.get_base().index, Some(0));

    project.add_object(mid_level_folder);
    project.add_object(child_scene);

    top_level_folder.set_index(1, &project.objects).unwrap();

    let child = project.objects.remove(&child_scene_id).unwrap();

    assert_eq!(child.borrow().get_base().index, Some(0));
    assert!(child.borrow().get_file().exists());

    assert!(
        child
            .borrow()
            .get_path()
            .ends_with("000-New_Folder/000-New_Scene.md")
    );
}

#[test]
/// Run save on a folder and scene without changing anything, ensure that they don't get re-writtten
fn test_avoid_pointless_save() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut scene = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 0)
        .unwrap();

    let scene_old_modtime = scene.get_base().file.modtime;
    // Check that we get the correct modtime
    assert_eq!(scene.get_base().file.modtime, scene_old_modtime);

    // Try to save again, we shouldn't do anything
    scene.save(&HashMap::new()).unwrap();
    assert_eq!(scene.get_base().file.modtime, scene_old_modtime);

    let mut folder = SCHEMA
        .create_file(FOLDER, base_dir.path().to_path_buf(), 1)
        .unwrap();

    let folder_old_modtime = folder.get_base().file.modtime;
    folder.save(&HashMap::new()).unwrap();
    assert_eq!(folder.get_base().file.modtime, folder_old_modtime);
}

#[test]
fn test_save_in_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let sample_text = "sample body";

    let mut folder = SCHEMA
        .create_file(FOLDER, base_dir.path().to_path_buf(), 0)
        .unwrap();

    let mut scene = folder.create_child_at_end(SCENE).unwrap();

    scene.load_body(sample_text.to_owned());
    scene.get_base_mut().file.modified = true;

    let scene_id = scene.get_base().metadata.id.clone();

    let mut map: FileObjectStore = HashMap::new();
    let id = scene.get_base().metadata.id.clone();
    map.insert(id, RefCell::new(scene));

    folder.save(&map).unwrap();

    let scene = map.get(&scene_id).unwrap();
    assert!(!scene.borrow().get_base().file.modified);
    assert!(scene.borrow().get_file().exists());
    assert!(
        read_to_string(scene.borrow().get_file())
            .unwrap()
            .contains(sample_text)
    );
}

#[test]
fn test_reload_objects() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let sample_body = "sample body";
    let scene_summary = "it gets more gay";
    let character_appearance = "tall";
    let folder_notes = "this is a folder";
    let place_description = "lots and lots of trees!";

    let mut scene = SCHEMA
        .create_file(SCENE, base_dir.path().to_path_buf(), 0)
        .unwrap();
    let mut character = SCHEMA
        .create_file(CHARACTER, base_dir.path().to_path_buf(), 1)
        .unwrap();
    let mut folder = SCHEMA
        .create_file(FOLDER, base_dir.path().to_path_buf(), 2)
        .unwrap();
    let mut place = SCHEMA
        .create_file(PLACE, base_dir.path().to_path_buf(), 3)
        .unwrap();

    scene.load_body(sample_body.to_string());
    *scene.get_test_field() = scene_summary.to_string();
    scene.get_base_mut().file.modified = true;

    *character.get_test_field() = character_appearance.to_string();
    character.get_base_mut().file.modified = true;

    *folder.get_test_field() = folder_notes.to_string();
    folder.get_base_mut().file.modified = true;

    *place.get_test_field() = place_description.to_string();
    place.get_base_mut().file.modified = true;

    // Save all of the objects
    scene.save(&HashMap::new()).unwrap();
    character.save(&HashMap::new()).unwrap();
    folder.save(&HashMap::new()).unwrap();
    place.save(&HashMap::new()).unwrap();

    // Keep track of paths and ids
    let scene_path = scene.get_path();
    let scene_id = scene.get_base().metadata.id.clone();
    let character_path = character.get_path();
    let character_id = character.get_base().metadata.id.clone();
    let folder_path = folder.get_path();
    let folder_id = folder.get_base().metadata.id.clone();
    let place_path = place.get_path();
    let place_id = place.get_base().metadata.id.clone();

    // Drop all of the objects (just to make sure we're reloading them)
    drop(scene);
    drop(character);
    drop(folder);
    drop(place);

    let mut objects = FileObjectStore::new();

    let scene_id_loaded = SCHEMA.load_file(&scene_path, &mut objects).unwrap();
    let character_id_loaded = SCHEMA.load_file(&character_path, &mut objects).unwrap();
    let folder_id_loaded = SCHEMA.load_file(&folder_path, &mut objects).unwrap();
    let place_id_loaded = SCHEMA.load_file(&place_path, &mut objects).unwrap();

    assert_eq!(scene_id, scene_id_loaded);
    let mut scene_loaded = objects.get(&scene_id).unwrap().borrow_mut();
    assert_eq!(scene_loaded.get_type(), SCENE);
    assert_eq!(scene_loaded.get_body().trim(), sample_body);
    assert_eq!(*scene_loaded.get_test_field(), scene_summary);

    assert_eq!(character_id, character_id_loaded);
    let mut character_loaded = objects.get(&character_id).unwrap().borrow_mut();
    assert_eq!(character_id, character_id_loaded);
    assert_eq!(*character_loaded.get_test_field(), character_appearance);

    assert_eq!(folder_id, folder_id_loaded);
    let mut folder_loaded = objects.get(&folder_id).unwrap().borrow_mut();
    assert_eq!(folder_id, folder_id_loaded);
    assert_eq!(*folder_loaded.get_test_field(), folder_notes);

    assert_eq!(place_id, place_id_loaded);
    let mut place_loaded = objects.get(&place_id).unwrap().borrow_mut();
    assert_eq!(place_id, place_id_loaded);
    assert_eq!(*place_loaded.get_test_field(), place_description);
}

#[test]
fn test_reload_project() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let sample_body = "sample body";

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut scene = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    let scene_id = scene.get_base().metadata.id.clone();

    let character = project
        .objects
        .get(&project.characters_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(CHARACTER)
        .unwrap();
    let character_id = character.get_base().metadata.id.clone();

    let folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    let folder_id = folder.get_base().metadata.id.clone();

    let place = project
        .objects
        .get(&project.worldbuilding_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(PLACE)
        .unwrap();
    let place_id = place.get_base().metadata.id.clone();

    // modify the file objects:
    scene.load_body(sample_body.to_string());
    scene.get_base_mut().file.modified = true;

    project.add_object(scene);
    project.add_object(character);
    project.add_object(folder);
    project.add_object(place);

    project.save().unwrap();

    let project_path = project.get_path();

    drop(project);

    let project = Project::load(project_path).unwrap();

    // Verify the counts in each folder are correct:
    // Text (scene, folder + metadata)
    assert_eq!(
        read_dir(
            project
                .objects
                .get(&project.text_id)
                .unwrap()
                .borrow()
                .get_path()
        )
        .unwrap()
        .count(),
        3
    );

    // Characters (character + metadata)
    assert_eq!(
        read_dir(
            project
                .objects
                .get(&project.characters_id)
                .unwrap()
                .borrow()
                .get_path()
        )
        .unwrap()
        .count(),
        2
    );

    // Worldbuilding (place + metadata)
    assert_eq!(
        read_dir(
            project
                .objects
                .get(&project.worldbuilding_id)
                .unwrap()
                .borrow()
                .get_path()
        )
        .unwrap()
        .count(),
        2
    );

    // Now inspect the file objects
    let scene = project.objects.get(&scene_id).unwrap();
    let character = project.objects.get(&character_id).unwrap();
    let folder = project.objects.get(&folder_id).unwrap();
    let place = project.objects.get(&place_id).unwrap();

    // Go through each folder:
    // Text (scene, folder + metadata)
    assert!(scene.borrow().get_file().exists());
    assert_eq!(scene.borrow().get_base().index, Some(0));
    assert!(scene.borrow().get_body().contains(sample_body));

    assert!(folder.borrow().get_file().exists());
    assert_eq!(folder.borrow().get_base().index, Some(1));

    // Characters (character + metadata)
    assert!(character.borrow().get_file().exists());
    assert_eq!(character.borrow().get_base().index, Some(0));

    // Worldbuilding (place + metadata)
    assert!(place.borrow().get_file().exists());
    assert_eq!(place.borrow().get_base().index, Some(0));
}

/// Make sure that a `.md` file gets loaded without a text editor
#[test]
fn test_load_markdown() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let sample_body = "sample body";

    // open and immediately drop the project (just creating the files)
    Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/000-New_Scene.md"),
        sample_body.as_bytes(),
    )
    .unwrap();

    let project = Project::load(base_dir.path().join("test_project")).unwrap();

    let text_child = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_base()
        .children
        .first()
        .unwrap()
        .clone();

    assert_eq!(
        project
            .objects
            .get(&text_child)
            .unwrap()
            .borrow()
            .get_body()
            .trim(),
        sample_body
    );
}

/// Make sure metadata gets filled in
#[test]
fn test_load_partial_metadata() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let file_text = r#"id = "1"
name = "Other title"
summary = """multiline block inside
another multiline block
"""
++++++++
contents1
"#;
    // open and immediately drop the project (just creating the files)
    Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/000-New_Scene.md"),
        file_text.as_bytes(),
    )
    .unwrap();

    let mut project = Project::load(base_dir.path().join("test_project")).unwrap();
    project.save().unwrap();

    let scene_path = project
        .objects
        .get(&Rc::new("1".to_string()))
        .unwrap()
        .borrow()
        .get_path();

    let scene_id_loaded = SCHEMA.load_file(&scene_path, &mut project.objects).unwrap();

    let scene = project.objects.get(&scene_id_loaded).unwrap();
    let mut scene = scene.borrow_mut();

    assert!(scene.get_type() == SCENE);
    assert_eq!(scene.get_body().trim(), "contents1");
    assert_eq!(
        *scene.get_test_field(),
        "multiline block inside\nanother multiline block\n"
    );

    assert!(
        read_to_string(scene_path)
            .unwrap()
            .contains(r#"notes = """#)
    );
}

/// Make sure that the filename is kept when an object gets renamed
#[test]
fn test_name_from_filename() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let text_path = SCHEMA
        .create_top_level_folder(base_dir.path().to_path_buf(), "Text")
        .unwrap()
        .get_path();

    let mut objects = FileObjectStore::new();

    write_with_temp_file(
        &text_path.join("4-scene2.md"),
        "contents1".to_string().as_bytes(),
    )
    .unwrap();

    let scene_id_loaded = SCHEMA.load_file(&text_path, &mut objects).unwrap();
    let folder = objects.get(&scene_id_loaded).unwrap();
    let mut folder = folder.borrow_mut();

    assert_eq!(folder.get_type(), FOLDER);
    folder.save(&objects).unwrap();
    assert!(folder.get_path().join("000-scene2.md").exists());
}

/// Load various files with indexes out of order (and some missing) and verify that they all get indexed correctly
#[test]
fn test_fix_indexing_on_load() {
    // Create files with known id for convenience, verify that the children dict ends up in the expect place
    // after loading

    let base_dir = tempfile::TempDir::new().unwrap();

    let text_path = SCHEMA
        .create_top_level_folder(base_dir.path().to_path_buf(), "Text")
        .unwrap()
        .get_path();

    write_with_temp_file(
        &text_path.join("4-scene2.md"),
        r#"id = "0"
++++++++"#
            .to_string()
            .as_bytes(),
    )
    .unwrap();

    std::fs::create_dir(text_path.join("05-dir")).unwrap();

    write_with_temp_file(
        &text_path.join("05-dir/metadata.toml"),
        r#"id = "1""#.to_string().as_bytes(),
    )
    .unwrap();

    write_with_temp_file(
        &text_path.join("05-dir/2-scene.md"),
        r#"id = "1-0"
++++++++
contents123"#
            .to_string()
            .as_bytes(),
    )
    .unwrap();

    write_with_temp_file(
        &text_path.join("10-scene2.md"),
        r#"id = "2"
++++++++"#
            .to_string()
            .as_bytes(),
    )
    .unwrap();

    write_with_temp_file(
        &text_path.join("scene_no_index.md"),
        r#"id = "3"
++++++++"#
            .to_string()
            .as_bytes(),
    )
    .unwrap();

    let mut objects = FileObjectStore::new();

    let scene_id_loaded = SCHEMA.load_file(&text_path, &mut objects).unwrap();
    let folder = objects.get(&scene_id_loaded).unwrap();
    let mut folder = folder.borrow_mut();

    assert_eq!(folder.get_type(), FOLDER);
    folder.save(&objects).unwrap();
    assert!(folder.get_path().join("000-scene2.md").exists());

    let child = objects.get(&file_id("1-0")).unwrap();
    assert_eq!(child.borrow().get_base().index, Some(0));
    assert_eq!(child.borrow().get_body(), "contents123\n");
}

/// Try to delete a file object, verifying it gets removed from disk
#[test]
fn test_delete() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    // baseline:
    assert!(project.get_path().join("text/000-folder1/").exists());

    assert!(
        project
            .get_path()
            .join("text/000-folder1/000-scene1.md")
            .exists()
    );

    assert!(
        project
            .get_path()
            .join("text/000-folder1/001-scene2.md")
            .exists()
    );

    <dyn FileObject>::remove_child(&scene2_id, &folder1_id, &mut project.objects).unwrap();

    // we should have removed the ending scene, check on disk
    assert!(project.get_path().join("text/000-folder1/").exists());

    assert!(
        project
            .get_path()
            .join("text/000-folder1/000-scene1.md")
            .exists()
    );

    assert!(
        !project
            .get_path()
            .join("text/000-folder1/001-scene2.md")
            .exists()
    );

    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene1_id
    );

    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    assert!(!project.objects.contains_key(&scene2_id));

    // Now, try to remove the folder
    <dyn FileObject>::remove_child(&folder1_id, &project.text_id, &mut project.objects).unwrap();

    // we should have removed the ending scene, check on disk
    assert!(project.get_path().join("text/metadata.toml").exists());

    assert!(!project.get_path().join("text/000-folder1/").exists());

    assert!(
        !project
            .get_path()
            .join("text/000-folder1/001-scene2.md")
            .exists()
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    assert!(!project.objects.contains_key(&scene1_id));
    assert!(!project.objects.contains_key(&folder1_id));
}

/// Try to delete a file object in the middle of a folder, ensuring indexing works correctly afterwards
#[test]
fn test_delete_middle() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene2);
    project.add_object(scene1);
    project.save().unwrap();

    <dyn FileObject>::remove_child(&folder1_id, &project.text_id, &mut project.objects).unwrap();

    assert!(!project.get_path().join("text/000-folder1/").exists());
    assert!(project.get_path().join("text/000-scene2.md").exists());

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    assert!(!project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene2_id));
}

/// Simple move, move a scene from the end of one folder to the end of another
#[test]
fn test_move_simple() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut folder2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder2.get_base_mut().metadata.name = "folder2".to_string();
    folder2.get_base_mut().file.modified = true;

    let mut scene_to_move = folder1.create_child_at_end(SCENE).unwrap();
    scene_to_move.get_base_mut().metadata.name = "scene1".to_string();
    scene_to_move.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let folder2_id = folder2.get_base().metadata.id.clone();
    let scene_id = scene_to_move.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(folder2);
    project.add_object(scene_to_move);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(!project_path.join("text/001-folder2/000-scene1.md").exists());

    // Do the move
    SCHEMA
        .move_child(&scene_id, &folder1_id, &folder2_id, 0, &project.objects)
        .unwrap();

    // Verify that the move happened on disk
    assert!(!project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder2/000-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );
}

/// Try moving file multiple times to make sure we're setting properties correctly
#[test]
fn test_move_multiple_times() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut folder2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder2.get_base_mut().metadata.name = "folder2".to_string();
    folder2.get_base_mut().file.modified = true;

    let mut scene_to_move: Box<dyn FileObject> = folder1.create_child_at_end(SCENE).unwrap();
    scene_to_move.get_base_mut().metadata.name = "scene1".to_string();
    scene_to_move.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let folder2_id = folder2.get_base().metadata.id.clone();
    let scene_id = scene_to_move.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(folder2);
    project.add_object(scene_to_move);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(!project_path.join("text/001-folder2/000-scene1.md").exists());

    // Do the first move
    SCHEMA
        .move_child(&scene_id, &folder1_id, &folder2_id, 0, &project.objects)
        .unwrap();

    // Verify that the move happened on disk
    assert!(!project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder2/000-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    // Do the second move (back)
    SCHEMA
        .move_child(&scene_id, &folder2_id, &folder1_id, 0, &project.objects)
        .unwrap();

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    assert!(project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(!project_path.join("text/001-folder2/000-scene1.md").exists());
}

/// Move a folder that contains things. Almost the same as `test_move_simple`,
/// but moves the entire folder instead
#[test]
fn test_move_folder_contents() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut folder2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder2.get_base_mut().metadata.name = "folder2".to_string();
    folder2.get_base_mut().file.modified = true;

    let mut scene = folder2.create_child_at_end(SCENE).unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let folder2_id = folder2.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(folder2);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(!project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder2/000-scene1.md").exists());

    // Do the move (folder2 (which contains scene) into folder1)
    SCHEMA
        .move_child(&folder2_id, &text_id, &folder1_id, 0, &project.objects)
        .unwrap();

    // Verify that the move happened on disk:
    // 1. old folder isn't there
    assert!(!project_path.join("text/001-folder2").exists());
    // 2. folder got moved
    assert!(
        project_path
            .join("text/000-folder1/000-folder2/metadata.toml")
            .exists()
    );
    assert!(project_path.join("text/000-folder1/000-folder2").exists());
    // 3. scene got moved too
    assert!(
        project_path
            .join("text/000-folder1/000-folder2/000-scene1.md")
            .exists()
    );
    // 4. nothing happened to the folder2 metadata.toml
    assert!(project_path.join("text/000-folder1/metadata.toml").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    // Folder 1 contains folder2's ID
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &folder2_id
    );

    // Folder 2 has 1 child
    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    // Folder 2 contains scene's ID
    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene_id
    );
}

/// Move an object within a folder backwards (the easy case)
#[test]
fn test_move_within_folder_backwards() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder.get_base_mut().metadata.name = "folder1".to_string();
    folder.get_base_mut().file.modified = true;

    let mut scene = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder_id = folder.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/").exists());
    assert!(project_path.join("text/001-scene1.md").exists());

    // Do the move, easy case first: moving scene1 backwards
    SCHEMA
        .move_child(&scene_id, &text_id, &text_id, 0, &project.objects)
        .unwrap();

    // Verify that the move happened on disk:
    assert!(project_path.join("text/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder1/").exists());
    assert!(!project_path.join("text/000-folder1/").exists());
    assert!(!project_path.join("text/001-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&scene_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );

    // Check that the values are properly ordered within the children
    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap()
            .to_owned(),
        folder_id
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned(),
        scene_id
    );
}

/// Move an object within a folder forwards (the hard case)
#[test]
fn test_move_within_folder_forwards() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder.get_base_mut().metadata.name = "folder1".to_string();
    folder.get_base_mut().file.modified = true;

    let mut scene = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder_id = folder.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/").exists());
    assert!(project_path.join("text/001-scene1.md").exists());

    // Do the move
    SCHEMA
        .move_child(&folder_id, &text_id, &text_id, 1, &project.objects)
        .unwrap();

    // Verify that the move happened on disk:
    assert!(project_path.join("text/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder1/").exists());
    assert!(!project_path.join("text/000-folder1/").exists());
    assert!(!project_path.join("text/001-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&scene_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_eq!(
        project
            .objects
            .get(&folder_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );

    // Check that the values are properly ordered within the children
    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap()
            .to_owned(),
        folder_id
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned(),
        scene_id
    );
}

/// Move an object from the middle of a folder to another folder and back
///
/// Starting layout:
///
/// ```
/// text
/// ├── 000-folder1
/// │   ├── 000-a.md
/// │   ├── 001-b.md
/// │   ├── 002-c.md
/// │   └── metadata.toml
/// ├── 001-folder2
/// │   └── metadata.toml
/// └── metadata.toml
/// ```
///
/// `b` moves to folder 2, leaving `000-a.md` and `001-c.md`,
/// then `b` moves to index 0 of folder 1, leaving `000-b.md`, `001-a.md` and `002-c.md`
///
/// Currently the most comprehensive test (at checking it's assumptions in multiple places),
/// probably because it was written last
#[test]
fn test_move_between_folder_contents() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut folder2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder2.get_base_mut().metadata.name = "folder2".to_string();
    folder2.get_base_mut().file.modified = true;

    let mut scene_a = folder1.create_child_at_end(SCENE).unwrap();
    scene_a.get_base_mut().metadata.name = "a".to_string();
    scene_a.get_base_mut().file.modified = true;

    let mut scene_b = folder1.create_child_at_end(SCENE).unwrap();
    scene_b.get_base_mut().metadata.name = "b".to_string();
    scene_b.get_base_mut().file.modified = true;

    let mut scene_c = folder1.create_child_at_end(SCENE).unwrap();
    scene_c.get_base_mut().metadata.name = "c".to_string();
    scene_c.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let folder2_id = folder2.get_base().metadata.id.clone();
    let scene_a_id = scene_a.get_base().metadata.id.clone();
    let scene_b_id = scene_b.get_base().metadata.id.clone();
    let scene_c_id = scene_c.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(folder2);
    project.add_object(scene_a);
    project.add_object(scene_b);
    project.add_object(scene_c);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/000-a.md").exists());
    assert!(project_path.join("text/000-folder1/001-b.md").exists());
    assert!(project_path.join("text/000-folder1/002-c.md").exists());

    // Move b into folder 2
    SCHEMA
        .move_child(&scene_b_id, &folder1_id, &folder2_id, 0, &project.objects)
        .unwrap();

    // Ensure the file got moved
    assert!(project_path.join("text/001-folder2/000-b.md").exists());
    assert!(!project_path.join("text/000-folder1/001-b.md").exists());

    // Ensure indexing is correct in the old folder
    assert!(project_path.join("text/000-folder1/000-a.md").exists());
    assert!(project_path.join("text/000-folder1/001-c.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        2
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        1
    );

    // children are in the correct spots in the folder
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene_a_id
    );

    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap(),
        &scene_c_id
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene_b_id
    );

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&scene_a_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_eq!(
        project
            .objects
            .get(&scene_c_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&scene_b_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );

    assert!(
        project
            .objects
            .get(&scene_b_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/001-folder2/000-b.md")
    );

    assert!(
        project
            .objects
            .get(&scene_a_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/000-folder1/000-a.md")
    );

    assert!(
        project
            .objects
            .get(&scene_c_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/000-folder1/001-c.md")
    );

    // Now, move b back into the start of folder 1
    SCHEMA
        .move_child(&scene_b_id, &folder2_id, &folder1_id, 0, &project.objects)
        .unwrap();

    // Ensure indexing is correct in the new folder
    assert!(!project_path.join("text/001-folder2/000-b.md").exists());
    assert!(project_path.join("text/000-folder1/000-b.md").exists());
    assert!(project_path.join("text/000-folder1/001-a.md").exists());
    assert!(project_path.join("text/000-folder1/002-c.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        3
    );

    assert_eq!(
        project
            .objects
            .get(&folder2_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    // children are in the correct spots in the folder
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene_b_id
    );

    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap(),
        &scene_a_id
    );

    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(2)
            .unwrap(),
        &scene_c_id
    );

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&scene_b_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_eq!(
        project
            .objects
            .get(&scene_a_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&scene_c_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(2)
    );

    assert!(
        project
            .objects
            .get(&scene_b_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/000-folder1/000-b.md")
    );

    assert!(
        project
            .objects
            .get(&scene_a_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/000-folder1/001-a.md")
    );

    assert!(
        project
            .objects
            .get(&scene_c_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("text/000-folder1/002-c.md")
    );
}

/// Move an object to its parent
#[test]
fn test_move_to_parent() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene = folder1.create_child_at_end(SCENE).unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(!project_path.join("text/001-scene1.md").exists());

    // Do the move (folder2 (which contains scene) into folder1)
    SCHEMA
        .move_child(&scene_id, &folder1_id, &text_id, 1, &project.objects)
        .unwrap();

    // Verify that the move happened on disk:
    assert!(project_path.join("text/000-folder1/metadata.toml").exists());
    assert!(!project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(project_path.join("text/001-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    // Text contains Folder 1
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &folder1_id
    );

    // Text has 2 children
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        2
    );

    // Text contains scene's ID
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap(),
        &scene_id
    );
}

/// Move an object to its parent, where it currently is
#[test]
fn test_move_to_parent_current_position() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene = folder1.create_child_at_end(SCENE).unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/000-scene1.md").exists());
    assert!(!project_path.join("text/001-scene1.md").exists());

    // Do the move (folder2 (which contains scene) into folder1)
    SCHEMA
        .move_child(&scene_id, &folder1_id, &text_id, 0, &project.objects)
        .unwrap();

    // Verify that the move happened on disk:
    assert!(project_path.join("text/000-scene1.md").exists());
    assert!(project_path.join("text/001-folder1/metadata.toml").exists());
    assert!(!project_path.join("text/001-folder1/000-scene1.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&folder1_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        0
    );

    // Text children contains scene
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene_id
    );

    // Text contains Folder 1
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap(),
        &folder1_id
    );

    // Text has 2 children
    assert_eq!(
        project
            .objects
            .get(&text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        2
    );
}

/// Move something where it already is (should be no-op)
#[test]
fn test_move_to_self() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_id = project.text_id.clone();

    let mut folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder.get_base_mut().metadata.name = "folder1".to_string();
    folder.get_base_mut().file.modified = true;

    let mut scene = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene.get_base_mut().metadata.name = "scene1".to_string();
    scene.get_base_mut().file.modified = true;

    let folder_id = folder.get_base().metadata.id.clone();
    let scene_id = scene.get_base().metadata.id.clone();

    project.add_object(folder);
    project.add_object(scene);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-folder1/").exists());
    assert!(project_path.join("text/001-scene1.md").exists());

    let scene_original_modtime = project
        .objects
        .get(&scene_id)
        .unwrap()
        .borrow()
        .get_base()
        .file
        .modtime
        .unwrap();

    let folder_original_modtime = project
        .objects
        .get(&folder_id)
        .unwrap()
        .borrow()
        .get_base()
        .file
        .modtime
        .unwrap();

    // Do the move
    SCHEMA
        .move_child(&folder_id, &text_id, &text_id, 0, &project.objects)
        .unwrap();

    // Verify that nothing happened on disk:
    assert!(project_path.join("text/000-folder1/").exists());
    assert!(project_path.join("text/001-scene1.md").exists());
    assert!(!project_path.join("text/000-scene1.md").exists());
    assert!(!project_path.join("text/001-folder1/").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&scene_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&folder_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );

    // Check that the values are properly ordered within the children
    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned(),
        folder_id
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .get(1)
            .unwrap()
            .to_owned(),
        scene_id
    );

    let scene_new_modtime = project
        .objects
        .get(&scene_id)
        .unwrap()
        .borrow()
        .get_base()
        .file
        .modtime
        .unwrap();

    let folder_new_modtime = project
        .objects
        .get(&folder_id)
        .unwrap()
        .borrow()
        .get_base()
        .file
        .modtime
        .unwrap();

    assert_eq!(scene_original_modtime, scene_new_modtime);
    assert_eq!(folder_original_modtime, folder_new_modtime);
}

/// Try to move a folder into one of it's (distant) children, verify that it does not allow it
#[test]
fn test_move_to_child() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut top_level_folder = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    let top_level_folder_id = top_level_folder.get_base().metadata.id.clone();
    top_level_folder.get_base_mut().metadata.name = String::from("top");
    top_level_folder.get_base_mut().file.modified = true;

    let mut mid_level_folder = top_level_folder.create_child_at_end(FOLDER).unwrap();
    let mid_level_folder_id = mid_level_folder.get_base().metadata.id.clone();
    mid_level_folder.get_base_mut().metadata.name = String::from("mid");
    mid_level_folder.get_base_mut().file.modified = true;

    let mut child_folder = mid_level_folder.create_child_at_end(SCENE).unwrap();
    let child_folder_id = child_folder.get_base().metadata.id.clone();
    child_folder.get_base_mut().metadata.name = String::from("child");
    child_folder.get_base_mut().file.modified = true;

    assert!(child_folder.get_file().exists());
    assert_eq!(child_folder.get_base().index, Some(0));

    project.add_object(top_level_folder);
    project.add_object(mid_level_folder);
    project.add_object(child_folder);

    project.save().unwrap();

    // Try to move into a folder it directly contains:
    let immediate_move = SCHEMA.move_child(
        &top_level_folder_id,
        &project.text_id,
        &mid_level_folder_id,
        1,
        &project.objects,
    );

    assert!(immediate_move.err().unwrap().to_string().contains(&format!(
        "attempted to move {} into itself",
        &top_level_folder_id
    )));

    // Try to move into a folder contained within a child:
    let child_move = SCHEMA.move_child(
        &top_level_folder_id,
        &project.text_id,
        &child_folder_id,
        1,
        &project.objects,
    );

    assert!(child_move.err().unwrap().to_string().contains(&format!(
        "attempted to move {} into itself",
        &top_level_folder_id
    )));

    // Make sure nothing moved on disk:
    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned()
            .as_str(),
        top_level_folder_id.as_str()
    );

    assert_eq!(
        project
            .objects
            .get(&top_level_folder_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned()
            .as_str(),
        mid_level_folder_id.as_str()
    );

    assert_eq!(
        project
            .objects
            .get(&mid_level_folder_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap()
            .to_owned()
            .as_str(),
        child_folder_id.as_str()
    );

    assert!(
        project
            .objects
            .get(&child_folder_id)
            .unwrap()
            .borrow()
            .get_path()
            .ends_with("000-top/000-mid/000-child.md")
    );
    assert!(
        project
            .objects
            .get(&child_folder_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );
}

#[test]
fn test_move_no_clobber() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut scene1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene1.get_base_mut().metadata.name = "a".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene2.get_base_mut().metadata.name = "a".to_string();
    scene2.get_base_mut().file.modified = true;

    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    let project_path = project.get_path();

    // Check before the move
    assert!(project_path.join("text/000-a.md").exists());
    assert!(project_path.join("text/001-a.md").exists());

    // Move b into folder 2
    SCHEMA
        .move_child(
            &scene1_id,
            &project.text_id,
            &project.text_id,
            1,
            &project.objects,
        )
        .unwrap();

    assert!(project_path.join("text/000-a.md").exists());
    assert!(project_path.join("text/001-a.md").exists());

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .len(),
        2
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene2_id
    );

    assert_eq!(
        project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_base()
            .children
            .first()
            .unwrap(),
        &scene2_id
    );

    // Make sure the file objects moved the children appropriately
    assert_eq!(
        project
            .objects
            .get(&scene1_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
}

/// Make sure places can nest
#[test]
fn test_place_nesting() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut text = SCHEMA
        .create_top_level_folder(base_dir.path().to_path_buf(), "Text")
        .unwrap();

    let mut place1 = text.create_child_at_end(PLACE).unwrap();

    let place2 = place1.create_child_at_end(PLACE).unwrap();

    assert!(place2.get_file().exists());
    assert_eq!(place1.get_base().index, Some(0));
    assert_eq!(place2.get_base().index, Some(0));
}

#[test]
fn test_place_loading() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let place_file_text = r#"id = "1"
file_type = "place""#;
    let worldbuilding_file_text = r#"id = "2"
file_type = "worldbuilding""#;

    // open and immediately drop the project (just creating the files)
    Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    std::fs::create_dir(Path::join(
        base_dir.path(),
        "test_project/worldbuilding/000-place1/",
    ))
    .unwrap();

    write_with_temp_file(
        &Path::join(
            base_dir.path(),
            "test_project/worldbuilding/000-place1/metadata.toml",
        ),
        place_file_text.as_bytes(),
    )
    .unwrap();

    std::fs::create_dir(Path::join(
        base_dir.path(),
        "test_project/worldbuilding/001-place2/",
    ))
    .unwrap();

    write_with_temp_file(
        &Path::join(
            base_dir.path(),
            "test_project/worldbuilding/001-place2/metadata.toml",
        ),
        worldbuilding_file_text.as_bytes(),
    )
    .unwrap();

    let mut project = Project::load(base_dir.path().join("test_project")).unwrap();
    project.save().unwrap();

    let place = project.objects.get(&file_id("1")).unwrap();
    let place = place.borrow();

    assert_eq!(place.get_type(), PLACE);
    assert!(
        read_to_string(place.get_file())
            .unwrap()
            .contains(r#"notes = """#)
    );

    let place2 = project.objects.get(&file_id("2")).unwrap();
    let place2 = place2.borrow();

    assert_eq!(place2.get_type(), PLACE);
    assert!(
        read_to_string(place2.get_file())
            .unwrap()
            .contains(r#"notes = """#)
    );
}

#[test]
fn test_tracker_creation_basic() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let scene_text = "123456";

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    assert_eq!(project.objects.len(), 3);

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/scene.md"),
        scene_text.as_bytes(),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 4);
}

/// Create a new folder and something in it, ensure that it all gets read in
#[test]
fn test_tracker_creation_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let scene_text = "123456";

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    assert_eq!(project.objects.len(), 3);

    let folder1_path = base_dir.path().join("test_project/text/folder1");

    create_dir(&folder1_path).unwrap();

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/folder1/scene.md"),
        scene_text.as_bytes(),
    )
    .unwrap();

    save_and_process_updates(&mut project);

    let folder1_path_final = base_dir.path().join("test_project/text/000-folder1");

    assert_eq!(project.objects.len(), 5);
    // There should be the metadata file and the scene file
    assert_eq!(std::fs::read_dir(&folder1_path_final).unwrap().count(), 2);
}

/// Ensure that a place gets read as one single object
#[test]
fn test_tracker_creation_place() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let place_file_text = r#"id = "1"
file_type = "place""#;

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    assert_eq!(project.objects.len(), 3);

    create_dir(Path::join(
        base_dir.path(),
        "test_project/worldbuilding/000-place1/",
    ))
    .unwrap();

    write_with_temp_file(
        &Path::join(
            base_dir.path(),
            "test_project/worldbuilding/000-place1/metadata.toml",
        ),
        place_file_text.as_bytes(),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 4);
    assert!(project.objects.contains_key(&file_id("1")));
}

/// First, create a file in another folder, then move it in
#[test]
fn test_tracker_creation_by_movement() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let other_dir = tempfile::TempDir::new().unwrap();

    let scene_text = r#"id = "1"
++++++++
123456"#;

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    write_with_temp_file(&other_dir.path().join("scene.md"), scene_text.as_bytes()).unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 3);

    std::fs::rename(
        other_dir.path().join("scene.md"),
        base_dir.path().join("test_project/text/scene.md"),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 4);
    assert!(project.objects.contains_key(&file_id("1")));
}

/// First, create a folder in another place, then move it in
#[test]
fn test_tracker_creation_by_movement_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();
    let other_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    create_dir(Path::join(other_dir.path(), "000-folder1")).unwrap();

    let scene_text = r#"id = "1"
++++++++
123456"#;

    write_with_temp_file(
        &other_dir.path().join("000-folder1/scene.md"),
        scene_text.as_bytes(),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 3);

    std::fs::rename(
        other_dir.path().join("000-folder1"),
        base_dir.path().join("test_project/text/000-folder1"),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 5);
    assert!(project.objects.contains_key(&file_id("1")));
}

#[test]
fn test_tracker_delete_file() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));

    assert_eq!(project.objects.len(), 6);

    let scene1_path = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();

    // Delete the file
    std::fs::remove_file(&scene1_path).unwrap();

    assert!(!scene1_path.exists());
    assert!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );

    process_updates(&mut project);

    assert!(!project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene2_id));

    assert_eq!(project.objects.len(), 5);
    assert!(!scene1_path.exists());
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert!(scene2_path_new.exists());
    assert_ne!(scene2_path_new, scene2_path_orig);

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    assert!(!project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene2_id));

    assert_eq!(project.objects.len(), 5);
    assert!(scene2_path_new.exists());
    assert_ne!(scene2_path_new, scene2_path_orig);
}

#[test]
fn test_tracker_delete_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));

    assert_eq!(project.objects.len(), 6);

    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let scene1_path = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path = project.objects.get(&scene2_id).unwrap().borrow().get_path();

    assert_eq!(
        std::fs::read_dir(base_dir.path().join("test_project/text/"))
            .unwrap()
            .count(),
        2
    );

    // Delete the file
    std::fs::remove_dir_all(&folder1_path).unwrap();

    assert!(!scene1_path.exists());
    assert!(!scene2_path.exists());
    assert!(!folder1_path.exists());

    process_updates(&mut project);

    assert!(!project.objects.contains_key(&scene1_id));
    assert!(!project.objects.contains_key(&scene2_id));
    assert!(!project.objects.contains_key(&folder1_id));

    assert_eq!(project.objects.len(), 3);
    assert!(!scene1_path.exists());
    assert!(!scene2_path.exists());
    assert!(!folder1_path.exists());

    assert_eq!(
        std::fs::read_dir(base_dir.path().join("test_project/text/"))
            .unwrap()
            .count(),
        1
    );
    // ensure that a save doesn't mess with things
    project.save().unwrap();

    assert!(!project.objects.contains_key(&scene1_id));
    assert!(!project.objects.contains_key(&scene2_id));
    assert!(!project.objects.contains_key(&folder1_id));

    assert_eq!(project.objects.len(), 3);

    assert!(!scene1_path.exists());
    assert!(!scene2_path.exists());
    assert!(!folder1_path.exists());

    assert_eq!(
        std::fs::read_dir(base_dir.path().join("test_project/text/"))
            .unwrap()
            .count(),
        1
    );
}

/// Rename a file on disk, the tracker should still process it
#[test]
fn test_tracker_rename_file() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 6);
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    let scene1_path_new = folder1_path.join("000-alt_name_scene1.md");

    // Actual start of the testing
    std::fs::rename(&scene1_path_orig, &scene1_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene1_path_orig.exists());
    assert!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should still exist and shouldn't have moved
    assert!(scene2_path_orig.exists());
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_eq!(scene2_path_new, scene2_path_orig);

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    // check 5: check that the file is currently at the new path instead of the
    // old path (based on the name). This is being tested to encode the behavior
    // (and be aware if it changes), but I don't think I'm particularly attached
    // to the behavior here either way
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_actual);

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    // After the save, check that we can safely rename the file again
    {
        let mut scene1 = project.objects.get(&scene1_id).unwrap().borrow_mut();
        scene1.get_base_mut().metadata.name = String::from("scene1 new name");
        scene1.get_base_mut().file.modified = true;
    }

    project.save().unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_final = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_final.exists());
    assert!(scene1_path_final.ends_with("000-scene1_new_name.md"));

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);
}

/// Rename a file on disk, the tracker should still process it
#[test]
fn test_tracker_rename_folder() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path_orig = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 6);
    assert_eq!(std::fs::read_dir(&folder1_path_orig).unwrap().count(), 3);

    let folder1_path_new = text_path.join("000-alt_name_folder1");

    // Actual start of the testing
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!folder1_path_orig.exists());

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scenes should have moved
    assert!(!scene1_path_orig.exists());
    assert!(!scene2_path_orig.exists());
    let scene1_path_new = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert_ne!(scene1_path_new, scene1_path_orig);

    // check 3: the scene should still exist on disk
    let folder1_path_actual = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    assert!(folder1_path_actual.exists());

    // check 4: there should be the same number of files in text and the folder
    assert_eq!(std::fs::read_dir(&folder1_path_actual).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // check 5: check that the file is currently at the new path instead of the
    // old path (based on the name). This is being tested to encode the behavior
    // (and be aware if it changes), but I don't think I'm particularly attached
    // to the behavior here either way
    assert!(!folder1_path_orig.exists());
    assert!(folder1_path_new.exists());
    assert_ne!(folder1_path_actual, folder1_path_orig);
    assert_eq!(folder1_path_new, folder1_path_actual);

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path_actual).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // After the save, check that we can safely rename the folder again
    {
        let mut scene1 = project.objects.get(&folder1_id).unwrap().borrow_mut();
        scene1.get_base_mut().metadata.name = String::from("folder1 new name");
        scene1.get_base_mut().file.modified = true;
    }

    project.save().unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_final = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_final.exists());
    assert!(scene1_path_final.ends_with("000-scene1.md"));

    let folder1_path_final = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    assert!(folder1_path_final.exists());
    assert!(folder1_path_final.ends_with("000-folder1_new_name"));

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path_final).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // Make sure that file deletion still works (by deleting scene1)
    <dyn FileObject>::remove_child(&scene1_id, &folder1_id, &mut project.objects).unwrap();

    project.save().unwrap();

    process_updates(&mut project);

    // Checks for after deletion:
    assert_eq!(project.objects.len(), 5);
    assert!(folder1_path_final.exists());
    assert_eq!(std::fs::read_dir(&folder1_path_final).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    assert!(!scene1_path_final.exists());

    let scene2_path_final = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert!(scene2_path_final.exists());
    assert!(scene2_path_final.ends_with("000-scene2.md"));
}

/// Move a file on disk and ensure the tracker processes it
#[test]
fn test_tracker_move_file() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 6);
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    let scene1_path_new = text_path.join("001-scene1.md");

    // Actual start of the testing
    std::fs::rename(&scene1_path_orig, &scene1_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene1_path_orig.exists());
    assert!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be one less file in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_actual);

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    process_updates(&mut project);

    // This seems more fragile on a save, recheck everything
    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be one less file in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_actual);

    assert_eq!(
        project
            .objects
            .get(&scene1_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    // Finally, check that we can safely rename the file again
    {
        let mut scene1 = project.objects.get(&scene1_id).unwrap().borrow_mut();
        scene1.get_base_mut().metadata.name = String::from("scene1 new name");
        scene1.get_base_mut().file.modified = true;
    }

    project.save().unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_final = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_final.exists());
    assert!(scene1_path_final.ends_with("001-scene1_new_name.md"));

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);
}

/// Move a folder on disk and ensure the tracker processes it
#[test]
fn test_tracker_move_folder() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut folder2 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder2.get_base_mut().metadata.name = "folder2".to_string();
    folder2.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let folder2_id = folder2.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(folder2);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path_orig = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let folder2_path_orig = project
        .objects
        .get(&folder2_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&folder2_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 7);
    assert_eq!(std::fs::read_dir(&folder1_path_orig).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);

    let folder1_path_new = folder2_path_orig.join("000-folder1");

    // Actual start of the testing, move the folder
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene1_path_orig.exists());
    assert!(!folder1_path_orig.exists());

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&folder2_id));

    assert_eq!(project.objects.len(), 7);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_new = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_new.exists());
    let folder1_path_actual = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();

    // check 4: there should be the same files in folder1
    assert_eq!(std::fs::read_dir(&folder1_path_actual).unwrap().count(), 3);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_new, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_new);

    assert!(!folder1_path_orig.exists());
    assert!(folder1_path_actual.exists());
    assert_ne!(folder1_path_actual, folder1_path_orig);
    // folder2 path will change so we can't compare with folder1_path_new
    assert!(folder1_path_actual.ends_with("text/000-folder2/000-folder1"));

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    process_updates(&mut project);

    // This seems more fragile on a save, recheck everything
    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&folder2_id));

    assert_eq!(project.objects.len(), 7);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_new = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_new.exists());
    let folder1_path_actual = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();

    // check 4: there should be the same files in folder1
    assert_eq!(std::fs::read_dir(&folder1_path_actual).unwrap().count(), 3);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_new, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_new);

    assert!(!folder1_path_orig.exists());
    assert!(folder1_path_actual.exists());
    assert_ne!(folder1_path_actual, folder1_path_orig);
    // folder2 path will change so we can't compare with folder1_path_new
    assert!(folder1_path_actual.ends_with("text/000-folder2/000-folder1"));
}

/// Move a file on disk to reindex it and ensure the tracker processes it
#[test]
fn test_tracker_move_file_reindex() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 6);
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    let scene1_path_new = folder1_path.join("005-scene1.md");

    // Actual start of the testing
    std::fs::rename(&scene1_path_orig, &scene1_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene1_path_orig.exists());
    assert!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // check 5: check that the file is where we expect
    assert!(!scene1_path_orig.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_ne!(scene1_path_actual, scene1_path_new);
    assert!(scene1_path_actual.ends_with("text/000-folder1/001-scene1.md"));
    assert!(scene2_path_new.ends_with("text/000-folder1/000-scene2.md"));

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    process_updates(&mut project);

    // This seems more fragile on a save, recheck everything
    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // check 5: check that the file is where we expect
    assert!(!scene1_path_orig.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_ne!(scene1_path_actual, scene1_path_new);
    assert!(scene1_path_actual.ends_with("text/000-folder1/001-scene1.md"));
    assert!(scene2_path_new.ends_with("text/000-folder1/000-scene2.md"));

    // Finally, check that we can safely rename the file again
    {
        let mut scene1 = project.objects.get(&scene1_id).unwrap().borrow_mut();
        scene1.get_base_mut().metadata.name = String::from("scene1 new name");
        scene1.get_base_mut().file.modified = true;
    }

    project.save().unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_final = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_final.exists());
    assert!(scene1_path_final.ends_with("000-folder1/001-scene1_new_name.md"));

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
}

/// Test that the tracker updates files in place
#[test]
fn test_tracker_modification() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let scene_text = r#"id = "1"
++++++++
123456"#;

    let scene1_path = base_dir.path().join("test_project/text/000-scene1.md");

    write_with_temp_file(&scene1_path, scene_text.as_bytes()).unwrap();

    process_updates(&mut project);

    {
        assert_eq!(project.objects.len(), 4);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456");
    }

    let new_scene_text = r#"id = "1"
++++++++
asdfjkl123"#;

    std::fs::write(scene1_path, new_scene_text).unwrap();

    process_updates(&mut project);

    {
        // Ensure that the file object still exists (and we don't have duplicates)
        assert_eq!(project.objects.len(), 4);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "asdfjkl123");
    }
}

/// Move a file on disk by copying and deleting the old file, should trigger duplicate detection
/// This is almost the same as `test_tracker_move_file` but it emits different file
/// events (which are harder to keep track of)
#[test]
fn test_tracker_move_file_copy_delete() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert_eq!(project.objects.len(), 6);
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);

    let scene1_path_new = text_path.join("001-scene1.md");

    // Actual start of the testing
    std::fs::copy(&scene1_path_orig, &scene1_path_new).unwrap();
    std::fs::remove_file(&scene1_path_orig).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene1_path_orig.exists());
    assert!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_path()
            .exists()
    );

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be one less file in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_actual);

    // ensure that a save doesn't mess with things
    project.save().unwrap();

    process_updates(&mut project);

    // This seems more fragile on a save, recheck everything
    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));

    assert_eq!(project.objects.len(), 6);

    // check 2: scene2 should have moved to index 0
    let scene2_path_new = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    assert_ne!(scene2_path_new, scene2_path_orig);
    assert!(scene2_path_new.exists());
    assert!(!scene2_path_orig.exists());

    // check 3: the scene should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());

    // check 4: there should be one less file in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);

    // check 5: check that the file is currently at the new path instead
    assert!(!scene1_path_orig.exists());
    assert!(scene1_path_new.exists());
    assert_ne!(scene1_path_actual, scene1_path_orig);
    assert_eq!(scene1_path_new, scene1_path_actual);

    // Finally, check that we can safely rename the file again
    {
        let mut scene1 = project.objects.get(&scene1_id).unwrap().borrow_mut();
        scene1.get_base_mut().metadata.name = String::from("scene1 new name");
        scene1.get_base_mut().file.modified = true;
    }

    project.save().unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 6);

    // check 3: the scene should still exist on disk
    let scene1_path_final = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    assert!(scene1_path_final.exists());
    assert!(scene1_path_final.ends_with("001-scene1_new_name.md"));

    // check 4: there should be the same number of files in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 2);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);
}

/// test movement and file contents being updated between tracker updates
#[test]
fn test_tracker_move_modification() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let scene_text = r#"id = "1"
++++++++
123456"#;

    let scene1_path = base_dir.path().join("test_project/text/000-scene1.md");

    std::fs::create_dir(base_dir.path().join("test_project/text/001-folder1")).unwrap();

    write_with_temp_file(&scene1_path, scene_text.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    {
        assert_eq!(project.objects.len(), 5);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456");
        assert_eq!(scene1_file_object.get_base().index, Some(0));
    }

    let new_scene_text = r#"id = "1"
++++++++
asdfjkl123"#;

    let new_scene1_path = base_dir
        .path()
        .join("test_project/text/001-folder1/000-scene1.md");
    assert!(scene1_path.exists());
    std::fs::rename(&scene1_path, &new_scene1_path).unwrap();

    std::fs::write(new_scene1_path, new_scene_text).unwrap();

    process_updates(&mut project);

    {
        // Ensure that the file object still exists (and we don't have duplicates)
        assert_eq!(project.objects.len(), 5);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "asdfjkl123");

        assert_eq!(
            std::fs::read_dir(base_dir.path().join("test_project/text/000-folder1"))
                .unwrap()
                .count(),
            2
        );
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }
}

/// Move a file into the middle of a folder, ensure that its indexes are processed correctly
#[test]
fn test_tracker_move_reindex_folder() {
    // Setup file objects
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let mut folder1 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(FOLDER)
        .unwrap();
    folder1.get_base_mut().metadata.name = "folder1".to_string();
    folder1.get_base_mut().file.modified = true;

    let mut scene1 = folder1.create_child_at_end(SCENE).unwrap();
    scene1.get_base_mut().metadata.name = "scene1".to_string();
    scene1.get_base_mut().file.modified = true;

    let mut scene2 = folder1.create_child_at_end(SCENE).unwrap();
    scene2.get_base_mut().metadata.name = "scene2".to_string();
    scene2.get_base_mut().file.modified = true;

    let mut scene3 = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow_mut()
        .create_child_at_end(SCENE)
        .unwrap();
    scene3.get_base_mut().metadata.name = "scene3".to_string();
    scene3.get_base_mut().file.modified = true;

    let folder1_id = folder1.get_base().metadata.id.clone();
    let scene1_id = scene1.get_base().metadata.id.clone();
    let scene2_id = scene2.get_base().metadata.id.clone();
    let scene3_id = scene3.get_base().metadata.id.clone();

    project.add_object(folder1);
    project.add_object(scene1);
    project.add_object(scene2);
    project.add_object(scene3);
    project.save().unwrap();

    process_updates(&mut project);

    let scene1_path_orig = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_orig = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let scene3_path_orig = project.objects.get(&scene3_id).unwrap().borrow().get_path();
    let folder1_path = project
        .objects
        .get(&folder1_id)
        .unwrap()
        .borrow()
        .get_path();
    let text_path = project
        .objects
        .get(&project.text_id)
        .unwrap()
        .borrow()
        .get_path();

    // a few baseline checks about our starting env
    assert!(project.objects.contains_key(&folder1_id));
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&scene3_id));
    assert_eq!(project.objects.len(), 7);
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 3);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);

    let scene3_path_new = folder1_path.join("001-scene3.md");
    let scene2_path_new = folder1_path.join("002-scene2.md");

    // Actual start of the testing
    std::fs::rename(&scene2_path_orig, &scene2_path_new).unwrap();
    std::fs::rename(&scene3_path_orig, &scene3_path_new).unwrap();

    // mostly checking our test logic, we expect the original file to not exist
    assert!(!scene2_path_orig.exists());
    assert!(!scene3_path_orig.exists());

    process_updates(&mut project);

    // check 1: all of the files should still be in the project
    assert!(project.objects.contains_key(&scene1_id));
    assert!(project.objects.contains_key(&scene2_id));
    assert!(project.objects.contains_key(&scene3_id));
    assert!(project.objects.contains_key(&folder1_id));

    assert_eq!(project.objects.len(), 7);

    // check 2: index order should be scene1, scene3, scene2
    assert_eq!(
        project
            .objects
            .get(&scene1_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(0)
    );
    assert_eq!(
        project
            .objects
            .get(&scene3_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(1)
    );
    assert_eq!(
        project
            .objects
            .get(&scene2_id)
            .unwrap()
            .borrow()
            .get_base()
            .index,
        Some(2)
    );

    // check 3: the scenes should still exist on disk
    let scene1_path_actual = project.objects.get(&scene1_id).unwrap().borrow().get_path();
    let scene2_path_actual = project.objects.get(&scene2_id).unwrap().borrow().get_path();
    let scene3_path_actual = project.objects.get(&scene3_id).unwrap().borrow().get_path();
    assert!(scene1_path_actual.exists());
    assert!(scene2_path_actual.exists());
    assert!(scene3_path_actual.exists());

    // check 4: there should be one more file in that directory
    assert_eq!(std::fs::read_dir(&folder1_path).unwrap().count(), 4);
    assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);

    // check 5: check that the files are where we expect
    assert_eq!(scene1_path_actual, scene1_path_orig);
    assert!(scene1_path_actual.ends_with("text/000-folder1/000-scene1.md"));
    assert_ne!(scene3_path_actual, scene3_path_orig);
    assert_eq!(scene3_path_actual, scene3_path_new);
    assert!(scene3_path_new.ends_with("text/000-folder1/001-scene3.md"));
    assert_ne!(scene2_path_actual, scene2_path_orig);
    assert_eq!(scene2_path_actual, scene2_path_new);
    assert!(scene2_path_new.ends_with("text/000-folder1/002-scene2.md"));

    assert!(scene1_path_orig.exists());
    assert!(!scene2_path_orig.exists());
    assert!(!scene3_path_orig.exists());
}

/// test file contents of a subfolder are being updated after that folder moves
/// tested with a folder and four scenes:
/// scene1 - already existing, updated before the move
/// scene2 - already existing, updated after the move
/// scene3 - newly created before move
/// scene4 - newly created after move
#[test]
fn test_tracker_move_and_modify_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let folder1_path_orig = base_dir.path().join("test_project/text/000-folder1");
    std::fs::create_dir(&folder1_path_orig).unwrap();

    let scene1_path_orig = folder1_path_orig.join("000-scene1.md");
    let scene1_text_orig = r#"id = "1"
++++++++
123456"#;

    let scene2_path_orig = folder1_path_orig.join("001-scene2.md");
    let scene2_text_orig = r#"id = "2"
++++++++

asdf"#;

    // Write scene1 and scene2 before sleeping
    write_with_temp_file(&scene1_path_orig, scene1_text_orig.as_bytes()).unwrap();
    write_with_temp_file(&scene2_path_orig, scene2_text_orig.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    // Starting assumptions
    {
        assert_eq!(project.objects.len(), 6);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456");
    }

    // Before the move, update scene1 and scene3
    let mut scene1_raw = read_to_string(&scene1_path_orig).unwrap();
    scene1_raw.push_str("updated");
    std::fs::write(&scene1_path_orig, scene1_raw).unwrap();

    let scene3_path_orig = folder1_path_orig.join("002-scene3.md");
    let scene3_text = r#"id = "3"
++++++++
scene3"#;
    std::fs::write(&scene3_path_orig, scene3_text).unwrap();

    // actually update the metadata for the moving folder
    let folder1_metadata_path = folder1_path_orig.join("metadata.toml");
    let folder1_metadata = read_to_string(&folder1_metadata_path).unwrap();
    let folder1_metadata_new = folder1_metadata.replace("folder1", "folder1_alt");
    std::fs::write(&folder1_metadata_path, folder1_metadata_new).unwrap();

    let folder1_path_new = base_dir.path().join("test_project/text/000-folder1_alt");

    // Now, rename the folder
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // And update scene2 and scene4 after the move
    let scene2_path_new = folder1_path_new.join("001-scene2.md");
    let scene2_text_new = r#"id = "2"
++++++++

asdfjkl123"#;
    std::fs::write(&scene2_path_new, scene2_text_new).unwrap();

    let scene4_path = folder1_path_new.join("003-scene4.md");
    let scene4_text = r#"id = "4"
++++++++
scene4"#;
    std::fs::write(&scene4_path, scene4_text).unwrap();

    process_updates(&mut project);

    {
        // The folder should still be populated by all four scenes
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 5);

        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));
        assert_eq!(project.objects.len(), 8);

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456\nupdated");

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_body().trim(), "asdfjkl123");

        let scene3_file_object = project.objects.get(&file_id("3")).unwrap().borrow();
        assert_eq!(scene3_file_object.get_type(), SCENE);
        assert_eq!(scene3_file_object.get_body().trim(), "scene3");

        let scene4_file_object = project.objects.get(&file_id("4")).unwrap().borrow();
        assert_eq!(scene4_file_object.get_type(), SCENE);
        assert_eq!(scene4_file_object.get_body().trim(), "scene4");

        // And a basic check around text
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }

    save_and_process_updates(&mut project);

    // Do all the checks again after the save
    {
        // The folder should still be populated by all four scenes
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 5);

        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));
        assert_eq!(project.objects.len(), 8);

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456\nupdated");

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_body().trim(), "asdfjkl123");

        let scene3_file_object = project.objects.get(&file_id("3")).unwrap().borrow();
        assert_eq!(scene3_file_object.get_type(), SCENE);
        assert_eq!(scene3_file_object.get_body().trim(), "scene3");

        let scene4_file_object = project.objects.get(&file_id("4")).unwrap().borrow();
        assert_eq!(scene4_file_object.get_type(), SCENE);
        assert_eq!(scene4_file_object.get_body().trim(), "scene4");

        // And a basic check around text
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }
}

/// Same as `test_tracker_move_and_modify_folder` but copying the folder and deleting the old one instead
/// of an actual move
#[test]
fn test_tracker_copy_move_and_modify_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let folder1_path_orig = base_dir.path().join("test_project/text/000-folder1");
    std::fs::create_dir(&folder1_path_orig).unwrap();

    let scene1_path_orig = folder1_path_orig.join("000-scene1.md");
    let scene1_text_orig = r#"id = "1"
++++++++
123456"#;

    let scene2_path_orig = folder1_path_orig.join("001-scene2.md");
    let scene2_text_orig = r#"id = "2"
++++++++

asdf"#;

    // Write scene1 and scene2 before sleeping
    write_with_temp_file(&scene1_path_orig, scene1_text_orig.as_bytes()).unwrap();
    write_with_temp_file(&scene2_path_orig, scene2_text_orig.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    // Starting assumptions
    {
        assert_eq!(project.objects.len(), 6);
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456");
    }

    // Before the move, update scene1 and scene3
    let mut scene1_raw = read_to_string(&scene1_path_orig).unwrap();
    scene1_raw.push_str("updated");
    std::fs::write(&scene1_path_orig, scene1_raw).unwrap();

    let scene3_path_orig = folder1_path_orig.join("002-scene3.md");
    let scene3_text = r#"id = "3"
++++++++
scene3"#;
    std::fs::write(&scene3_path_orig, scene3_text).unwrap();

    // actually update the metadata for the moving folder
    let folder1_metadata_path = folder1_path_orig.join("metadata.toml");
    let folder1_metadata = read_to_string(&folder1_metadata_path).unwrap();
    let folder1_metadata_new = folder1_metadata.replace("folder1", "folder1_alt");
    std::fs::write(&folder1_metadata_path, folder1_metadata_new).unwrap();

    let folder1_path_new = base_dir.path().join("test_project/text/000-folder1_alt");

    // Now, rename the folder with a copy/delete
    std::fs::create_dir(&folder1_path_new).unwrap();
    let scene1_path_new = folder1_path_new.join("000-scene1.md");
    let scene2_path_new = folder1_path_new.join("001-scene2.md");
    let scene3_path_new = folder1_path_new.join("002-scene3.md");
    let folder1_metadata_path_new = folder1_path_new.join("metadata.toml");
    std::fs::copy(&scene1_path_orig, &scene1_path_new).unwrap();
    std::fs::copy(&scene2_path_orig, &scene2_path_new).unwrap();
    std::fs::copy(&scene3_path_orig, &scene3_path_new).unwrap();
    std::fs::copy(&folder1_metadata_path, &folder1_metadata_path_new).unwrap();
    std::fs::remove_file(&scene1_path_orig).unwrap();
    std::fs::remove_file(&scene2_path_orig).unwrap();
    std::fs::remove_file(&scene3_path_orig).unwrap();
    std::fs::remove_file(&folder1_metadata_path).unwrap();
    std::fs::remove_dir(&folder1_path_orig).unwrap();

    // And update scene2 and scene4 after the move
    let scene2_text_new = r#"id = "2"
++++++++

asdfjkl123"#;
    std::fs::write(&scene2_path_new, scene2_text_new).unwrap();

    let scene4_path = folder1_path_new.join("003-scene4.md");
    let scene4_text = r#"id = "4"
++++++++
scene4"#;
    std::fs::write(&scene4_path, scene4_text).unwrap();

    process_updates(&mut project);

    {
        // The folder should still be populated by all four scenes
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 5);

        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));
        assert_eq!(project.objects.len(), 8);

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456\nupdated");

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_body().trim(), "asdfjkl123");

        let scene3_file_object = project.objects.get(&file_id("3")).unwrap().borrow();
        assert_eq!(scene3_file_object.get_type(), SCENE);
        assert_eq!(scene3_file_object.get_body().trim(), "scene3");

        let scene4_file_object = project.objects.get(&file_id("4")).unwrap().borrow();
        assert_eq!(scene4_file_object.get_type(), SCENE);
        assert_eq!(scene4_file_object.get_body().trim(), "scene4");

        // And a basic check around text
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }

    save_and_process_updates(&mut project);

    // check it again
    {
        // The folder should still be populated by all four scenes
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 5);

        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));
        assert_eq!(project.objects.len(), 8);

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456\nupdated");

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_body().trim(), "asdfjkl123");

        let scene3_file_object = project.objects.get(&file_id("3")).unwrap().borrow();
        assert_eq!(scene3_file_object.get_type(), SCENE);
        assert_eq!(scene3_file_object.get_body().trim(), "scene3");

        let scene4_file_object = project.objects.get(&file_id("4")).unwrap().borrow();
        assert_eq!(scene4_file_object.get_type(), SCENE);
        assert_eq!(scene4_file_object.get_body().trim(), "scene4");

        // And a basic check around text
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }
}

/// Test the tracker by moving a file object into a folder that has also moved
#[test]
fn test_tracker_move_into_moved_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_path = base_dir.path().join("test_project/text");

    let folder1_path_orig = text_path.join("000-folder1");
    std::fs::create_dir(&folder1_path_orig).unwrap();

    let folder2_path = text_path.join("001-folder2");
    std::fs::create_dir(&folder2_path).unwrap();

    let scene1_path_orig = folder1_path_orig.join("000-scene1.md");
    let scene1_text_orig = r#"id = "1"
++++++++
scene1"#;

    let scene2_path_orig = folder2_path.join("000-scene2.md");
    let scene2_text_orig = r#"id = "2"
++++++++
123456"#;

    let scene3_path_orig = folder2_path.join("001-scene3.md");
    let scene3_text_orig = r#"id = "3"
++++++++
scene3"#;

    let scene4_path_orig = folder2_path.join("002-scene4.md");
    let scene4_text_orig = r#"id = "4"
++++++++
scene4"#;

    // Write all scenes before sleeping
    write_with_temp_file(&scene1_path_orig, scene1_text_orig.as_bytes()).unwrap();
    write_with_temp_file(&scene2_path_orig, scene2_text_orig.as_bytes()).unwrap();
    write_with_temp_file(&scene3_path_orig, scene3_text_orig.as_bytes()).unwrap();
    write_with_temp_file(&scene4_path_orig, scene4_text_orig.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    // Starting assumptions
    {
        assert_eq!(project.objects.len(), 9); // 3 top level folders, 2 folders, 4 scenes
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "scene1");
    }

    // Before the folder move, update and move scene2
    let mut scene2_raw = read_to_string(&scene2_path_orig).unwrap();
    scene2_raw.push_str("updated");
    std::fs::write(&scene2_path_orig, scene2_raw).unwrap();

    let scene2_path_new = folder1_path_orig.join("002-scene2.md");
    std::fs::rename(&scene2_path_orig, &scene2_path_new).unwrap();

    // actually update the metadata for the moving folder
    let folder1_metadata_path = folder1_path_orig.join("metadata.toml");
    let folder1_metadata = read_to_string(&folder1_metadata_path).unwrap();
    let folder1_metadata_new = folder1_metadata.replace("folder1", "folder1 alt");
    std::fs::write(&folder1_metadata_path, folder1_metadata_new).unwrap();

    let folder1_id = get_id_from_file(&folder1_path_orig).unwrap();
    let folder2_id = get_id_from_file(&folder2_path).unwrap();

    let folder1_path_new = base_dir.path().join("test_project/text/000-folder1_alt");

    // Now, rename folder1
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // And move scene3
    let scene3_path_new = folder1_path_new.join("001-scene3.md");
    std::fs::rename(&scene3_path_orig, &scene3_path_new).unwrap();

    process_updates(&mut project);

    {
        // folder1 should have three scenes (all except scene4)
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 4);
        assert_eq!(std::fs::read_dir(&folder2_path).unwrap().count(), 2);

        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));
        assert!(project.objects.contains_key(&file_id("3")));
        assert!(project.objects.contains_key(&file_id("4")));
        assert_eq!(project.objects.len(), 9);

        let folder1_object = project.objects.get(&folder1_id).unwrap();
        assert_eq!(folder1_object.borrow().get_base().children.len(), 3);

        let folder2_object = project.objects.get(&folder2_id).unwrap();
        assert_eq!(folder2_object.borrow().get_base().children.len(), 1);

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "scene1");
        assert_eq!(scene1_file_object.get_base().index, Some(0));
        assert!(scene1_file_object.get_path().exists());

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_body().trim(), "123456\nupdated");
        assert_eq!(scene2_file_object.get_base().index, Some(2));
        assert!(scene2_file_object.get_path().exists());

        let scene3_file_object = project.objects.get(&file_id("3")).unwrap().borrow();
        assert_eq!(scene3_file_object.get_type(), SCENE);
        assert_eq!(scene3_file_object.get_body().trim(), "scene3");
        assert_eq!(scene3_file_object.get_base().index, Some(1));
        assert!(scene3_file_object.get_path().exists());

        let scene4_file_object = project.objects.get(&file_id("4")).unwrap().borrow();
        assert_eq!(scene4_file_object.get_type(), SCENE);
        assert_eq!(scene4_file_object.get_body().trim(), "scene4");
        assert_eq!(scene4_file_object.get_base().index, Some(0));
        assert!(scene4_file_object.get_path().exists());

        // And a basic check around text
        let text_path = project
            .objects
            .get(&project.text_id)
            .unwrap()
            .borrow()
            .get_path();

        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);
    }

    save_and_process_updates(&mut project);
}

/// Test the tracker by moving a file object from a folder that was first moved.
/// Should replicate the bug in https://codeberg.org/ByteOfBrie/cheese-paper/issues/149,
/// although this doesn't use two instances of the project to do it
#[test]
fn test_tracker_move_from_moved_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_path = base_dir.path().join("test_project/text");

    let folder1_path_orig = text_path.join("000-folder1");
    std::fs::create_dir(&folder1_path_orig).unwrap();

    let scene1_path_orig = folder1_path_orig.join("000-scene1.md");
    let scene1_text_orig = r#"id = "1"
++++++++
scene1"#;

    // Write all scenes before sleeping
    write_with_temp_file(&scene1_path_orig, scene1_text_orig.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    // Starting assumptions
    {
        assert_eq!(project.objects.len(), 5); // 3 top level folders, 2 folders, 4 scenes
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "scene1");
    }

    // grab folder1's id for later
    let folder1_id = get_id_from_file(&folder1_path_orig).unwrap();

    // simulate a move that cheese-paper would do for scene1 being index 0 in text
    let folder1_path_new = text_path.join("001-folder1");

    // Rename folder1
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // And move scene1
    let scene1_path_new = text_path.join("000-scene1.md");
    std::fs::rename(folder1_path_new.join("000-scene1.md"), &scene1_path_new).unwrap();

    process_updates(&mut project);

    {
        // Check that we have all of the scenes we would expect
        assert!(project.objects.contains_key(&file_id("1")));
        assert_eq!(project.objects.len(), 5);

        let folder1_object = project.objects.get(&folder1_id).unwrap();
        assert_eq!(folder1_object.borrow().get_base().children.len(), 0);
        assert_eq!(folder1_object.borrow().get_base().index, Some(1));

        // Check scene contents
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "scene1");
        assert_eq!(scene1_file_object.get_base().index, Some(0));
        assert!(scene1_file_object.get_path().exists());

        // folder1 should just have metadata, text should have metadata + 2 objects
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 3);
    }
}

/// Check that we don't keep file objects that no longer exist around
#[test]
fn test_tracker_orphaned_file_objects() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let text_path = base_dir.path().join("test_project/text");

    let folder1_path_orig = text_path.join("000-folder1");
    std::fs::create_dir(&folder1_path_orig).unwrap();

    let scene1_path_orig = folder1_path_orig.join("000-scene1.md");
    let scene1_text_orig = r#"id = "1"
++++++++
scene1"#;

    // Write all scenes before sleeping
    write_with_temp_file(&scene1_path_orig, scene1_text_orig.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    // Starting assumptions
    {
        assert_eq!(project.objects.len(), 5); // 3 top level folders, 2 folders, 4 scenes
        assert!(project.objects.contains_key(&file_id("1")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "scene1");
    }

    // grab folder1's id for later
    let folder1_id = get_id_from_file(&folder1_path_orig).unwrap();

    // Move folder1
    let folder1_path_new = text_path.join("000-folder1-alt");
    std::fs::rename(&folder1_path_orig, &folder1_path_new).unwrap();

    // And remove scene3
    std::fs::remove_file(folder1_path_new.join("000-scene1.md")).unwrap();

    process_updates(&mut project);

    {
        // Check that we have all of the scenes we would expect
        assert!(!project.objects.contains_key(&file_id("1")));
        assert_eq!(project.objects.len(), 4);

        let folder1_object = project.objects.get(&folder1_id).unwrap();
        assert_eq!(folder1_object.borrow().get_base().children.len(), 0);
        assert_eq!(folder1_object.borrow().get_base().index, Some(0));

        // folder1 should just have metadata, text should have metadata + folder
        assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(&text_path).unwrap().count(), 2);
    }
}

/// Test that files and folders have their metadata populated after creation
#[test]
fn test_tracker_metadata_population() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    let scene_text = r#"id = "1"
++++++++
123456"#;

    let scene1_path = base_dir.path().join("test_project/text/000-scene1.md");
    let folder1_path = base_dir.path().join("test_project/text/001-folder1");

    std::fs::create_dir(&folder1_path).unwrap();

    write_with_temp_file(&scene1_path, scene_text.as_bytes()).unwrap();

    save_and_process_updates(&mut project);

    assert_eq!(project.objects.len(), 5);
    assert!(project.objects.contains_key(&file_id("1")));

    // Check the file contents (first)
    let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
    assert_eq!(scene1_file_object.get_type(), SCENE);
    assert_eq!(scene1_file_object.get_body().trim(), "123456");
    assert_eq!(scene1_file_object.get_base().index, Some(0));

    let scene1_raw = read_to_string(&scene1_path).unwrap();
    let folder1_raw = read_to_string(folder1_path.join("metadata.toml")).unwrap();

    // Check that some of the keys we expect in scenes and folders have been populated
    assert!(scene1_raw.contains("id"));
    assert!(scene1_raw.contains("file_type"));
    assert!(scene1_raw.contains("name"));
    assert!(scene1_raw.contains("summary"));
    assert!(scene1_raw.contains("notes"));

    assert!(folder1_raw.contains("id"));
    assert!(folder1_raw.contains("file_type"));
    assert!(folder1_raw.contains("name"));
    assert!(folder1_raw.contains("summary"));
    assert!(folder1_raw.contains("notes"));

    // Check that the names and scene body have the expected values, using a regex to avoid whitespace
    let scene1_name_regex = regex::Regex::new(r#"name\s*=\s*"scene1""#).unwrap();
    assert!(scene1_name_regex.is_match(&scene1_raw));
    let scene1_body_regex = regex::Regex::new(r#"\+{8}\s*123456"#).unwrap();
    assert!(scene1_body_regex.is_match(&scene1_raw));

    let folder1_name_regex = regex::Regex::new(r#"name\s*=\s*"folder1""#).unwrap();
    assert!(folder1_name_regex.is_match(&folder1_raw));
}

#[test]
fn test_multiple_schemas_reload() {
    let base_dir = tempfile::TempDir::new().unwrap();

    fn project_name(i: usize) -> String {
        format!("project_number_{i}_{}", SCHEMA_LIST[i])
    }

    fn file_body_content(i: usize, file_type: FileType) -> String {
        format!(
            "This is the file content of the {file_type} file in project : {i} ({})",
            SCHEMA_LIST[i]
        )
    }

    fn file_metadata_content(i: usize, file_type: FileType) -> String {
        format!(
            "This is the metadata of the {file_type} file in project : {i} ({})",
            SCHEMA_LIST[i]
        )
    }

    let mut project_infos = Vec::new();

    for (i, &schema) in SCHEMA_LIST.iter().enumerate() {
        let project = Project::new(schema, base_dir.path().to_path_buf(), project_name(i)).unwrap();

        let mut project_file_ids = Vec::new();

        let mut root_folder = project.objects.get(&project.text_id).unwrap().borrow_mut();

        for &file_type in schema.get_all_file_types() {
            let mut file = root_folder.create_child_at_end(file_type).unwrap();

            if file_type.has_body() {
                file.load_body(file_body_content(i, file_type));
            }
            *file.get_test_field() = file_metadata_content(i, file_type);
            file.get_base_mut().file.modified = true;

            file.save(&project.objects).unwrap();

            project_file_ids.push(file.get_base().metadata.id.clone());
        }

        project_infos.push((i, schema, project.get_path(), project_file_ids));
    }

    for (i, schema, path, project_file_ids) in project_infos {
        let project = Project::load(path).unwrap();

        assert_eq!(project.base_metadata.name, project_name(i));

        for (j, &file_type) in schema.get_all_file_types().iter().enumerate() {
            let mut file_object = project
                .objects
                .get(&project_file_ids[j])
                .unwrap()
                .borrow_mut();

            assert_eq!(file_object.get_type(), file_type);
            if file_type.has_body() {
                assert_eq!(
                    file_object.get_body().trim(),
                    file_body_content(i, file_type)
                );
            }
            assert_eq!(
                *file_object.get_test_field(),
                file_metadata_content(i, file_type)
            );
        }
    }
}

// PanicPrint from https://internals.rust-lang.org/t/print-this-variable-on-panic-annotations/6150/3

pub struct PanicPrint<'a, D: Display + ?Sized + 'a> {
    msg: &'a D,
}

impl<'a, D: Display + ?Sized + 'a> PanicPrint<'a, D> {
    pub fn new(msg: &'a D) -> Self {
        PanicPrint { msg }
    }
}

impl<'a, D: Display + ?Sized + 'a> Drop for PanicPrint<'a, D> {
    fn drop(&mut self) {
        if thread::panicking() {
            eprintln!("{}", self.msg);
        }
    }
}

/// Create files out of index order, trying to cause a bug around timing
/// This test is particularly fragile and might be heavily dependent on
#[test]
fn test_tracker_reindex_timing() {
    for i in 0..20 {
        let fail_message = format!("Failed on run: {i}");
        let _fail_printer = PanicPrint::new(&fail_message);

        let base_dir = tempfile::TempDir::new().unwrap();

        let mut project = Project::new(
            SCHEMA,
            base_dir.path().to_path_buf(),
            "test project".to_string(),
        )
        .unwrap();

        let scene1_text = r#"id = "1"
++++++++
123456"#;

        let scene2_text = r#"id = "2"
++++++++
"#;

        let scene1_path = base_dir.path().join("test_project/text/000-scene1.md");
        let scene2_path = base_dir.path().join("test_project/text/001-scene2.md");

        write_with_temp_file(&scene2_path, scene2_text.as_bytes()).unwrap();

        project.process_updates();
        thread::sleep(time::Duration::from_millis(20));

        write_with_temp_file(&scene1_path, scene1_text.as_bytes()).unwrap();

        process_updates(&mut project);

        assert!(project.objects.contains_key(&file_id("1")));
        assert!(project.objects.contains_key(&file_id("2")));

        // Check the file contents (first)
        let scene1_file_object = project.objects.get(&file_id("1")).unwrap().borrow();
        assert_eq!(scene1_file_object.get_type(), SCENE);
        assert_eq!(scene1_file_object.get_body().trim(), "123456");
        assert_eq!(scene1_file_object.get_base().index, Some(0));

        let scene2_file_object = project.objects.get(&file_id("2")).unwrap().borrow();
        assert_eq!(scene2_file_object.get_type(), SCENE);
        assert_eq!(scene2_file_object.get_base().index, Some(1));
    }
}

#[test]
fn test_tracker_new_file_index() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let scene_text = r#"id = "1"
++++++++
contents1
"#;

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    assert_eq!(project.objects.len(), 3);

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/scene.md"),
        scene_text.as_bytes(),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 4);

    assert!(project.objects.contains_key(&file_id("1")));

    // Check the file contents (first)
    let scene1 = project.objects.get(&file_id("1")).unwrap().borrow();
    assert_eq!(scene1.get_type(), SCENE);
    assert_eq!(scene1.get_body().trim(), "contents1");
    assert_eq!(scene1.get_base().index, Some(0));

    assert_eq!(
        std::fs::read_dir(Path::join(base_dir.path(), "test_project/text/"))
            .unwrap()
            .count(),
        2
    );

    assert!(scene1.get_file().exists());
    assert_eq!(
        scene1.get_file(),
        Path::join(base_dir.path(), "test_project/text/000-scene.md")
    );
}

/// Create a new folder and something in it, process updates then move the folder before saving
#[test]
fn test_tracker_creation_then_move_folder() {
    let base_dir = tempfile::TempDir::new().unwrap();

    let scene_text = "123456";

    let mut project = Project::new(
        SCHEMA,
        base_dir.path().to_path_buf(),
        "test project".to_string(),
    )
    .unwrap();

    assert_eq!(project.objects.len(), 3);

    let folder1_path = base_dir.path().join("test_project/text/folder1");
    let folder1_path_moved = base_dir.path().join("test_project/text/000-folder1");
    let folder1_path_new = base_dir.path().join("test_project/text/000-folder1-alt");

    create_dir(&folder1_path).unwrap();

    write_with_temp_file(
        &Path::join(base_dir.path(), "test_project/text/folder1/scene.md"),
        scene_text.as_bytes(),
    )
    .unwrap();

    process_updates(&mut project);

    assert_eq!(project.objects.len(), 5);
    std::fs::rename(&folder1_path_moved, &folder1_path_new).unwrap();

    process_updates(&mut project);
    assert_eq!(project.objects.len(), 5);

    // There should just be the scene file, metadata hasn't been created by a save yet
    assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 1);

    // Ensure this doesn't panic
    save_and_process_updates(&mut project);

    assert_eq!(project.objects.len(), 5);

    // There should be the metadata file and the scene file
    assert_eq!(std::fs::read_dir(&folder1_path_new).unwrap().count(), 2);
}
