use crate::ui::prelude::*;

use hunspell_rs::Hunspell;
use std::{fmt::Debug, path::PathBuf};

pub struct Dictionary {
    dict: Hunspell,
    dic_path: PathBuf,
    aff_path: PathBuf,
}

impl Dictionary {
    pub fn new(aff_path: PathBuf, dic_path: PathBuf) -> Result<Self, CheeseError> {
        if let Some(dic_path_str) = dic_path.to_str()
            && let Some(aff_path_str) = aff_path.to_str()
        {
            let dict = Hunspell::new(aff_path_str, dic_path_str);

            Ok(Self {
                dict,
                dic_path,
                aff_path,
            })
        } else {
            Err(cheese_error!("Could not process dictionary path"))
        }
    }

    /// Create a *fresh* clone of this dictionary. It will not keep any words that were added, nor
    /// will it
    pub fn try_clone(&self) -> Result<Self, CheeseError> {
        Self::new(self.aff_path.clone(), self.dic_path.clone())
    }

    pub fn check(&self, word: impl AsRef<str>) -> bool {
        self.dict.check(word.as_ref()) == hunspell_rs::CheckResult::FoundInDictionary
    }

    pub fn add(&mut self, word: impl AsRef<str>) -> Result<(), CheeseError> {
        if self.dict.add(word.as_ref()) {
            Ok(())
        } else {
            // We don't get any visibility into why this happened, probably won't come up much
            Err(cheese_error!(
                "Unknown hunspell error when adding '{}' to dictionary",
                word.as_ref()
            ))
        }
    }

    pub fn suggest(&self, word: impl AsRef<str>) -> Vec<String> {
        self.dict.suggest(word.as_ref())
    }
}

impl Debug for Dictionary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dictionary")
            .field("dic_path", &self.dic_path)
            .field("aff_path", &self.aff_path)
            .finish()
    }
}

#[test]
fn test_dictionary_check() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let en_dict = match Dictionary::new(
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

    let mut en_dict = match Dictionary::new(
        base_path.join("resources/spellcheck/en_US/en_US.aff"),
        base_path.join("resources/spellcheck/en_US/en_US.dic"),
    ) {
        Ok(en_dict) => en_dict,
        Err(err) => {
            panic!("Could not load dictionary: {err}");
        }
    };

    assert!(!en_dict.check("cheesepaperisagoodeditor"));
    assert!(en_dict.add("cheesepaperisagoodeditor").is_ok());
    assert!(en_dict.check("cheesepaperisagoodeditor"));
}

#[test]
fn test_dictionary_suggest() {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let en_dict = match Dictionary::new(
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
    let suggestions = en_dict.suggest("tes");
    assert!(suggestions.contains(&String::from("test")));
}
