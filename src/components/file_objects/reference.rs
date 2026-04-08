use cow_utils::CowUtils;
use icu_casemap::{CaseMapper, CaseMapperBorrowed};

use crate::components::file_objects::{FileID, FileObjectStore, FileType};

/// A reference to an object that is currently unknown (e.g., does not reference an object
/// currently loaded into the editor)
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnknownReference {
    pub name: String,
    /// The ID present in the reference, if any. Will not be generated if missing. This should
    /// possibly be an option, which would make more sense in some ways, although an empty string
    /// is not meaningfully different from an option with None
    pub id: String,
    pub file_type: Option<FileType>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub enum ObjectReference {
    Known(FileID),
    Unknown(UnknownReference),
    #[default]
    None,
}

impl ObjectReference {
    pub fn new(mut value: String, file_type: Option<FileType>) -> Self {
        if value.is_empty() {
            ObjectReference::None
        } else {
            if value.starts_with('[') && value.ends_with(']') {
                value.pop();
                value.remove(0);
            }
            match value.split_once('|').unwrap_or((value.as_str(), "")) {
                ("", "") => ObjectReference::None,
                (name, id) => ObjectReference::Unknown(UnknownReference {
                    name: name.to_string(),
                    id: id.to_string(),
                    file_type,
                }),
            }
        }
    }

    /// This should probably have a better name, there should also be a "to title" function that
    /// gets called by the outline
    pub fn to_string(&self, objects: &FileObjectStore) -> String {
        let mut output = String::new();
        output.push('[');

        match self {
            Self::Known(file_id) => {
                match objects.get(file_id) {
                    Some(referenced_object) => {
                        // Add the string name, removing invalid characters
                        output.push_str(
                            &referenced_object
                                .borrow()
                                .get_base()
                                .metadata
                                .name
                                .cow_replace('|', "")
                                .cow_replace('[', "")
                                .cow_replace(']', ""),
                        );
                    }
                    None => {
                        log::error!(
                            "Could not find file ID {file_id} for reference while attempting to save"
                        );
                        output.push_str("ERROR");
                    }
                }

                output.push('|');
                output.push_str(file_id);
            }
            Self::Unknown(unknown) => {
                // Add the string name, removing invalid characters
                output.push_str(
                    &unknown
                        .name
                        .cow_replace('|', "")
                        .cow_replace('[', "")
                        .cow_replace(']', ""),
                );

                output.push('|');
                output.push_str(&unknown.id);
            }
            Self::None => {}
        }
        output.push(']');
        output
    }

    pub fn to_display(&self, objects: &FileObjectStore) -> String {
        match self {
            Self::Known(file_id) => match objects.get(file_id) {
                Some(referenced_object) => {
                    referenced_object.borrow().get_base().metadata.name.clone()
                }
                None => {
                    log::error!(
                        "Could not find file ID {file_id} for reference while attempting to save"
                    );
                    String::from("Error finding character's name")
                }
            },
            Self::Unknown(unknown) => {
                if unknown.name.is_empty() {
                    String::from("Unknown")
                } else {
                    unknown.name.clone()
                }
            }
            Self::None => String::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum WordMatch {
    Exact,
    FullNeedleAsPrefix,
    None,
}

impl UnknownReference {
    /// Attempt to resolve this option into a FileID. This should be called in one specific place
    /// that will have to handle the actual transformation
    pub fn resolve(&self, objects: &FileObjectStore) -> Option<FileID> {
        static CASE_MAPPER: std::sync::LazyLock<CaseMapperBorrowed<'_>> =
            std::sync::LazyLock::new(CaseMapper::new);

        if self.id.is_empty() {
            // We don't have an ID, look through all of the objects

            // Create a few variables to track state
            let mut best_match_id: Option<&FileID> = None;
            let mut prefix_len = WordMatch::None;
            let mut found_multiple = false;

            // name of the object we're searching for, case folded so we can make case-insensitive
            // comparisons (see https://www.w3.org/TR/charmod-norm/#definitionCaseFolding)
            let needle_name = CASE_MAPPER.fold_string(&self.name);

            // Compare this reference to every object to see if it matches up
            for (id, object_refcell) in objects.iter() {
                let object = object_refcell.borrow();

                // If we have a known file object type and this doesn't match it (e.g., we're trying to
                // resolve a character reference and this is a scene, give up)
                if let Some(this_file_type) = self.file_type
                    && this_file_type != object.get_type()
                {
                    continue;
                }

                let object_name = CASE_MAPPER.fold_string(&object.get_base().metadata.name);

                if needle_name == object_name {
                    // exact name match
                    if prefix_len == WordMatch::Exact {
                        // we've found two exact name matches, we can't distinguish between them, give up
                        log::warn!(
                            "Found multiple exact matches of name '{object_name}' while attempting \
                             to resolve reference, giving up"
                        );
                        return None;
                    } else {
                        best_match_id = Some(id);
                        prefix_len = WordMatch::Exact;
                        found_multiple = false;
                    }
                } else if object_name.starts_with(&*needle_name) {
                    if prefix_len == WordMatch::FullNeedleAsPrefix {
                        // We could find an exact match later, keep going
                        found_multiple = true;
                    } else {
                        // some full prefix matches probably *shouldn't* be resolved automatically:
                        // https://codeberg.org/ByteOfBrie/cheese-paper/issues/119
                        prefix_len = WordMatch::FullNeedleAsPrefix;
                        best_match_id = Some(id);
                    }
                }
            }

            if found_multiple {
                if prefix_len == WordMatch::Exact {
                    log::error!(
                        "Found multiple exact matches late in the program, should be impossible"
                    )
                }
                log::debug!(
                    "Found multiple instances of name '{needle_name}' while attempting \
                     to resolve reference, giving up"
                );
                return None;
            }

            match prefix_len {
                WordMatch::Exact => best_match_id.cloned(),
                WordMatch::FullNeedleAsPrefix => best_match_id.cloned(),
                WordMatch::None => {
                    log::debug!(
                        "No prefixes found when attempting to resolve reference {needle_name}"
                    );
                    None
                }
            }
        } else {
            // We have an ID string, we're only looking for that ID
            if let Some(object_ref) = objects.get(&self.id) {
                let object = object_ref.borrow();

                if let Some(this_file_type) = self.file_type
                    && this_file_type != object.get_type()
                {
                    log::warn!(
                        "Found object with id {}, but it has type {:?}, was expecting type {:?}",
                        &self.id,
                        std::convert::Into::<FileType>::into(object.get_type()),
                        this_file_type
                    );
                    None
                } else {
                    // we either found a object of the "correct" type, or we're not looking for a
                    // reference of a specific type
                    Some(object.id().clone())
                }
            } else {
                // No IDs found, give up. We don't want to break a real reference by searching
                // if we want to be smart about this, we can add logic to ask if the user
                // wants to merge later
                None
            }
        }
    }
}

/// Find the longest common prefix of two strings, character by character, should support unicode
/// characters correctly.
#[allow(dead_code)] // We'll probably use this in fuzzy search logic, keep it for now
fn longest_common_prefix(s1: &str, s2: &str) -> String {
    let mut common_prefix = String::new();
    let mut chars1 = s1.chars();
    let mut chars2 = s2.chars();

    loop {
        match (chars1.next(), chars2.next()) {
            (Some(c1), Some(c2)) if c1 == c2 => {
                common_prefix.push(c1);
            }
            _ => {
                break; // Mismatch or one of the strings ended
            }
        }
    }
    common_prefix
}

#[test]
fn test_largest_common_prefix() {
    // I don't specifically care about the result of emojis, but emojis have multi-byte UTF-8 encodings
    // and zero-width joiners (both of which are possible in normal text depending on the language),
    // but complex emojis are easier for me to understand than languages that would actually use
    // zero width joiners in unicode. Contributions of examples from those languages are still welcome
    let s1 = "👋🏻"; // U+1F44B U+1F3FB
    let s2 = "👋🏿"; // U+1F44B U+1F3FF
    let s3 = "🤚🏻"; // U+1F91A U+1F3FB

    // simple tests, these should always pass
    assert_eq!(longest_common_prefix(s1, s1), s1);
    assert_eq!(longest_common_prefix("value", "val1"), "val");
    assert_eq!(longest_common_prefix("value", ""), "");
    assert_eq!(longest_common_prefix("1value", "2value"), "");

    // both strings start with `0xF0 0x9F` when utf-8 encoded, but shouldn't match at all
    assert_eq!(longest_common_prefix(s1, s3), "");

    // anything below this point *can* be modified if the library/function changes, just think about
    // it a little bit

    // this specific behavior isn't critical, but we should know *how* it works
    assert_eq!(longest_common_prefix(s1, s2), "👋"); // U+1F44B should match
    assert_eq!(longest_common_prefix("test👋🏻", "test👋🏿"), "test👋");

    // what happens when you start to include zero width joiners?
    // U+1F636 U+200D U+1F32B U+FE0F compared with U+1F636
    assert_eq!(longest_common_prefix("😶‍🌫️", "😶"), "😶");
    // U+1F3F3 U+FE0F U+200D U+1F308 compared with U+1F3F3 U+FE0F
    assert_eq!(longest_common_prefix("🏳️‍🌈", "🏳️"), "🏳️");
    // U+1F642 U+200D U+2194 U+FE0F compared with U+1F642 U+200D U+2195 U+FE0F
    assert_eq!(longest_common_prefix("🙂‍↔️", "🙂‍↕️"), "🙂\u{200d}");
    // U+1F3F3 U+FE0F U+200D U+1F308 compared with U+1F3F3 U+FE0F	U+200D	U+26A7 U+FE0F
    assert_eq!(longest_common_prefix("🏳️‍🌈", "🏳️‍⚧️"), "🏳️\u{200d}");
}

/// Run a few tests on case folding, mostly to verify assumptions. This should use whatever system
/// we use
#[test]
fn test_case_mapping() {
    let cm = CaseMapper::new();

    assert_eq!(cm.fold_string("val"), cm.fold_string("val"));
    assert_eq!(cm.fold_string("Val"), cm.fold_string("val"));
    assert_eq!(cm.fold_string("val"), cm.fold_string("VAL"));
    assert_eq!(cm.fold_string("Straße"), cm.fold_string("strasse"));
    // I'm blindly trusting these examples from elsewhere, I don't know enough to verify them
    assert_eq!(cm.fold_string("Привет мир"), cm.fold_string("привет мир"));
    assert_eq!(
        cm.fold_string("Γειά σου Κόσμε"),
        cm.fold_string("γειά σου κόσμε")
    );
}
