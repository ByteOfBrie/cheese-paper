mod apply;
mod dictionaries;
pub mod settings_page;
pub mod theme;

use crate::ui::prelude::*;

use dictionaries::AvailableDictionary;
use spellbook::Dictionary;

use crate::components::file_objects::utils::{
    create_dir_if_missing, process_name_for_filename, write_with_temp_file,
};

use std::fs::read_dir;
use std::time::SystemTime;
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
pub struct Setting<T, U = T>
where
    T: PartialEq,
{
    /// The value itself, if it has been defined
    value: Option<T>,

    // override for the value in the project-local settings
    pl_value: Option<T>,

    /// The default value for this setting
    default: T,

    /// The value currently entered in the user in the settings interface
    pub interface_value: U,

    /// Same, for project-local interface
    pub pl_interface_value: U,

    /// Has this value been modified
    ///
    /// Set to true when the interface value overrides the selected value
    /// Set to false when the Settings are processed and saved
    modified: bool,

    convert: fn(T) -> U,

    /// function for convertingt the user selected value into the selected setting
    validate: fn(&U) -> Result<T, &'static str>,

    /// an error message, if any
    ///
    /// not every type will be able to have errors
    pub error_message: Option<String>,
}

impl<T: PartialEq + Clone> Setting<T> {
    /// A setting in which the Ui interface is a transparant representation of the setting value
    pub fn transparent(default: T) -> Self {
        fn validate<T: Clone>(t: &T) -> Result<T, &'static str> {
            Ok(t.clone())
        }

        Self::with_validation_fn(default, std::convert::identity, validate)
    }
}

impl<T, U> Setting<T, U>
where
    T: PartialEq + Clone,
    U: Clone,
{
    fn with_validation_fn(
        default: T,
        convert: fn(T) -> U,
        validate: fn(&U) -> Result<T, &'static str>,
    ) -> Self {
        Self {
            value: None,
            pl_value: None,
            interface_value: convert(default.clone()),
            pl_interface_value: convert(default.clone()),
            default,
            modified: false,
            convert,
            validate,
            error_message: None,
        }
    }

    pub fn get_value(&self) -> &T {
        if let Some(v) = &self.pl_value {
            v
        } else if let Some(v) = &self.value {
            v
        } else {
            &self.default
        }
    }

    pub fn set_value(&mut self, value: Option<T>, project_local: bool) {
        self.pl_interface_value = (self.convert)(value.as_ref().unwrap_or(&self.default).clone());
        self.interface_value = (self.convert)(value.as_ref().unwrap_or(&self.default).clone());
        if project_local {
            self.pl_value = value;
        } else {
            self.value = value;
        }
    }

    pub fn update_value(&mut self, project_local: bool) {
        let new_value = (self.validate)(if project_local {
            &self.pl_interface_value
        } else {
            &self.interface_value
        });

        if let Ok(new_value) = new_value
            && new_value != *self.get_value()
        {
            self.set_value(Some(new_value), project_local);
            self.modified = true;
        }
    }

    pub fn reset_value(&mut self, project_local: bool) {
        if project_local {
            self.pl_value = None;
            self.pl_interface_value = (self.convert)(self.default.clone());
        } else {
            self.value = None;
            self.interface_value = (self.convert)(self.default.clone());
        }
        self.modified = true;
    }

    fn value(&mut self, project_local: bool) -> &mut Option<T> {
        if project_local {
            &mut self.pl_value
        } else {
            &mut self.value
        }
    }

    fn interface_value(&mut self, project_local: bool) -> &mut U {
        if project_local {
            &mut self.pl_interface_value
        } else {
            &mut self.interface_value
        }
    }
}

#[derive(Debug)]
struct SettingsData {
    settings_path: PathBuf,

    /// size of the text font
    font_size: Setting<f32, String>,

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

    available_dict: Vec<AvailableDictionary>,

    selected_dictionary: Setting<String>,

    theme_settings_modified: bool,

    /// theming for visuals.
    theme: Theme,

    selected_theme: ThemeSelection,

    // theme_selection: ThemeSelection,
    available_themes: Rc<Vec<(String, Theme)>>,

    next_apply: Option<SystemTime>,

    pl_next_apply: Option<SystemTime>,
}

fn convert_font(font: f32) -> String {
    format!("{font}")
}

#[allow(clippy::ptr_arg)]
fn validate_font(input: &String) -> Result<f32, &'static str> {
    let f = input
        .parse::<f32>()
        .map_err(|_| "Could not parse text as float");

    if let Ok(value) = f
        && value < 5.0
    {
        return Err("Value too small to use as a font size");
    }

    f
}

impl SettingsData {
    pub fn new(settings_path: PathBuf) -> Self {
        Self {
            settings_path,
            font_size: Setting::with_validation_fn(18.0, convert_font, validate_font),
            reopen_last: Setting::transparent(true),
            indent_line_start: Setting::transparent(false),
            highlight_multiple_spaces: Setting::transparent(true),
            highlight_spaces_before_punctuation: Setting::transparent(true),
            check_for_updates: Setting::transparent(true),
            available_dict: Vec::new(),
            selected_dictionary: Setting::transparent("en_US".to_owned()),
            theme_settings_modified: false,
            theme: Theme::default(),
            selected_theme: ThemeSelection::Default,
            available_themes: Rc::new(Vec::new()),
            next_apply: None,
            pl_next_apply: None,
        }
    }

    pub fn load(&mut self, table: &DocumentMut, project_local: bool) {
        if let Some(font_size_item) = table.get("font_size") {
            if let Some(font_size) = font_size_item.as_float() {
                self.font_size
                    .set_value(Some(font_size as f32), project_local);
            } else if let Some(font_size) = font_size_item.as_integer() {
                self.font_size
                    .set_value(Some(font_size as f32), project_local);
            } else {
                log::debug!("Found font size setting but could not parse: {font_size_item:?}");
                self.font_size.error_message =
                    Some(format!("Could not parse as float: {font_size_item}"));
            };
        }

        if let Some(reopen_last_item) = table.get("reopen_last")
            && let Some(reopen_last) = reopen_last_item.as_bool()
        {
            self.reopen_last.set_value(Some(reopen_last), project_local);
        }

        if let Some(indent_line_start_item) = table.get("indent_line_start")
            && let Some(indent_line_start) = indent_line_start_item.as_bool()
        {
            self.indent_line_start
                .set_value(Some(indent_line_start), project_local);
        }

        if let Some(highlight_multiple_spaces_item) = table.get("highlight_multiple_spaces")
            && let Some(highlight_multiple_spaces) = highlight_multiple_spaces_item.as_bool()
        {
            self.highlight_multiple_spaces
                .set_value(Some(highlight_multiple_spaces), project_local);
        }

        if let Some(highlight_spaces_before_punctuation_item) =
            table.get("highlight_spaces_before_punctuation")
            && let Some(highlight_spaces_before_punctuation) =
                highlight_spaces_before_punctuation_item.as_bool()
        {
            self.highlight_spaces_before_punctuation
                .set_value(Some(highlight_spaces_before_punctuation), project_local);
        }

        if let Some(check_for_updates_item) = table.get("check_for_updates")
            && let Some(check_for_updates) = check_for_updates_item.as_bool()
        {
            self.check_for_updates
                .set_value(Some(check_for_updates), project_local);
        }

        if let Some(selected_dictionary) = table.get("selected_dictionary")
            && let Some(selected_dictionary) = selected_dictionary.as_str()
        {
            self.selected_dictionary
                .set_value(Some(selected_dictionary.to_owned()), project_local);
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

        self.load_available_dictionaries()
            .unwrap_or_else(|err| log::error!("Error loading availabel dictionaries: {err}"));
    }

    /// Write from settings *values* to a toml table
    pub fn save(&mut self, table: &mut DocumentMut, project_local: bool) -> bool {
        // We always try to update the entire document
        // or if any of the sub-values have been modified
        let modified = self.font_size.modified
            || self.reopen_last.modified
            || self.indent_line_start.modified
            || self.highlight_multiple_spaces.modified
            || self.highlight_spaces_before_punctuation.modified
            || self.check_for_updates.modified
            || self.theme_settings_modified;

        self.font_size.modified = false;
        if let Some(font_size) = *self.font_size.value(project_local) {
            table.insert("font_size", value(font_size as f64));
        } else {
            table.remove("font_size");
        }

        self.reopen_last.modified = false;
        if let Some(reopen_last) = *self.reopen_last.value(project_local) {
            table.insert("reopen_last", value(reopen_last));
        } else {
            table.remove("reopen_last");
        }

        self.indent_line_start.modified = false;
        if let Some(indent_line_start) = *self.indent_line_start.value(project_local) {
            table.insert("indent_line_start", value(indent_line_start));
        } else {
            table.remove("indent_line_start");
        }

        self.highlight_multiple_spaces.modified = false;
        if let Some(highlight_multiple_spaces) =
            *self.highlight_multiple_spaces.value(project_local)
        {
            table.insert(
                "highlight_multiple_spaces",
                value(highlight_multiple_spaces),
            );
        } else {
            table.remove("highlight_multiple_spaces");
        }

        self.highlight_spaces_before_punctuation.modified = false;
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

        self.check_for_updates.modified = false;
        if let Some(check_for_updates) = *self.check_for_updates.value(project_local) {
            table.insert("check_for_updates", value(check_for_updates));
        } else {
            table.remove("check_for_updates");
        }

        self.selected_dictionary.modified = false;
        if let Some(selected_dictionary) = &self.selected_dictionary.value(project_local) {
            table.insert("selected_dictionary", value(selected_dictionary));
        } else {
            table.remove("selected_dictionary");
        }

        if !project_local {
            // TODO: this is somewhat hack-y as we change settings around, this
            // should get looked at again later
            self.theme_settings_modified = false;
            if self.selected_theme == ThemeSelection::Default {
                table.remove("selected_theme");
            } else {
                table.insert("selected_theme", value(self.selected_theme));
            }
        }

        modified
    }

    /// Call every [`Setting::update_value`] function, moving the data from the UI into
    /// the values, should be called on a regular basis
    pub fn update_values(&mut self, project_local: bool) {
        self.font_size.update_value(project_local);
        self.indent_line_start.update_value(project_local);
        self.highlight_multiple_spaces.update_value(project_local);
        self.highlight_spaces_before_punctuation
            .update_value(project_local);
        self.reopen_last.update_value(project_local);
        self.check_for_updates.update_value(project_local);
        self.selected_dictionary.update_value(project_local);
    }

    /// Try to load the dictionary corresponding to the selected dictionary from the filesystem
    pub fn load_dictionary(&mut self) -> Option<Dictionary> {
        self.selected_dictionary.error_message = None;

        let selection = self.selected_dictionary.get_value();
        if selection.is_empty() {
            return None;
        }

        let dict_selection = self
            .available_dict
            .iter()
            .find(|dict| dict.name == *selection)?;

        dict_selection
            .load()
            .map_err(|err| {
                format!(
                    "An error was encountered loading the dictionary from {dict_selection:?}: {err}"
                )
            }) // chained map_err for lifetime reasons
            .map_err(|error_msg| {
                log::error!("{}", error_msg);
                self.selected_dictionary.error_message = Some(error_msg);
            })
            .ok()
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

        data.load(&settings_toml, false);

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

    pub fn load_project_local(&mut self, project_metadata: &DocumentMut) {
        let mut data = self.0.borrow_mut();
        data.load(project_metadata, true);
    }

    pub fn need_processing(&self, ctx: &egui::Context) -> (bool, bool) {
        let mut data = self.0.borrow_mut();

        let now = SystemTime::now();

        if let Some(next_apply) = data.next_apply {
            if now >= next_apply {
                data.next_apply = None;
                return (true, false);
            } else {
                ctx.request_repaint_after(next_apply.duration_since(now).unwrap());
            }
        }

        if let Some(next_apply) = data.pl_next_apply {
            if now >= next_apply {
                data.next_apply = None;
                return (true, true);
            } else {
                ctx.request_repaint_after(next_apply.duration_since(now).unwrap());
            }
        }

        (false, false)
    }

    pub fn update(&mut self, project_local: bool) {
        self.0.borrow_mut().update_values(project_local);
    }

    pub fn font_size(&self) -> f32 {
        *self.0.borrow().font_size.get_value()
    }

    pub fn reopen_last(&self) -> bool {
        *self.0.borrow().reopen_last.get_value()
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

    /// Try to load the dictionary corresponding to the selected dictionary from the filesystem
    pub fn load_dictionary(&self) -> Option<Dictionary> {
        let mut data = self.0.borrow_mut();

        data.load_dictionary()
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
