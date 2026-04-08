use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
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
    let dangerous_character_filter = Regex::new(r#"[./\?%*:|"<>\x7F\x00-\x1F]"#).unwrap();
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
    let dirname = dest_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidFilename,
            "Must pass a path with a parent",
        )
    })?;

    if !std::fs::exists(dirname)? {
        std::fs::create_dir(dirname)?;
    }

    Ok(dest_path)
}

/// Atomically write a file
pub fn write_with_temp_file<P: AsRef<Path>>(
    dest_path: P,
    contents: impl Into<String>,
) -> Result<(), CheeseError> {
    let dirname = dest_path
        .as_ref()
        .parent()
        .ok_or_else(|| cheese_error!("Must pass a path with a parent: {:?}", dest_path.as_ref()))?;

    let mut file = match Builder::new().suffix(".tmp").tempfile_in(dirname) {
        Ok(tempfile) => tempfile,
        Err(err) => {
            return Err(cheese_error!(
                "Could not create tempfile for {:?}: {err}",
                dest_path.as_ref()
            ));
        }
    };

    let file_contents = contents.into();
    // We need to copy it here if we're on windows, other OSes should hopefully
    // optimize this out (since it will be unused)
    let contents_copy = file_contents.clone();

    if let Err(err) = file.write_all(file_contents.as_bytes()) {
        return Err(cheese_error!(
            "Could not write to tempfile for {:?}: {err}",
            dest_path.as_ref()
        ));
    };

    const WINDOWS_SLEEP_DURATION: Duration = Duration::from_millis(500);

    if cfg!(windows) {
        // We have to potentially retry on windows because antivirus software can
        // lock the directory. There isn't a super nice way to do this logic since
        // we consume the tempfile each time, but we can just create a new one...
        // https://github.com/Stebalien/tempfile/issues/316
        if let Err(err) = file.persist(dest_path.as_ref()) {
            let start = Instant::now();

            log::warn!(
                "Could not persist tempfile on windows: retrying. This may be due to \
                antivirus software, consider adjusting settings if this issue persists"
            );

            loop {
                let mut file = Builder::new().suffix(".tmp").tempfile_in(dirname)?;
                file.write_all(contents_copy.clone().as_bytes())?;

                if file.persist(dest_path.as_ref()).is_ok() {
                    break;
                }

                if start.elapsed() > WINDOWS_SLEEP_DURATION {
                    return Err(cheese_error!(
                        "Could not persist tempfile with path {:?}, even after \
                        {WINDOWS_SLEEP_DURATION:?} sec of retries: {err}",
                        dest_path.as_ref()
                    ));
                }

                thread::sleep(Duration::from_millis(20));
            }
        }
    } else if let Err(err) = file.persist(dest_path.as_ref()) {
        return Err(cheese_error!(
            "Could not persist tempfile with path {:?}: {err}",
            dest_path.as_ref()
        ));
    }

    Ok(())
}

#[test]
fn test_write_with_temp_file() -> Result<(), CheeseError> {
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
