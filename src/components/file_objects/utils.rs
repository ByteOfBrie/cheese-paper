use std::io::Write;
use std::path::Path;
use tempfile::Builder;
use toml_edit::TableLike;

use crate::cheese_error;
use crate::util::CheeseError;

/// Value that splits the header of any file that contains non-metadata content
pub const HEADER_SPLIT: &str = "++++++++";

/// the maximum length of a name before we start trying to truncate it
pub const FILENAME_MAX_LENGTH: usize = 30;

/// filename of the object within a folder containing its metadata (without extension)
pub const FOLDER_METADATA_FILE_NAME: &str = "metadata.toml";

/// Generic file utilities
use regex::Regex;

/// make sure the name fits within a specified length, trying to split on word boundaries
pub fn truncate_name(name: &str, max_length: usize) -> &str {
    let name = name.trim();
    // if the name is already short enough, we're done
    if name.len() <= max_length {
        return name;
    }

    // split by word, go with increasingly fewer words
    let split_name: Vec<&str> = name.split(" ").collect();
    for number_of_words in (1..split_name.len()).rev() {
        // construct the split text into a temporary string
        let shortened = &split_name[..number_of_words].join(" ");
        if shortened.len() < max_length && !shortened.is_empty() {
            // return a slice of the actual name so it retains the original lifetime
            return &name[..shortened.len()];
        }
    }

    // we the first word is longer than `max_length`, give up on being smart
    &name[..max_length]
}

#[test]
fn test_truncate_name() {
    assert_eq!(truncate_name("Hello World", 30), "Hello World");
    assert_eq!(truncate_name("Hello World", 9), "Hello");
    assert_eq!(truncate_name("Hello World", 11), "Hello World");
    assert_eq!(truncate_name("Hello World", 5), "Hello");
    assert_eq!(truncate_name("Hello World", 4), "Hell");
    assert_eq!(truncate_name(" Hello World", 2), "He");
    assert_eq!(truncate_name("Hello World   ", 30), "Hello World");
}
/// Translates a name into something we can put on disk
pub fn process_name_for_filename(name: &str) -> String {
    // get rid of spaces in names for editing convenience
    let name = name.replace(" ", "_");
    let name = name.replace("'", "");

    // Characters that might be annoying to escape/handle sometimes, avoid including them at all
    let dangerous_character_filter = Regex::new(r#"[/\?%*:|"<>\x7F\x00-\x1F]"#).unwrap();
    dangerous_character_filter
        .replace_all(&name, "-")
        .into_owned()
}

#[test]
fn test_process_name_for_filename() {
    assert_eq!(process_name_for_filename(r"hello world"), "hello_world");
    assert_eq!(process_name_for_filename(r"possessive's"), "possessives");
    assert_eq!(process_name_for_filename(r"asdf?'?s"), "asdf--s");
}

/// Just adds an index to a name, no real logic
pub fn add_index_to_name(name: &str, index: usize) -> String {
    format!("{index:03}-{name}")
}

/// Gets the file index from a string if it exists
pub fn get_index_from_name(name: &str) -> Option<usize> {
    // This can probably be done smarter with maps but I don't see how to do it now and I'm sleepy
    match name.split_once('-') {
        Some((prefix, _suffix)) => prefix.parse().ok(),
        None => None,
    }
}

pub fn create_dir_if_missing(dest_path: &Path) -> std::io::Result<&Path> {
    let dirname = dest_path.parent().expect("Must pass a path with a parent");

    if !std::fs::exists(dirname)? {
        std::fs::create_dir(dirname)?;
    }

    Ok(dest_path)
}

/// Atomically write a file
pub fn write_with_temp_file(dest_path: &Path, contents: impl Into<String>) -> std::io::Result<()> {
    let dirname = dest_path.parent().expect("Must pass a path with a parent");
    let basename = dest_path.file_name().expect("Must write to a file");

    let mut file = Builder::new().suffix(".tmp").tempfile_in(dirname)?;

    file.write_all(contents.into().as_bytes())?;

    let mut dest_path = dirname.to_path_buf();
    dest_path.push(basename);

    file.persist(dest_path)?;

    Ok(())
}

#[test]
fn test_write_with_temp_file() -> std::io::Result<()> {
    let base_dir = tempfile::TempDir::new()?;
    let filename = std::ffi::OsString::from("file.md");
    let contents = "some file contents";

    let file_full_path = base_dir.path().join(&filename);

    assert!(!file_full_path.exists());
    assert_eq!(std::fs::read_dir(base_dir.path())?.count(), 0);

    write_with_temp_file(&file_full_path, contents)?;

    assert!(file_full_path.exists());
    assert_eq!(std::fs::read_dir(base_dir.path())?.count(), 1);

    let disk_contents = std::fs::read_to_string(&file_full_path)?;

    assert_eq!(contents, disk_contents);

    Ok(())
}

pub fn metadata_extract_u64(
    table: &dyn TableLike,
    field_name: &str,
    allow_bool: bool,
) -> Result<Option<u64>, CheeseError> {
    match table.get(field_name) {
        Some(value) => {
            if let Some(value) = value.as_integer() {
                Ok(Some(value as u64))
            } else if allow_bool && let Some(value) = value.as_bool() {
                Ok(Some(value as u64))
            } else {
                Err(cheese_error!("{field_name} was not an integer"))
            }
        }
        None => Ok(None),
    }
}

pub fn metadata_extract_string(
    table: &dyn TableLike,
    field_name: &str,
) -> Result<Option<String>, CheeseError> {
    Ok(match table.get(field_name) {
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| cheese_error!("{field_name} was not string"))?
                .to_owned(),
        ),
        None => None,
    })
}

pub fn metadata_extract_bool(
    table: &dyn TableLike,
    field_name: &str,
) -> Result<Option<bool>, CheeseError> {
    Ok(match table.get(field_name) {
        Some(value) => Some(
            value
                .as_bool()
                .ok_or_else(|| cheese_error!("{field_name} was not bool"))?,
        ),
        None => None,
    })
}

pub fn write_outline_property(property_name: &str, property: &str, export_string: &mut String) {
    if property.is_empty() {
        return;
    }

    export_string.push_str(property_name);
    export_string.push(':');

    if property.contains('\n') || property.len() > 40 {
        export_string.push_str("\n\n");

        for line in property.split('\n') {
            export_string.push_str("> ");
            export_string.push_str(line);
            export_string.push('\n');
        }

        export_string.push_str("\n\n");
    } else {
        export_string.push(' ');
        export_string.push_str(property);
        export_string.push_str("\n\n");
    }
}

/// Reads the contents of a file from disk
pub fn read_file_contents(file_to_read: &Path) -> Result<(String, Option<String>), CheeseError> {
    let extension = match file_to_read.extension() {
        Some(val) => val,
        None => return Err(cheese_error!("value was not string")),
    };

    let file_data = std::fs::read_to_string(file_to_read)?;

    let (metadata_str, file_content): (&str, Option<&str>) = if extension == "md" {
        match file_data.split_once(HEADER_SPLIT) {
            None => ("", Some(&file_data)),
            Some((start, end)) => (start, Some(end)),
        }
    } else {
        (&file_data, None)
    };

    Ok((
        metadata_str.to_owned(),
        file_content.map(|s| s.trim().to_owned()),
    ))
}
