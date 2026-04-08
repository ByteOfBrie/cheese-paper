use egui::{Color32, Label, Sense, TextFormat, Vec2, Widget, text::LayoutJob};

use crate::ui::prelude::*;

#[derive(Debug)]
pub struct TextBoxSearchResult {
    // File object that this text box is in
    pub page: Page,

    pub box_name: String,

    // sorted list of search matches in the text
    pub finds: Vec<WordFind>,

    // version of the text that this was computed for
    pub text_version: usize,
}

#[derive(Debug, Clone)]
pub struct WordFind {
    pub start: usize,
    pub end: usize,
    preview: WordFindPreview,
}

#[derive(Debug, Clone)]
struct WordFindPreview {
    context: String,
    word_start: usize,
    word_end: usize,
    line_number: usize,
}

impl WordFind {
    pub fn ui(&self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(&self.preview)
    }
}

impl Widget for &WordFindPreview {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let font_id = egui::TextStyle::Body.resolve(ui.style());

        let context_format = TextFormat {
            font_id: font_id.clone(),
            ..Default::default()
        };
        let match_format = TextFormat {
            color: Color32::WHITE,
            font_id,
            ..Default::default()
        };

        let mut job = LayoutJob::default();

        job.append(
            &self.context[0..self.word_start],
            0.0,
            context_format.clone(),
        );
        job.append(
            &self.context[self.word_start..self.word_end],
            0.0,
            match_format,
        );
        job.append(
            &self.context[self.word_end..self.context.len()],
            0.0,
            context_format,
        );

        ui.horizontal(|ui| {
            ui.add_sized(
                Vec2::new(20.0, 10.0),
                Label::new(self.line_number.to_string()),
            );

            ui.add(
                Label::new(job)
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .selectable(false)
                    .sense(Sense::click()),
            )
        })
        .inner
    }
}

pub fn search(text: &Text, page: &Page, box_name: &str, search_term: &str) -> TextBoxSearchResult {
    let mut finds = Vec::new();

    let mut line_start = 0;

    for (line_number, line) in text.text.split('\n').enumerate() {
        for (start_in_line, m) in line.match_indices(search_term) {
            let preview = WordFindPreview {
                context: line.to_string(),
                word_start: start_in_line,
                word_end: start_in_line + m.len(),
                line_number: line_number + 1,
            };

            let start = line_start + start_in_line;
            let end = start + m.len();

            finds.push(WordFind {
                start,
                end,
                preview,
            });
        }

        line_start += line.len() + 1;
    }

    TextBoxSearchResult {
        page: page.clone(),
        box_name: box_name.to_string(),
        finds,
        text_version: text.version,
    }
}
