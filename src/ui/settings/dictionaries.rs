use std::{env, path::PathBuf};

use super::SettingsData;
use crate::ui::prelude::*;

use spellbook::Dictionary;

#[derive(Debug)]
pub struct AvailableDictionary {
    dic_path: PathBuf,

    aff_path: PathBuf,

    pub name: String,
}

impl PartialEq for AvailableDictionary {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

pub const SELECTED_NONE: &str = "<None>";

impl TryFrom<PathBuf> for AvailableDictionary {
    type Error = ();

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let extension = path.extension().ok_or(())?;

        if !(extension == "dic" || extension == "aff") {
            return Err(());
        }

        let dic_path = path.with_extension("dic");
        let aff_path = path.with_extension("aff");

        if !(dic_path.try_exists().map_err(|_| ())? && aff_path.try_exists().map_err(|_| ())?) {
            return Err(());
        }

        let name = path
            .file_prefix()
            .and_then(|s| s.to_str())
            .map(|s| s.to_owned())
            .ok_or(())?;

        Ok(Self {
            dic_path,
            aff_path,
            name,
        })
    }
}

impl AvailableDictionary {
    pub fn load(&self) -> Result<Dictionary, CheeseError> {
        let dic = std::fs::read_to_string(&self.dic_path)?;
        let aff = std::fs::read_to_string(&self.aff_path)?;

        Dictionary::new(&aff, &dic).map_err(|err| {
            cheese_error!(
                "Error reading dictionary files ({:?} and {:?}):\n{err}",
                &self.dic_path,
                &self.aff_path
            )
        })
    }
}

impl SettingsData {
    pub fn load_available_dictionaries(&mut self) -> Result<(), CheeseError> {
        self.available_dict.clear();

        // For each dictionary name, we will only keep the first dictionary we load
        // The order in which search locations are in this list determines
        // which dictionary we pick in priority if there are mulitple
        let mut dict_search_paths = Vec::new();

        let mut app_dict_folder_path = self.settings_path.clone();
        app_dict_folder_path.push(PathBuf::from("dictionaries"));

        if !std::fs::exists(&app_dict_folder_path)? {
            std::fs::create_dir(&app_dict_folder_path)?;
        }

        dict_search_paths.push(app_dict_folder_path);

        if env::consts::OS == "linux" {
            dict_search_paths.push(PathBuf::from("/usr/share/hunspell/"));
        } else if env::consts::OS == "macos"
            && let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_folder) = exe_path.parent()
        {
            dict_search_paths.push(exe_folder.join("../Resources/resources/spellcheck/en_US/"));
        }

        // TODO add more system-dependant search paths

        for search_path in &dict_search_paths {
            for entry in std::fs::read_dir(search_path)? {
                let path = entry?.path();

                if let Ok(dict) = AvailableDictionary::try_from(path)
                    && !self.available_dict.contains(&dict)
                {
                    self.available_dict.push(dict);
                }
            }
        }

        Ok(())
    }
}
