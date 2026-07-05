use std::time::{Duration, Instant};

use crate::{components::file_objects::WordCountInfo, ui::prelude::*};

impl Project {
    pub fn statistics_ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> CheeseResponse {
        if ctx
            .statistics
            .last_update
            .is_none_or(|time| time.elapsed() >= Duration::from_secs(5))
        {
            self.update_statistics(ctx);
        }

        ui.label(format!(
            "Words in export: {}",
            ctx.statistics.word_count.export
        ));

        ui.label(format!(
            "Words in scenes not in export: {}",
            ctx.statistics.word_count.non_export
        ));

        ui.label(format!(
            "Words in all other fields: {}",
            ctx.statistics.word_count.all_fields
        ));

        CheeseResponse::default()
    }

    /// Update the word count of the project
    fn update_statistics(&self, ctx: &mut EditorContext) {
        let text_word_count = self
            .objects
            .get(&self.top_level_folders[0])
            .unwrap()
            .borrow()
            .word_counts(&self.objects, ctx);

        let mut top_level_folders = self.top_level_folders.iter();
        top_level_folders.next();

        let other_word_counts = top_level_folders
            .map(|id| {
                self.objects
                    .get(id)
                    .unwrap()
                    .borrow()
                    .word_counts(&self.objects, ctx)
            })
            .fold(WordCountInfo::default(), |acc, item| acc + item);

        let current_word_count = WordCountInfo {
            export: text_word_count.export,
            non_export: text_word_count.non_export
                + other_word_counts.export
                + other_word_counts.non_export,
            all_fields: text_word_count.all_fields + other_word_counts.all_fields,
        };

        ctx.statistics.word_count = current_word_count;
        ctx.statistics.last_update = Some(Instant::now());
    }
}
