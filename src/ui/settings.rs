pub mod settings_page;
pub mod theme;

use crate::ui::prelude::*;

use crate::components::file_objects::utils::{
    create_dir_if_missing, process_name_for_filename, write_with_temp_file,
};

use std::fs::read_dir;
use std::{fs::read_to_string, path::PathBuf};

use toml_edit::{DocumentMut, Item, Value, value};

pub use theme::Theme;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ThemeSelection {
    #[default]
    Default,
    DefaultLight,
    Random,
    Preset(usize),
}

impl From<ThemeSelection> for Value {
    fn from(value: ThemeSelection) -> Self {
        match value {
            ThemeSelection::Default => "default".to_string().into(),
            ThemeSelection::DefaultLight => "light".to_string().into(),
            ThemeSelection::Random => "random".to_string().into(),
            ThemeSelection::Preset(idx) => (idx as i64).into(),
        }
    }
}

impl TryFrom<&Item> for ThemeSelection {
    type Error = ();
    fn try_from(value: &Item) -> Result<Self, Self::Error> {
        if let Some(s) = value.as_str() {
            match s {
                "default" => return Ok(Self::Default),
                "light" => return Ok(Self::DefaultLight),
                "random" => return Ok(Self::Random),
                _ => return Err(()),
            }
        }

        if let Some(v) = value.as_integer() {
            let u: usize = v.try_into().map_err(|_| ())?;
            return Ok(Self::Preset(u));
        }

        Err(())
    }
}

/// instance of a setting. All UIs should expose [`Self::user_editable`]
/// or [`Self::user_entry`]
#[derive(Debug, Clone)]
pub struct Setting<T: std::cmp::PartialEq> {
    /// The value itself, if it has been defined
    value: Option<T>,

    /// The default value for this setting
    default: T,

    /// For types (like bools) that have no system for validation, this is
    /// something that can be exposed
    pub user_editable: T,

    /// Any text entry in progress, only useful for types that have
    /// a way to convert a string to a value
    pub user_entry: String,

    /// Has this entry been modified since the last attempted save to value.
    /// This should be set to true whenever user_entry is modified (by the
    /// function that modifies it). [`Self::validate_update`] will set this
    /// to false
    pub modified_entry: bool,

    /// Has this value been modified since the last time it was written to disk.
    /// Should be set to false whenever this is saved.
    ///
    // We could keep track of this without this bool (in the calling functions),
    // but that sounds annoying to manage
    pub modified_value: bool,

    /// an error message, if any
    ///
    /// not every type will be able to have errors
    pub error_message: Option<String>,
}

impl<T: PartialEq + Clone> Setting<T> {
    pub fn new(default: T) -> Self {
        Self::new_with_value(None, default)
    }

    pub fn new_with_value(value: Option<T>, default: T) -> Self {
        let user_editable = default.clone();
        Self {
            value,
            default,
            user_editable,
            user_entry: String::new(),
            modified_entry: false,
            modified_value: false,
            error_message: None,
        }
    }

    pub fn get_value(&self) -> &T {
        self.value.as_ref().unwrap_or(&self.default)
    }

    pub fn update_entry(&mut self) {
        self.modified_entry = false;
        if self
            .value
            .as_ref()
            .is_none_or(|self_value| *self_value != self.user_editable)
        {
            self.value = Some(self.user_editable.clone());
            self.modified_value = true;
        }
    }

    /// Given a conversion function, try to convert from the string value
    /// to the underlying type
    pub fn validate_update<F>(&mut self, validation_function: F)
    where
        F: Fn(&str) -> Result<T, String>,
    {
        self.modified_entry = false;
        match validation_function(&self.user_entry) {
            Ok(value) => {
                if self
                    .value
                    .as_ref()
                    .is_none_or(|self_value| *self_value != value)
                {
                    self.value = Some(value);
                    self.modified_value = true;
                }
                self.error_message = None;
            }
            Err(err) => {
                self.error_message = Some(err);
            }
        }
    }
}

// String to PathBuf conversion cannot fail, maybe we should get rid of this
pub fn validate_pathbuf(path_str: &str) -> Result<PathBuf, String> {
    path_str.parse::<PathBuf>().map_err(|_| String::new())
}

pub fn validate_f32(float_str: &str) -> Result<f32, String> {
    float_str
        .parse::<f32>()
        .map_err(|float_err| format!("Could not parse number: {float_err}"))
}

#[derive(Debug)]
struct SettingsData {
    settings_path: PathBuf,

    /// size of the text font
    font_size: Setting<f32>,

    /// visual indentation at the start of lines (buggy)
    indent_line_start: Setting<bool>,

    /// highlight multiple spaces in a row
    highlight_multiple_spaces: Setting<bool>,

    /// highlight things like "word ."
    highlight_spaces_before_punctuation: Setting<bool>,

    /// re-open the last project when launching the app
    reopen_last: Setting<bool>,

    /// Check against the latest released version, we should not do
    /// network calls if this is false
    check_for_updates: Setting<bool>,

    /// Location of the Dictionary
    dictionary_location: Setting<PathBuf>,

    theme_settings_modified: bool,

    /// theming for visuals.
    theme: Theme,

    selected_theme: ThemeSelection,

    // theme_selection: ThemeSelection,
    available_themes: Rc<Vec<(String, Theme)>>,
}

impl SettingsData {
    pub fn new(settings_path: PathBuf) -> Self {
        // TODO: #235: do better about this per-platform
        let default_dictionary_location = PathBuf::from("/usr/share/hunspell/en_US");

        Self {
            settings_path,
            font_size: Setting::new(18.0),
            reopen_last: Setting::new(true),
            indent_line_start: Setting::new(false),
            highlight_multiple_spaces: Setting::new(true),
            highlight_spaces_before_punctuation: Setting::new(true),
            check_for_updates: Setting::new(true),
            dictionary_location: Setting::new(default_dictionary_location),
            theme_settings_modified: false,
            theme: Theme::default(),
            selected_theme: ThemeSelection::Default,
            available_themes: Rc::new(Vec::new()),
        }
    }

    pub fn load(&mut self, table: &DocumentMut) {
        if let Some(font_size_item) = table.get("font_size") {
            if let Some(font_size) = font_size_item.as_float() {
                self.font_size.value = Some(font_size as f32);
            } else if let Some(font_size) = font_size_item.as_integer() {
                self.font_size.value = Some(font_size as f32);
            } else {
                log::debug!("Found font size setting but could not parse: {font_size_item:?}");
                self.font_size.error_message =
                    Some(format!("Could not parse as float: {font_size_item}"));
            };

            self.font_size.user_entry = font_size_item.to_string();
        }

        if let Some(reopen_last_item) = table.get("reopen_last")
            && let Some(reopen_last) = reopen_last_item.as_bool()
        {
            self.reopen_last.value = Some(reopen_last);
        }

        if let Some(indent_line_start_item) = table.get("indent_line_start")
            && let Some(indent_line_start) = indent_line_start_item.as_bool()
        {
            self.indent_line_start.value = Some(indent_line_start);
        }

        if let Some(highlight_multiple_spaces_item) = table.get("highlight_multiple_spaces")
            && let Some(highlight_multiple_spaces) = highlight_multiple_spaces_item.as_bool()
        {
            self.highlight_multiple_spaces.value = Some(highlight_multiple_spaces);
        }

        if let Some(highlight_spaces_before_punctuation_item) =
            table.get("highlight_spaces_before_punctuation")
            && let Some(highlight_spaces_before_punctuation) =
                highlight_spaces_before_punctuation_item.as_bool()
        {
            self.highlight_spaces_before_punctuation.value =
                Some(highlight_spaces_before_punctuation);
        }

        if let Some(check_for_updates_item) = table.get("check_for_updates")
            && let Some(check_for_updates) = check_for_updates_item.as_bool()
        {
            self.check_for_updates.value = Some(check_for_updates);
        }

        if let Some(dictionary_location) = table
            .get("dictionary_location")
            .and_then(|location| location.as_str())
        {
            // Just put the dictionary location into the user input and try to parse it
            self.dictionary_location.user_entry = dictionary_location.to_owned();
            self.dictionary_location.validate_update(validate_pathbuf);

            // We just read this from disk, so the value isn't actually modified
            self.dictionary_location.modified_value = false;
        }

        if let Some(theme_table) = table
            .get("theme")
            .and_then(|theme_item| theme_item.as_table_like())
        {
            self.theme = Theme::load(theme_table);
        } else if let Some(selected_theme) = table
            .get("selected_theme")
            .and_then(|i| ThemeSelection::try_from(i).ok())
        {
            self.selected_theme = selected_theme;
        }
    }

    /// Write from settings *values* to a toml table
    pub fn save(&mut self, table: &mut DocumentMut) -> bool {
        // We always try to update the entire document
        // or if any of the sub-values have been modified
        let modified = self.font_size.modified_value
            || self.reopen_last.modified_value
            || self.indent_line_start.modified_value
            || self.highlight_multiple_spaces.modified_value
            || self.highlight_spaces_before_punctuation.modified_value
            || self.check_for_updates.modified_value
            || self.theme_settings_modified;

        self.font_size.modified_value = false;
        if let Some(font_size) = self.font_size.value {
            table.insert("font_size", value(font_size as f64));
        } else {
            table.remove("font_size");
        }

        self.reopen_last.modified_value = false;
        if let Some(reopen_last) = self.reopen_last.value {
            table.insert("reopen_last", value(reopen_last));
        } else {
            table.remove("reopen_last");
        }

        self.indent_line_start.modified_value = false;
        if let Some(indent_line_start) = self.indent_line_start.value {
            table.insert("indent_line_start", value(indent_line_start));
        } else {
            table.remove("indent_line_start");
        }

        self.highlight_multiple_spaces.modified_value = false;
        if let Some(highlight_multiple_spaces) = self.highlight_multiple_spaces.value {
            table.insert(
                "highlight_multiple_spaces",
                value(highlight_multiple_spaces),
            );
        } else {
            table.remove("highlight_multiple_spaces");
        }

        self.highlight_spaces_before_punctuation.modified_value = false;
        if let Some(highlight_spaces_before_punctuation) =
            self.highlight_spaces_before_punctuation.value
        {
            table.insert(
                "highlight_spaces_before_punctuation",
                value(highlight_spaces_before_punctuation),
            );
        } else {
            table.remove("highlight_spaces_before_punctuation");
        }

        self.check_for_updates.modified_value = false;
        if let Some(check_for_updates) = self.check_for_updates.value {
            table.insert("check_for_updates", value(check_for_updates));
        } else {
            table.remove("check_for_updates");
        }

        // TODO: this is somewhat hack-y as we change settings around, this
        // should get looked at again later
        self.theme_settings_modified = false;
        if self.selected_theme == ThemeSelection::Default {
            table.remove("selected_theme");
        } else {
            table.insert("selected_theme", value(self.selected_theme));
        }

        modified
    }

    /// Call every validate_update function, moving the data from the UI into
    /// the values, should be called on a regular basis
    pub fn update_values(&mut self) {
        self.font_size.validate_update(validate_f32);
        self.indent_line_start.update_entry();
        self.highlight_multiple_spaces.update_entry();
        self.highlight_spaces_before_punctuation.update_entry();
        self.reopen_last.update_entry();
        self.check_for_updates.update_entry();
        self.dictionary_location.update_entry();
    }

    fn config_file_path(&self) -> PathBuf {
        self.settings_path.join("settings.toml")
    }

    fn themes_path(&self) -> PathBuf {
        self.settings_path.join("themes")
    }
}

// TODO: convert to named struct
#[derive(Debug, Clone)]
pub struct Settings(Rc<RefCell<SettingsData>>, DocumentMut);

impl Settings {
    pub fn new(config_dir: PathBuf) -> Self {
        Self(
            Rc::new(RefCell::new(SettingsData::new(config_dir))),
            DocumentMut::new(),
        )
    }

    pub fn load(&mut self) -> Result<(), CheeseError> {
        let mut data = self.0.borrow_mut();

        let settings_toml = if data.config_file_path().exists() {
            match read_to_string(data.config_file_path()) {
                Ok(config) => config
                    .parse::<DocumentMut>()
                    .map_err(|err| cheese_error!("invalid toml settings file: {err}"))?,
                Err(err) => {
                    return Err(cheese_error!(
                        "Unknown error while reading editor settings: {err}"
                    ));
                }
            }
        } else {
            // we don't have a config file, create the documentmut now
            DocumentMut::new()
        };

        data.load(&settings_toml);

        self.1 = settings_toml;

        let mut available_themes = Vec::new();

        if let Ok(dir) = read_dir(data.themes_path()) {
            for entry in dir {
                let entry_path = entry?.path();
                if entry_path.is_file()
                    && entry_path.extension().is_some_and(|ext| ext == "toml")
                    && let Ok(theme_config) = read_to_string(&entry_path)
                {
                    let theme_config = match theme_config.parse::<DocumentMut>() {
                        Ok(doc) => doc,
                        Err(err) => {
                            log::error!(
                                "Error encountered while reading config at {} : {err}",
                                entry_path.to_str().unwrap_or_default()
                            );
                            continue;
                        }
                    };

                    let Some(name) = theme_config.get("name").and_then(|item| item.as_str()) else {
                        log::error!(
                            "Error while parsing theme at {}: theme must have a name",
                            entry_path.to_str().unwrap_or_default()
                        );
                        continue;
                    };

                    let theme = Theme::load(theme_config.as_table());

                    available_themes.push((name.to_string(), theme));
                }
            }
        }

        data.available_themes = Rc::new(available_themes);

        let selected_theme = data.selected_theme;

        // release the RefCell
        drop(data);

        if !matches!(selected_theme, ThemeSelection::Default) {
            self.select_theme(selected_theme).unwrap_or_else(|err| {
                log::error!("Error encountered while applying the config's selected theme: {err}");
            });
        }

        Ok(())
    }

    pub fn save(&mut self) -> Result<(), CheeseError> {
        let mut data = self.0.borrow_mut();

        data.update_values();

        // If we have chan
        if data.save(&mut self.1) {
            write_with_temp_file(
                create_dir_if_missing(&data.config_file_path())?,
                self.1.to_string(),
            )
            .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;
        }

        Ok(())
    }

    pub fn font_size(&self) -> f32 {
        *self.0.borrow().font_size.get_value()
    }

    pub fn reopen_last(&self) -> bool {
        *self.0.borrow().reopen_last.get_value()
    }

    pub fn set_reopen_last(&mut self, reopen_last: bool) {
        // TODO: look at usage of this function and maybe do better
        self.0.borrow_mut().reopen_last.value = Some(reopen_last);
    }

    pub fn indent_line_start(&self) -> bool {
        *self.0.borrow().indent_line_start.get_value()
    }

    pub fn highlight_multiple_spaces(&self) -> bool {
        *self.0.borrow().highlight_multiple_spaces.get_value()
    }

    pub fn highlight_spaces_before_punctuation(&self) -> bool {
        *self
            .0
            .borrow()
            .highlight_spaces_before_punctuation
            .get_value()
    }

    pub fn check_for_updates(&self) -> bool {
        *self.0.borrow().check_for_updates.get_value()
    }

    pub fn dictionary_location(&self) -> PathBuf {
        self.0.borrow().dictionary_location.get_value().clone()
    }

    pub fn theme(&self) -> Theme {
        self.0.borrow().theme.clone()
    }

    pub fn select_theme(&self, selection: ThemeSelection) -> Result<(), CheeseError> {
        let mut data = self.0.borrow_mut();
        match selection {
            ThemeSelection::Default => {
                data.theme = Theme::default();
            }
            ThemeSelection::DefaultLight => {
                data.theme = Theme::default_light();
            }
            ThemeSelection::Random => {
                let new_theme = Theme::new_random();
                data.theme = new_theme;
            }
            ThemeSelection::Preset(idx) => {
                data.theme = data
                    .available_themes
                    .get(idx)
                    .ok_or(cheese_error!(
                        "there does not exist a custom theme with index {idx}"
                    ))?
                    .1
                    .clone()
            }
        }
        data.selected_theme = selection;
        data.theme_settings_modified = true;
        Ok(())
    }

    fn available_themes(&self) -> Rc<Vec<(String, Theme)>> {
        let data = self.0.borrow();
        data.available_themes.clone()
    }

    fn selected_theme(&self) -> ThemeSelection {
        self.0.borrow().selected_theme
    }

    fn save_current_theme(&self, name: &str) -> Result<(), CheeseError> {
        let data = self.0.borrow();

        let file_name = process_name_for_filename(name);

        let mut config = DocumentMut::new();

        config.insert("name", value(name));

        data.theme.save(config.as_table_mut());

        let mut dest_path = data.themes_path().join(file_name);
        dest_path.add_extension("toml");

        write_with_temp_file(create_dir_if_missing(&dest_path)?, config.to_string())
            .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;

        Ok(())
    }
}
