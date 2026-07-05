use crate::ui::prelude::*;

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use spellbook::Dictionary;
use std::{fmt::Debug, path::PathBuf};

/// Loose attempt at making other cheese-paper functions dictionary-agnostic
#[derive(Debug, Clone)]
pub struct DictionaryWrapper {
    dict: Dictionary,
    added_words: Vec<String>,
    pub lang_code: Option<String>,
}

impl DictionaryWrapper {
    pub fn new(aff_path: PathBuf, dic_path: PathBuf) -> Result<Self, CheeseError> {
        let aff_bytes = std::fs::read(&aff_path)?;
        let dic_bytes = std::fs::read(&dic_path)?;

        // we might be able to get away with assuming that the aff and dic have the same encoding,
        // but that might not *always* be true, so we'll be extra careful
        let mut aff_detector = EncodingDetector::new(Iso2022JpDetection::Deny);
        let mut dic_detector = EncodingDetector::new(Iso2022JpDetection::Deny);

        aff_detector.feed(&aff_bytes, true);
        dic_detector.feed(&dic_bytes, true);

        let aff_guessed_encoding = aff_detector.guess(None, Utf8Detection::Allow);
        let dic_guessed_encoding = dic_detector.guess(None, Utf8Detection::Allow);

        let (aff, aff_encoding_used, aff_replaced) = aff_guessed_encoding.decode(&aff_bytes);
        let (dic, dic_encoding_used, dic_replaced) = dic_guessed_encoding.decode(&dic_bytes);

        if aff_guessed_encoding != aff_encoding_used {
            log::warn!(
                "Guessed that {aff_path:?} would have encoding {aff_guessed_encoding:?}, but actually used {aff_encoding_used:?}"
            );
        }

        if dic_guessed_encoding != dic_encoding_used {
            log::warn!(
                "Guessed that {dic_path:?} would have encoding {dic_guessed_encoding:?}, but actually used {dic_encoding_used:?}"
            );
        }

        if aff_replaced {
            return Err(cheese_error!(
                "Found unexpected characters in dictionary aff file {aff_path:?} (encoding: {aff_encoding_used:?})"
            ));
        }

        if dic_replaced {
            return Err(cheese_error!(
                "Found unexpected characters in dictionary aff file {dic_path:?} (encoding: {dic_encoding_used:?})"
            ));
        }

        let dict = Dictionary::new(&aff, &dic).map_err(|err| {
            cheese_error!("Error loading dictionary from {aff_path:?} and {dic_path:?}:\n{err}")
        })?;

        let lang_code = dic_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .and_then(|file_name_str| file_name_str.split_once('_'))
            .map(|(lang_name, _)| lang_name.to_owned());

        Ok(Self {
            dict,
            added_words: Vec::new(),
            lang_code,
        })
    }

    pub fn check(&self, word: impl AsRef<str>) -> bool {
        self.dict.check(word.as_ref())
    }

    pub fn add(
        &mut self,
        word: impl AsRef<str>,
        test_word: Option<&str>,
        affix: Option<&str>,
    ) -> Result<bool, CheeseError> {
        if let Some(test_word) = test_word
            && let Some(affix) = affix
        {
            if self.check(&word) && self.check(test_word) {
                Ok(false)
            } else {
                let word_to_add = format!("{}/{affix}", word.as_ref());
                self.dict.add(&word_to_add).map_err(|err| {
                    cheese_error!("Error when adding '{}' to dictionary: {err}", word.as_ref())
                })?;

                self.added_words.push(word.as_ref().to_string());
                Ok(true)
            }
        } else if let Some(affix) = affix {
            // We have an affix but no test string, we just have to add it and hope that everything works out
            let word_to_add = format!("{}/{affix}", word.as_ref());

            self.dict.add(&word_to_add).map_err(|err| {
                cheese_error!("Error when adding '{}' to dictionary: {err}", word.as_ref())
            })?;

            self.added_words.push(word.as_ref().to_string());
            Ok(true)
        } else {
            if self.check(word.as_ref()) {
                Ok(false)
            } else {
                self.dict.add(word.as_ref()).map_err(|err| {
                    cheese_error!("Error when adding '{}' to dictionary: {err}", word.as_ref())
                })?;

                self.added_words.push(word.as_ref().to_string());
                Ok(true)
            }
        }
    }

    /// Remove a word *that has been added*. This can safely be called on arbitrary words and will not
    /// remove anything that has been added
    pub fn remove(&mut self, word: impl AsRef<str>) -> bool {
        if !self.added_words.iter().any(|e| e == word.as_ref()) {
            // we didn't add this word to the dictionary, we're done
            return false;
        }

        self.added_words
            .retain(|dict_word| word.as_ref() != dict_word);

        self.dict.remove_stem(word.as_ref())
    }

    /// Remove a word from the dictionary, even if we didn't add it ourself. Also see
    /// `Self::remove`
    pub fn full_remove(&mut self, word: impl AsRef<str>) -> bool {
        self.added_words
            .retain(|dict_word| word.as_ref() != dict_word);
        self.dict.remove_stem(word.as_ref())
    }

    pub fn suggest(&self, word: impl AsRef<str>, out: &mut Vec<String>) {
        self.dict.suggest(word.as_ref(), out);
    }
}

#[test]
fn test_dictionary_check() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(en_dict.check("test"));
    assert!(!en_dict.check("cheesepaperisagoodeditor"));
}

#[test]
fn test_dictionary_add() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(!en_dict.check("cheesepaperisagoodeditor"));
    assert!(en_dict.add("cheesepaperisagoodeditor", None, None).is_ok());
    assert!(en_dict.check("cheesepaperisagoodeditor"));
    assert!(!en_dict.check("cheesepaperisagoodeditor's"));
    assert!(
        en_dict
            .added_words
            .contains(&String::from("cheesepaperisagoodeditor"))
    );
}

#[test]
fn test_dictionary_add_affix() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(!en_dict.check("CheesePaper"));
    assert!(!en_dict.check("CheesePaper's"));
    assert!(
        en_dict
            .add("CheesePaper", Some("CheesePaper's"), Some("M"))
            .is_ok()
    );
    assert!(en_dict.check("CheesePaper"));
    assert!(en_dict.check("CheesePaper's"));
}

#[test]
fn test_dictionary_remove() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(!en_dict.check("cheesepaperisagoodeditor"));

    // add the word and verify test assumptions
    assert!(en_dict.add("cheesepaperisagoodeditor", None, None).is_ok());
    assert!(en_dict.check("cheesepaperisagoodeditor"));

    // remove it and check that it's no longer counted
    assert!(en_dict.remove("cheesepaperisagoodeditor"));
    assert!(!en_dict.check("cheesepaperisagoodeditor"));

    // removal of a word that isn't in the dictionary should be false
    assert!(!en_dict.remove("cheesepaperisagoodeditor"));
}

#[test]
fn test_dictionary_remove_affix() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(!en_dict.check("CheesePaper"));
    assert!(!en_dict.check("CheesePaper's"));
    assert!(
        en_dict
            .add("CheesePaper", Some("CheesePaper's"), Some("M"))
            .is_ok()
    );
    assert!(en_dict.check("CheesePaper"));
    assert!(en_dict.check("CheesePaper's"));
    assert!(en_dict.full_remove("CheesePaper"));
    assert!(!en_dict.check("CheesePaper"));
    assert!(!en_dict.check("CheesePaper's"));
}

#[test]
fn test_dictionary_remove_existing() {
    let base_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(en_dict.check("cheese"));
    assert!(en_dict.add("cheese", Some("cheese's"), Some("M")).is_ok());
    assert!(en_dict.check("cheese"));
    // We shouldn't have added it here
    assert!(!en_dict.added_words.contains(&String::from("cheese")));

    assert!(en_dict.add("cheese", None, None).is_ok());
    assert!(en_dict.check("cheese"));
    // Or here
    assert!(!en_dict.added_words.contains(&String::from("cheese")));

    // Now we can try to do the normal removal process, which shouldn't do anything
    assert!(!en_dict.remove("cheese"));
    assert!(en_dict.check("cheese"));

    // If we fully remove it, that should override what we just did
    assert!(en_dict.full_remove("cheese"));
    assert!(!en_dict.check("cheese"));
}

// TODO: check for adding an affix and then removing it

#[test]
fn test_dictionary_add_remove_existing() {
    let base_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let mut en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(en_dict.check("chem"));
    assert!(!en_dict.check("chem's"));
    assert!(en_dict.add("chem", Some("chem's"), Some("M")).is_ok());
    assert!(en_dict.check("chem"));
    assert!(en_dict.check("chem's"));
    assert!(en_dict.added_words.contains(&String::from("chem")));

    assert!(en_dict.remove("chem"));
    assert!(!en_dict.check("chem"));
    assert!(!en_dict.check("chem's"));
}

#[test]
fn test_dictionary_suggest() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let en_dict = match DictionaryWrapper::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(en_dict.check("test"));
    assert!(!en_dict.check("tes"));
    let mut suggestions = Vec::new();
    en_dict.suggest("tes", &mut suggestions);
    assert!(suggestions.contains(&String::from("test")));
}
