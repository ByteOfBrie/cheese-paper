use std::sync::LazyLock;

#[allow(unused)]
use crate::ui::prelude::*;

use egui::{Color32, Visuals};
use egui::{Style, style::WidgetVisuals};
use rand::{RngExt, rngs::ThreadRng};
use toml_edit::{InlineTable, TableLike, value};

#[derive(Debug, Clone)]
struct WidgetTheme {
    fg_stroke_color: Option<Color32>,
    bg_stroke_color: Option<Color32>,
    bg_fill: Option<Color32>,
    weak_bg_fill: Option<Color32>,
}

impl WidgetTheme {
    fn new_random(rng: &mut ThreadRng) -> Self {
        Self {
            fg_stroke_color: Some(random_color32(rng)),
            bg_stroke_color: Some(random_color32(rng)),
            bg_fill: Some(random_color32(rng)),
            weak_bg_fill: Some(random_color32(rng)),
        }
    }

    fn update_theme(&self, widget: &mut WidgetVisuals, default_widget: &WidgetVisuals) {
        widget.fg_stroke.color = match self.fg_stroke_color {
            Some(color) => color,
            None => default_widget.fg_stroke.color,
        };

        widget.bg_stroke.color = match self.bg_stroke_color {
            Some(color) => color,
            None => default_widget.bg_stroke.color,
        };

        widget.bg_fill = match self.bg_fill {
            Some(color) => color,
            None => default_widget.bg_fill,
        };

        widget.weak_bg_fill = match self.weak_bg_fill {
            Some(color) => color,
            None => default_widget.weak_bg_fill,
        };
    }
}

fn update_widget_theme(
    widget_theme: &Option<WidgetTheme>,
    widget: &mut WidgetVisuals,
    default_widget: &WidgetVisuals,
) {
    match widget_theme {
        Some(widget_theme) => widget_theme.update_theme(widget, default_widget),
        None => *widget = *default_widget,
    }
}

/// Most of the colors from https://docs.rs/egui/latest/egui/style/struct.Visuals.html
/// doesn't implement everything (because that requires more work), more can be added later
/// as requested/desired
#[derive(Debug, Default, Clone)]
pub struct Theme {
    // indicates if this should be a a light theme.
    // this also changes which default theme gets used for unset values
    light_mode: bool,

    override_text_color: Option<Color32>,

    weak_text_color: Option<Color32>,

    hyperlink_color: Option<Color32>,

    /// Barely different from bg color, used for striped grids
    faint_bg_color: Option<Color32>,

    extreme_bg_color: Option<Color32>,

    /// Default: extreme_bg_color
    text_edit_bg_color: Option<Color32>,

    warn_fg_color: Option<Color32>,

    error_fg_color: Option<Color32>,

    window_fill_color: Option<Color32>,

    panel_fill_color: Option<Color32>,

    window_stroke_color: Option<Color32>,

    selection_bg_color: Option<Color32>,

    selection_fg_stroke_color: Option<Color32>,

    active_widget: Option<WidgetTheme>,
    inactive_widget: Option<WidgetTheme>,
    noninteractive_widget: Option<WidgetTheme>,
    hovered_widget: Option<WidgetTheme>,
    open_widget: Option<WidgetTheme>,
}

fn read_color32(table: &dyn TableLike, field: &str) -> Option<Color32> {
    match Color32::from_hex(table.get(field)?.as_str()?) {
        Ok(color) => Some(color),
        Err(err) => {
            log::warn!("Could not parse color for {field}: {err:?}");
            None
        }
    }
}

fn write_color32(table: &mut dyn TableLike, field: &str, color: Option<Color32>) {
    if let Some(color) = color {
        table.insert(field, value(color.to_hex()));
    }
}

fn random_color32(rng: &mut ThreadRng) -> Color32 {
    Color32::from_rgb(rng.random(), rng.random(), rng.random())
}

fn read_widget_theme(table: &dyn TableLike, field: &str) -> Option<WidgetTheme> {
    match table.get(field).and_then(|field| field.as_table_like()) {
        Some(widget_table) => {
            let fg_stroke_color = read_color32(widget_table, "fg_stroke_color");
            let bg_stroke_color = read_color32(widget_table, "bg_stroke_color");
            let bg_fill = read_color32(widget_table, "bg_fill");
            let weak_bg_fill = read_color32(widget_table, "weak_bg_fill");

            Some(WidgetTheme {
                fg_stroke_color,
                bg_stroke_color,
                bg_fill,
                weak_bg_fill,
            })
        }
        None => None,
    }
}

fn write_widget_theme(table: &mut dyn TableLike, field: &str, theme: &Option<WidgetTheme>) {
    if let Some(theme) = theme {
        let mut inline_table = InlineTable::new();
        write_color32(&mut inline_table, "fg_stroke_color", theme.fg_stroke_color);
        write_color32(&mut inline_table, "bg_stroke_color", theme.bg_stroke_color);
        write_color32(&mut inline_table, "bg_fill", theme.bg_fill);
        write_color32(&mut inline_table, "weak_bg_fill", theme.weak_bg_fill);

        table.insert(field, value(inline_table));
    }
}

impl Theme {
    pub fn default_light() -> Self {
        Self {
            light_mode: true,
            ..Default::default()
        }
    }

    pub fn new_random() -> Self {
        let mut rng = rand::rng();

        Self {
            light_mode: rng.random_bool(0.5),
            override_text_color: Some(random_color32(&mut rng)),
            weak_text_color: Some(random_color32(&mut rng)),
            hyperlink_color: Some(random_color32(&mut rng)),
            faint_bg_color: Some(random_color32(&mut rng)),
            extreme_bg_color: Some(random_color32(&mut rng)),
            text_edit_bg_color: Some(random_color32(&mut rng)),
            warn_fg_color: Some(random_color32(&mut rng)),
            error_fg_color: Some(random_color32(&mut rng)),
            window_fill_color: Some(random_color32(&mut rng)),
            panel_fill_color: Some(random_color32(&mut rng)),
            window_stroke_color: Some(random_color32(&mut rng)),
            selection_bg_color: Some(random_color32(&mut rng)),
            selection_fg_stroke_color: Some(random_color32(&mut rng)),
            active_widget: Some(WidgetTheme::new_random(&mut rng)),
            inactive_widget: Some(WidgetTheme::new_random(&mut rng)),
            noninteractive_widget: Some(WidgetTheme::new_random(&mut rng)),
            hovered_widget: Some(WidgetTheme::new_random(&mut rng)),
            open_widget: Some(WidgetTheme::new_random(&mut rng)),
        }
    }

    pub fn load(theme_table: &dyn TableLike) -> Self {
        let light_mode = theme_table
            .get("light_mode")
            .and_then(|r| r.as_bool())
            .unwrap_or_default();
        let override_text_color = read_color32(theme_table, "override_text_color");
        let weak_text_color = read_color32(theme_table, "weak_text_color");
        let hyperlink_color = read_color32(theme_table, "hyperlink_color");
        let faint_bg_color = read_color32(theme_table, "faint_bg_color");
        let extreme_bg_color = read_color32(theme_table, "extreme_bg_color");
        let text_edit_bg_color = read_color32(theme_table, "text_edit_bg_color");
        let warn_fg_color = read_color32(theme_table, "warn_fg_color");
        let error_fg_color = read_color32(theme_table, "error_fg_color");
        let window_fill_color = read_color32(theme_table, "window_fill_color");
        let panel_fill_color = read_color32(theme_table, "panel_fill_color");

        let selection_bg_color = read_color32(theme_table, "selection_bg_color");
        let selection_fg_stroke_color = read_color32(theme_table, "selection_fg_stroke_color");
        let window_stroke_color = read_color32(theme_table, "window_stroke_color");

        let active_widget = read_widget_theme(theme_table, "active_widget");
        let inactive_widget = read_widget_theme(theme_table, "inactive_widget");
        let noninteractive_widget = read_widget_theme(theme_table, "noninteractive_widget");
        let hovered_widget = read_widget_theme(theme_table, "hovered_widget");
        let open_widget = read_widget_theme(theme_table, "open_widget");

        Theme {
            light_mode,
            override_text_color,
            weak_text_color,
            hyperlink_color,
            faint_bg_color,
            extreme_bg_color,
            text_edit_bg_color,
            warn_fg_color,
            error_fg_color,
            window_fill_color,
            panel_fill_color,
            selection_bg_color,
            selection_fg_stroke_color,
            window_stroke_color,
            active_widget,
            inactive_widget,
            noninteractive_widget,
            hovered_widget,
            open_widget,
        }
    }

    pub fn save(&self, theme_table: &mut dyn TableLike) {
        theme_table.insert("light_mode", value(self.light_mode));
        write_color32(theme_table, "override_text_color", self.override_text_color);
        write_color32(theme_table, "weak_text_color", self.weak_text_color);
        write_color32(theme_table, "hyperlink_color", self.hyperlink_color);
        write_color32(theme_table, "faint_bg_color", self.faint_bg_color);
        write_color32(theme_table, "extreme_bg_color", self.extreme_bg_color);
        write_color32(theme_table, "text_edit_bg_color", self.text_edit_bg_color);
        write_color32(theme_table, "warn_fg_color", self.warn_fg_color);
        write_color32(theme_table, "error_fg_color", self.error_fg_color);
        write_color32(theme_table, "window_fill_color", self.window_fill_color);
        write_color32(theme_table, "panel_fill_color", self.panel_fill_color);

        write_color32(theme_table, "selection_bg_color", self.selection_bg_color);
        write_color32(
            theme_table,
            "selection_fg_stroke_color",
            self.selection_fg_stroke_color,
        );
        write_color32(theme_table, "window_stroke_color", self.window_stroke_color);

        write_widget_theme(theme_table, "active_widget", &self.active_widget);
        write_widget_theme(theme_table, "inactive_widget", &self.inactive_widget);
        write_widget_theme(
            theme_table,
            "noninteractive_widget",
            &self.noninteractive_widget,
        );
        write_widget_theme(theme_table, "hovered_widget", &self.hovered_widget);
        write_widget_theme(theme_table, "open_widget", &self.open_widget);
    }

    pub fn apply(&self, style: &mut Style) {
        static DEFAULT_STYLE_DARK: LazyLock<Style> = LazyLock::new(Style::default);
        static DEFAULT_STYLE_LIGHT: LazyLock<Style> = LazyLock::new(|| Style {
            visuals: Visuals::light(),
            ..Default::default()
        });

        let default_style = if self.light_mode {
            &*DEFAULT_STYLE_LIGHT
        } else {
            &*DEFAULT_STYLE_DARK
        };

        style.visuals.dark_mode = !self.light_mode;

        style.visuals.override_text_color = self.override_text_color;

        style.visuals.weak_text_color = self.weak_text_color;

        style.visuals.text_edit_bg_color = self.text_edit_bg_color;

        style.visuals.hyperlink_color = match self.hyperlink_color {
            Some(color) => color,
            None => default_style.visuals.hyperlink_color,
        };

        style.visuals.faint_bg_color = match self.faint_bg_color {
            Some(color) => color,
            None => default_style.visuals.faint_bg_color,
        };

        style.visuals.extreme_bg_color = match self.extreme_bg_color {
            Some(color) => color,
            None => default_style.visuals.extreme_bg_color,
        };

        style.visuals.warn_fg_color = match self.warn_fg_color {
            Some(color) => color,
            None => default_style.visuals.warn_fg_color,
        };

        style.visuals.error_fg_color = match self.error_fg_color {
            Some(color) => color,
            None => default_style.visuals.error_fg_color,
        };

        style.visuals.window_fill = match self.window_fill_color {
            Some(color) => color,
            None => default_style.visuals.window_fill,
        };

        style.visuals.panel_fill = match self.panel_fill_color {
            Some(color) => color,
            None => default_style.visuals.panel_fill,
        };

        style.visuals.window_stroke.color = match self.window_stroke_color {
            Some(color) => color,
            None => default_style.visuals.window_stroke.color,
        };

        style.visuals.selection.bg_fill = match self.selection_bg_color {
            Some(color) => color,
            None => default_style.visuals.selection.bg_fill,
        };

        style.visuals.selection.stroke.color = match self.selection_fg_stroke_color {
            Some(color) => color,
            None => default_style.visuals.selection.stroke.color,
        };

        update_widget_theme(
            &self.active_widget,
            &mut style.visuals.widgets.active,
            &default_style.visuals.widgets.active,
        );

        update_widget_theme(
            &self.inactive_widget,
            &mut style.visuals.widgets.inactive,
            &default_style.visuals.widgets.inactive,
        );

        update_widget_theme(
            &self.noninteractive_widget,
            &mut style.visuals.widgets.noninteractive,
            &default_style.visuals.widgets.noninteractive,
        );

        update_widget_theme(
            &self.hovered_widget,
            &mut style.visuals.widgets.hovered,
            &default_style.visuals.widgets.hovered,
        );

        update_widget_theme(
            &self.open_widget,
            &mut style.visuals.widgets.open,
            &default_style.visuals.widgets.open,
        );
    }
}
