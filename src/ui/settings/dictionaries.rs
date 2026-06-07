use std::{
    collections::HashSet,
    env,
    path::{Path, PathBuf},
};

use super::SettingsData;
use crate::ui::prelude::*;

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
        Dictionary::new(self.aff_path.clone(), self.dic_path.clone())
    }
}

impl SettingsData {
    pub fn load_available_dictionaries(&mut self) -> Result<(), CheeseError> {
        self.available_dict.clear();

        // For each dictionary name, we will only keep the first dictionary we load
        // The order in which search locations are in this list determines
        // which dictionary we pick in priority if there are mulitple
        let mut dict_search_paths = HashSet::new();

        let mut app_dict_folder_path = self.settings_path.clone();
        app_dict_folder_path.push(PathBuf::from("dictionaries"));

        if !std::fs::exists(&app_dict_folder_path)? {
            std::fs::create_dir_all(&app_dict_folder_path)?;
        }

        dict_search_paths.insert(app_dict_folder_path);

        if env::consts::OS == "linux" {
            let parent_search_dirs = if let Ok(env_data_dirs) = env::var("XDG_DATA_DIRS")
                && !env_data_dirs.is_empty()
            {
                env_data_dirs
            } else {
                String::from("/usr/local/share/:/usr/share/")
            };

            for parent_search_dir in parent_search_dirs.split(":") {
                let dict_dir = Path::new(parent_search_dir).join("hunspell");
                if dict_dir.exists() {
                    dict_search_paths.insert(dict_dir);
                }
            }

            let data_home = env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| env::home_dir().unwrap().join(".local/share"));
            let home_dict_dir = data_home.join("hunspell");
            if home_dict_dir.exists() {
                dict_search_paths.insert(home_dict_dir);
            }
        } else if env::consts::OS == "macos"
            && let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_folder) = exe_path.parent()
        {
            dict_search_paths.insert(exe_folder.join("../Resources/resources/spellcheck/en_US/"));
        } else if env::consts::OS == "windows"
            && let Ok(exe_path) = std::env::current_exe()
            && let Some(exe_folder) = exe_path.parent()
        {
            dict_search_paths.insert(exe_folder.join("../resources/spellcheck/en_US/"));
        }

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
