use crate::ui::prelude::*;

use egui::{Color32, TextFormat, text::LayoutJob};

/// A ui element containing text which will be rendered nicely
///
/// Eventually, the Style logic will be facotrized with the text box style, to allow consistent markdown rendering in non-editable text boxes.
/// For now, however, this rendering will be much simpler
#[derive(Debug)]
pub struct RenderedText {
    text: String,
    layout_job: Option<(LayoutJob, usize, egui::Style)>,
}

impl RenderedText {
    pub fn new(text: String) -> Self {
        Self {
            text,
            layout_job: None,
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &EditorContext) {
        let redo_layout = self
            .layout_job
            .as_ref()
            .is_none_or(|(_, render_version, style)| {
                style != ui.style().as_ref() || render_version != &ctx.render_version
            });

        if redo_layout {
            let style = ui.style().as_ref();
            self.layout_job = Some((
                compute_layout_job(&self.text, ctx, style, ui.available_width()),
                ctx.render_version,
                style.clone(),
            ))
        }

        // TODO: determine if egui actually expects us to use fonts_mut here
        let galley = ui.fonts_mut(|f| f.layout_job(self.layout_job.as_ref().unwrap().0.clone()));

        ui.painter()
            .galley(ui.next_widget_position(), galley, Color32::BLACK);
    }
}

fn compute_layout_job(
    text: &str,
    _ctx: &EditorContext,
    egui_style: &egui::Style,
    wrap_width: f32,
) -> LayoutJob {
    let font_id = egui_style
        .text_styles
        .get(&egui::TextStyle::Body)
        .unwrap()
        .clone();

    let mut job = LayoutJob::simple(
        "".to_string(),
        font_id.clone(),
        egui_style.visuals.text_color(),
        wrap_width,
    );

    let format = TextFormat {
        font_id,
        color: egui_style.visuals.text_color(),
        ..Default::default()
    };

    job.append(text, 0.0, format);

    job
}
