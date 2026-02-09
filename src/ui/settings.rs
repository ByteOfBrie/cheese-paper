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

#[derive(Debug, Default, Clone, Copy)]
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

#[derive(Debug)]
struct SettingsData {
    settings_path: PathBuf,

    /// size of the text font
    font_size: f32,

    /// visual indentation at the start of lines (buggy)
    indent_line_start: bool,

    /// highlight multiple spaces in a row
    highlight_multiple_spaces: bool,

    /// re-open the last project when launching the app
    reopen_last: bool,

    /// Location of the Dictionary
    dictionary_location: PathBuf,

    /// theming for visuals.
    theme: Theme,

    selected_theme: ThemeSelection,

    // theme_selection: ThemeSelection,
    available_themes: Rc<Vec<(String, Theme)>>,

    modified: bool,
}

impl SettingsData {
    pub fn new(settings_path: PathBuf) -> Self {
        Self {
            settings_path,
            font_size: 18.0,
            reopen_last: true,
            indent_line_start: false,
            highlight_multiple_spaces: true,
            dictionary_location: PathBuf::from("/usr/share/hunspell/en_US"),
            theme: Theme::default(),
            selected_theme: ThemeSelection::Default,
            available_themes: Rc::new(Vec::new()),
            modified: false,
        }
    }

    pub fn load(&mut self, table: &DocumentMut) {
        match table.get("font_size") {
            Some(font_size_item) => {
                if let Some(font_size) = font_size_item.as_float() {
                    self.font_size = font_size as f32;
                } else if let Some(font_size) = font_size_item.as_integer() {
                    self.font_size = font_size as f32;
                } else {
                    self.modified = true;
                }
            }
            None => self.modified = true,
        }

        match table.get("reopen_last").and_then(|val| val.as_bool()) {
            Some(reopen_last) => self.reopen_last = reopen_last,
            None => self.modified = true,
        }

        match table.get("indent_line_start").and_then(|val| val.as_bool()) {
            Some(indent_line_start) => self.indent_line_start = indent_line_start,
            None => self.modified = true,
        }

        match table
            .get("highlight_multiple_spaces")
            .and_then(|val| val.as_bool())
        {
            Some(highlight_multiple_spaces) => {
                self.highlight_multiple_spaces = highlight_multiple_spaces
            }
            None => self.modified = true,
        }

        if let Some(dictionary_location) = table
            .get("dictionary_location")
            .and_then(|location| location.as_str())
        {
            self.dictionary_location = PathBuf::from(dictionary_location);
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

    pub fn save(&self, table: &mut DocumentMut) {
        table.insert("font_size", value(self.font_size as f64));
        table.insert("reopen_last", value(self.reopen_last));
        table.insert("indent_line_start", value(self.indent_line_start));
        table.insert(
            "highlight_multiple_spaces",
            value(self.highlight_multiple_spaces),
        );
        table.insert("selected_theme", value(self.selected_theme));
    }

    fn config_file_path(&self) -> PathBuf {
        self.settings_path.join("settings.toml")
    }

    fn themes_path(&self) -> PathBuf {
        self.settings_path.join("themes")
    }
}

#[derive(Debug, Clone)]
pub struct Settings(Rc<RefCell<SettingsData>>);

impl Settings {
    pub fn new(config_dir: PathBuf) -> Self {
        Self(Rc::new(RefCell::new(SettingsData::new(config_dir))))
    }

    pub fn load(&mut self) -> Result<(), CheeseError> {
        let mut data = self.0.borrow_mut();

        let settings_toml = match read_to_string(data.config_file_path()) {
            Ok(config) => config
                .parse::<DocumentMut>()
                .map_err(|err| cheese_error!("invalid toml settings file: {err}"))?,
            Err(err) => match err.kind() {
                // It's perfectly normal for there not to be a file, but any other IO error is a problem
                std::io::ErrorKind::NotFound => DocumentMut::new(),
                _ => {
                    return Err(cheese_error!(
                        "Unknown error while reading editor settings: {err}"
                    ));
                }
            },
        };

        data.load(&settings_toml);

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

    pub fn save(&self) -> Result<(), CheeseError> {
        let mut settings_toml = DocumentMut::new();
        let mut data = self.0.borrow_mut();

        data.save(&mut settings_toml);
        write_with_temp_file(
            create_dir_if_missing(&data.config_file_path())?,
            settings_toml.to_string(),
        )
        .map_err(|err| cheese_error!("Error while saving app settings\n{}", err))?;

        data.modified = false;

        Ok(())
    }

    pub fn font_size(&self) -> f32 {
        self.0.borrow().font_size
    }

    pub fn reopen_last(&self) -> bool {
        self.0.borrow().reopen_last
    }

    pub fn set_reopen_last(&mut self, reopen_last: bool) {
        self.0.borrow_mut().reopen_last = reopen_last;
    }

    pub fn indent_line_start(&self) -> bool {
        self.0.borrow().indent_line_start
    }

    pub fn highlight_multiple_spaces(&self) -> bool {
        self.0.borrow().highlight_multiple_spaces
    }

    pub fn dictionary_location(&self) -> PathBuf {
        self.0.borrow().dictionary_location.clone()
    }

    pub fn theme(&self) -> Theme {
        self.0.borrow().theme.clone()
    }

    pub fn modified(&self) -> bool {
        self.0.borrow().modified
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
        data.modified = true;
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
