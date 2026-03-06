//! A few useful functions for a neater layout
//!

use egui::{RichText, UiBuilder, epaint::MarginF32};

use crate::ui::prelude::*;

const MARGIN: MarginF32 = MarginF32 {
    left: 10.0,
    right: 10.0,
    top: 5.0,
    bottom: 0.0,
};

// This implementation of the margin box is probably not done right, and won't work well in all cases.
// But it's good enough for what we're doing with it for now.

/// A simple box with a nice margin for making pages
pub fn margin_box<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let mut max_rect = ui.max_rect() - MARGIN;
    max_rect.extend_with_y(f32::INFINITY);

    let ui_builder = UiBuilder {
        max_rect: Some(max_rect),
        ..Default::default()
    };

    ui.scope_builder(ui_builder, add_contents).inner
}

const SPACE_BEFORE: f32 = 4.0;
const SPACE_AFTER: f32 = 10.0;

pub fn heading(ui: &mut Ui, text: impl Into<RichText>) -> Response {
    ui.add_space(SPACE_BEFORE);

    let response = ui.heading(text);

    ui.add_space(SPACE_AFTER);

    response
}

pub fn horizontal_heading<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(SPACE_BEFORE);

    let response = ui.horizontal(add_contents).inner;

    ui.add_space(SPACE_AFTER);

    response
}
