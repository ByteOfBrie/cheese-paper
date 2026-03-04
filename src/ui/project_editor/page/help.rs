use crate::ui::prelude::*;

#[derive(Debug, Default)]
pub struct HelpPage {
    rendered_text: Option<RenderedText>,
}

const HELP_TEXT: &str = include_str!("../../../../manual.md");

impl HelpPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        let rendered_text = self
            .rendered_text
            .get_or_insert_with(|| RenderedText::new(HELP_TEXT.to_owned()));

        rendered_text.ui(ui, ctx);

        CheeseResponse::default()
    }
}
