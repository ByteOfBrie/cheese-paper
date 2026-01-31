mod format;
mod spellcheck;

use std::ops::Range;

use crate::ui::prelude::*;
use crate::ui::project_editor::search::textbox_search;
use egui::text::{CCursorRange, LayoutJob};
use egui::{Key, KeyboardShortcut, Modifiers, TextBuffer};

pub type Store = RenderDataStore<usize, TextBox>;

#[derive(Debug, Default)]
pub struct TextBox {
    // highlighter: MemoizedMarkdownHighlighter,

    // Memoized Layout Job
    layout_job: LayoutJob,

    word_count: usize,

    // manually force the layout to be redone
    redo_layout: bool,

    // formatting information that the highlight job was for
    // used to know when highlight needs to be redone
    text_signature: (usize, usize),
    editor_signature: usize,
    style: egui::Style,
}

impl TextBox {
    fn refresh(&mut self, text: &Text, ctx: &mut EditorContext) {
        let signature = (text.struct_uid, text.version);

        if signature != self.text_signature {
            self.text_signature = signature;

            self.word_count = spellcheck::word_count(text.as_str());
            self.redo_layout = true;
        }

        if ctx.version != self.editor_signature {
            self.editor_signature = ctx.version;
            self.redo_layout = true;
        }

        if ctx.search.active
            && let Some(search_results) = ctx.search.search_results.as_mut()
            && let Some(sr) = search_results.get_mut(&text.struct_uid)
            && sr.text_version != text.version
        {
            *sr = textbox_search::search(text, &sr.page, &sr.box_name, &ctx.search.find_text);
            ctx.search.clear_focus();
            self.redo_layout = true;
        }
    }

    fn get_layout(
        &mut self,
        ui: &egui::Ui,
        text: &dyn TextBuffer,
        ctx: &mut EditorContext,
    ) -> LayoutJob {
        let text = Text::downcast(text);

        self.refresh(text, ctx);

        if ui.style().as_ref() != &self.style {
            self.style = ui.style().as_ref().clone();

            self.redo_layout = true;
        }

        let (mut search_result, mut search_result_focus) = (None, None);

        if ctx.search.active {
            search_result = ctx
                .search
                .search_results
                .as_ref()
                .and_then(|sr| sr.get(&text.struct_uid));

            search_result_focus = ctx.search.focus.as_ref().and_then(|(uid, word_find)| {
                if *uid == text.struct_uid {
                    Some(word_find)
                } else {
                    None
                }
            });
        }

        if self.redo_layout {
            self.redo_layout = false;
            self.layout_job = format::compute_layout_job(
                text.as_str(),
                ctx,
                search_result,
                search_result_focus,
                &self.style,
            )
        }

        self.layout_job.clone()
    }
}

pub const SHORTCUT_BOLD: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::B);
pub const SHORTCUT_ITALICS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::I);

impl Text {
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorContext) -> Response {
        let rdata = ctx.stores.text_box.get(&self.struct_uid);
        let text_box: &mut TextBox = &mut rdata.borrow_mut();

        let mut layouter = |ui: &egui::Ui, text: &dyn TextBuffer, wrap_width: f32| {
            let mut layout_job = text_box.get_layout(ui, text, ctx);
            layout_job.wrap.max_width = wrap_width;
            ui.fonts(|f| f.layout_job(layout_job))
        };

        let text_box_id = self.struct_uid;

        let mut output = egui::TextEdit::multiline(self)
            .desired_width(f32::INFINITY)
            .layouter(&mut layouter)
            .min_size(egui::Vec2 { x: 50.0, y: 100.0 })
            .lock_focus(true)
            .id_salt(text_box_id)
            .show(ui);

        // Select the cursor text and scroll to it if requried
        if ctx.search.active
            && ctx.search.goto_focus
            && let Some((uid, word_find)) = &ctx.search.focus
            && uid == &self.struct_uid
        {
            // Cursor ranges are specified in character offsets, we have a byte offset,
            // find the word
            let mut start_char_pos: Option<usize> = None;
            let mut end_char_pos: Option<usize> = None;
            for (char_count, (offset, _char)) in self.text.char_indices().enumerate() {
                if start_char_pos.is_none() && offset >= word_find.start {
                    start_char_pos = Some(char_count);
                }
                if end_char_pos.is_none() && offset >= word_find.end {
                    end_char_pos = Some(char_count);
                    break;
                }
            }

            // Special case: if we're looking for a string that matches the end of the file,
            // end_char_pos will be None, we correct it here (rather than trying to fix my
            // logic above because that's complicated)
            if start_char_pos.is_some() && end_char_pos.is_none() {
                end_char_pos = Some(self.text.chars().count());
            }

            if let Some(start_pos) = start_char_pos
                && let Some(end_pos) = end_char_pos
                && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), output.response.id)
            {
                let ccursor = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(start_pos),
                    egui::text::CCursor::new(end_pos),
                );

                // Set the positition of the cursor in the text
                state.cursor.set_char_range(Some(ccursor));
                state.store(ui.ctx(), output.response.id);
                ui.ctx()
                    .memory_mut(|mem| mem.request_focus(output.response.id));

                // Find the position of the cursor position in the rendered text output
                let cursor_pos_in_galley = output
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(start_pos));

                let text_edit_pos = output.response.rect;

                // Add the minimum of the text edit widget to the galley position to get the
                // absolute rectangle
                let cursor_absolute_pos =
                    cursor_pos_in_galley.translate(text_edit_pos.min.to_vec2());

                ui.scroll_to_rect(cursor_absolute_pos, Some(egui::Align::Center));
            }

            // We've gone to our focus (or made our best effort), we're done
            ctx.search.goto_focus = false;
        }

        // Keep track of where we're typing and if it's new, used in spellcheck logic later on
        if let Some(cursor_range) = output.cursor_range {
            // Primary cursor tells us where we're at in the text edit box
            let primary_cursor_pos = cursor_range.primary.index;
            // Collect the character start and end boundaries (byte index)
            let current_word_pos = spellcheck::get_current_word(&self.text, primary_cursor_pos);

            if current_word_pos.is_empty() || current_word_pos.end == self.text.len() {
                ctx.typing_status.is_new_word = true;
                ctx.typing_status.current_word = current_word_pos;
            } else if ctx.typing_status.is_new_word
                && current_word_pos.contains(&ctx.typing_status.current_word.start)
            {
                ctx.typing_status.current_word = current_word_pos
            } else if !current_word_pos.contains(&primary_cursor_pos) {
                // we're editing a word elsewhere
                ctx.typing_status.is_new_word = false;
            }
        }

        // if we've just created a new word (pressed enter or space), force highlighting
        // to happen a second time. We're one frame behind on inputs which wouldn't
        // normally matter except we save highlight input. This means that the word
        // will be spellchecked again, this time not being ignored.
        //
        // We could *possibly* do a little bit better about this by detecting when a new
        // word has been created while still highlighting, but this is visually good and
        // less complicated to implement
        if ui.input(|i| i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Enter)) {
            text_box.redo_layout = true;
        }

        // Check for paste events that contain smart quotes, and remove them from the text
        ui.input_mut(|i| {
            for event in &i.events {
                if let egui::Event::Paste(contents) = event
                    && contents.contains(['“', '”', '‘', '’'])
                {
                    self.clean_up_quotes();

                    // We've changed the text, so we need to update the layout once again
                    text_box.redo_layout = true;
                }
            }
        });

        // Draw spellcheck menu for the current word
        if output.response.clicked_by(egui::PointerButton::Secondary)
            && let Some(cursor_range) = output.cursor_range
        {
            let clicked_pos = cursor_range.primary.index;

            let word_boundaries = spellcheck::get_current_word(&self.text, clicked_pos);

            let raw_word = &self.text[word_boundaries.clone()];

            // Will need word_range when spellcheck corrections are implemented, but it's not needed now
            let (check_word, word_range) = spellcheck::trim_word_for_spellcheck(raw_word);

            let actual_word_start = word_boundaries.start + word_range.start;
            let actual_word_length = word_range.end - word_range.start;
            let actual_word_end = actual_word_start + actual_word_length;

            ctx.spellcheck_status.selected_word = check_word.to_string();
            ctx.spellcheck_status.word_range = actual_word_start..actual_word_end;

            if let Some(dictionary) = ctx.dictionary_state.dictionary.as_ref() {
                if dictionary.check(&ctx.spellcheck_status.selected_word) {
                    ctx.spellcheck_status.correct = true;
                } else {
                    ctx.spellcheck_status.correct = false;
                    ctx.spellcheck_status.suggestions.clear();
                    dictionary.suggest(
                        &ctx.spellcheck_status.selected_word,
                        &mut ctx.spellcheck_status.suggestions,
                    );
                }
            }
        }

        output.response.context_menu(|ui| {
            if ui.button("Select All").clicked()
                && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), output.response.id)
            {
                let ccursor = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(self.text.chars().count()),
                );

                state.cursor.set_char_range(Some(ccursor));

                log::debug!("new range: {:?}, {:?}", state.cursor, output.response.id);
                state.store(ui.ctx(), output.response.id);
                ui.ctx()
                    .memory_mut(|mem| mem.request_focus(output.response.id));
            }

            if !ctx.spellcheck_status.correct {
                ui.separator();
                for suggestion in ctx.spellcheck_status.suggestions.iter() {
                    if ui.button(suggestion).clicked() {
                        let drained_text: String = self
                            .text
                            .drain(ctx.spellcheck_status.word_range.clone())
                            .collect();

                        // double check we didn't mess up indexes or something, we can still
                        // go back to the previous state
                        if drained_text == ctx.spellcheck_status.selected_word {
                            self.text
                                .insert_str(ctx.spellcheck_status.word_range.start, suggestion);
                            self.version += 1;
                        } else {
                            log::error!(
                                "Tried to remove {} at {:?}, but instead got {drained_text}",
                                ctx.spellcheck_status.selected_word,
                                ctx.spellcheck_status.word_range
                            );
                            self.text
                                .insert_str(ctx.spellcheck_status.word_range.start, &drained_text);
                        }
                    }
                }
                ui.separator();

                if ui.button("Add to Dictionary").clicked() {
                    ctx.dictionary_state
                        .add_ignored(&ctx.spellcheck_status.selected_word);
                    ctx.version += 1;
                }
            }
        });

        // process hotkeys like ctrl-b and ctrl-i:
        if let Some(focused_window) = ui.ctx().memory(|i| i.focused())
            && focused_window == output.response.id
            && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), output.response.id)
            && let Some(output_cursor_range) = state.cursor.char_range()
        {
            let mut cursor_range = output_cursor_range;
            let mut changed_formatting = false;

            ui.input_mut(|i| {
                for (shortcut, pattern) in [(SHORTCUT_ITALICS, "*"), (SHORTCUT_BOLD, "**")] {
                    if i.consume_shortcut(&shortcut) {
                        // make the change to the text
                        self.toggle_formatting(&mut cursor_range, pattern);
                        changed_formatting = true;
                    }
                }
            });

            if changed_formatting {
                state.cursor.set_char_range(Some(cursor_range));
                state.store(ui.ctx(), output.response.id);
                output.response.mark_changed();
            }
        }

        output.response
    }

    /// Toggles formatting like italic or bold
    fn toggle_formatting(&mut self, cursor_range: &mut CCursorRange, pattern: &str) {
        let current_working_range = self.get_selection_range_trimmed(cursor_range);

        let already_surrounded = match pattern {
            "*" => self.is_italic(&current_working_range),
            _ => self.is_formatted_generic(&current_working_range, pattern),
        };

        if already_surrounded {
            let deletion_range_start =
                current_working_range.start..current_working_range.start + pattern.len();
            let deletion_range_end =
                current_working_range.end - pattern.len()..current_working_range.end;

            // delete the text from the end first to avoid spoiling our indexes
            self.text.drain(deletion_range_end);
            self.text.drain(deletion_range_start);

            cursor_range.primary.index -= pattern.len();
            cursor_range.secondary.index -= pattern.len();
        } else {
            // add to the end first to avoid spoiling our indexes
            self.text.insert_str(current_working_range.end, pattern);
            self.text.insert_str(current_working_range.start, pattern);

            cursor_range.primary.index += pattern.len();
            cursor_range.secondary.index += pattern.len();
        }

        self.version += 1;
    }

    fn is_formatted_generic(&self, current_working_range: &Range<usize>, pattern: &str) -> bool {
        let working_range_len = current_working_range.end - current_working_range.start;

        // check for basic validity: if there are less than twice as many characters as the pattern, it can't
        // have a starting and ending token, so it can't be formatted
        if working_range_len < pattern.len() * 2 {
            return false;
        }

        match self.text.get(current_working_range.clone()) {
            Some(working_text) => {
                // check if we start end end with the pattern
                working_text.starts_with(pattern) && working_text.ends_with(pattern)
            }
            None => {
                log::error!("Encountered invalid index of text: {current_working_range:?}");
                false
            }
        }
    }

    fn is_italic(&self, current_working_range: &Range<usize>) -> bool {
        let working_range_len = current_working_range.end - current_working_range.start;

        // check for basic validity: if there are less than two characters, we can't have a starting
        // and ending token, so it can't be formatted
        if working_range_len < 2 {
            return false;
        }

        match self.text.get(current_working_range.clone()) {
            Some(working_text) => {
                // special case: we have exactly two characters and they're `*`:
                if working_text.len() == 2 && working_text == "**" {
                    return true;
                }

                // validate that we have `*` (italic) or `***` (bold and italic) but not `**` (just bold)
                let italic_start = working_text.starts_with("***")
                    || (working_text.starts_with('*') && !working_text.starts_with("**"));
                let italic_end = working_text.ends_with("***")
                    || (working_text.ends_with('*') && !working_text.ends_with("**"));
                italic_start && italic_end
            }
            None => {
                log::error!("Encountered invalid index of text: {current_working_range:?}");
                false
            }
        }
    }

    /// Gets the range of the current selection or word for hotkeys, filtering out punctuation that
    /// isn't `*` (useful for format hotkeys)
    fn get_selection_range_trimmed(&self, cursor_range: &CCursorRange) -> Range<usize> {
        let selection_range = self.get_selection_range(cursor_range);

        let selection = self.get(selection_range.clone()).unwrap();

        let start_trimmed_selection = selection.trim_start_matches(|chr: char| {
            (chr.is_ascii_punctuation() && chr != '*') || chr.is_whitespace()
        });
        let trimmed_selection = start_trimmed_selection.trim_end_matches(|chr: char| {
            (chr.is_ascii_punctuation() && chr != '*') || chr.is_whitespace()
        });

        let chars_trimmed_start = selection.len() - start_trimmed_selection.len();
        let chars_trimmed_end = start_trimmed_selection.len() - trimmed_selection.len();

        let trimmed_selection_start = selection_range.start + chars_trimmed_start;
        let trimmed_selection_end = selection_range.end - chars_trimmed_end;

        trimmed_selection_start..trimmed_selection_end
    }

    /// Get the range of the current selection as byte indexes in the text
    ///
    /// This does a bunch of separate calls to `get_current_word` which does a copy, we could reduce
    /// copies by making that take the char list as an argument, but we haven't bothered so far
    fn get_selection_range(&self, cursor_range: &CCursorRange) -> Range<usize> {
        let [primary, secondary] = cursor_range.sorted_cursors();

        // Simple case: no selection, just select the word
        if primary == secondary {
            return spellcheck::get_current_word(&self.text, primary.index);
        }

        let chars: Vec<_> = self.text.char_indices().collect();
        let mut starting_index = primary.index;
        let mut ending_index = secondary.index;

        let starting_text = &chars[primary.index..secondary.index];

        // if the selection is all whitespace, we should just return it
        if starting_text.iter().all(|pos| pos.1.is_whitespace()) {
            let starting_byte_index = chars[starting_index].0;
            let ending_byte_index = chars[ending_index].0;
            return starting_byte_index..ending_byte_index;
        }

        // clamp down on whitespace at beginning and ending
        while (chars[starting_index].1.is_whitespace()
            || (chars[starting_index].1.is_ascii_punctuation() && chars[starting_index].1 != '*'))
            && starting_index < ending_index
        {
            starting_index += 1;
        }

        while (chars[ending_index - 1].1.is_whitespace()
            || (chars[ending_index - 1].1.is_ascii_punctuation()
                && chars[ending_index - 1].1 != '*'))
            && starting_index <= ending_index
        {
            ending_index -= 1;
        }

        let starting_word = spellcheck::get_current_word(&self.text, starting_index);
        let ending_word = spellcheck::get_current_word(&self.text, ending_index);

        starting_word.start..ending_word.end
    }

    pub fn word_count(&self, ctx: &mut EditorContext) -> usize {
        let rdata = ctx.stores.text_box.get(&self.struct_uid);
        let text_box: &mut TextBox = &mut rdata.borrow_mut();

        text_box.refresh(self, ctx);
        text_box.word_count
    }

    /// Remove *all* smart quotes from text that was just pasted into. This could probably be made
    /// more efficient (e.g., we technically don't need to do this in a separate pass from formatting),
    /// but this works.
    fn clean_up_quotes(&mut self) {
        static SMART_QUOTE_REMOVAL_REGEX: SavedRegex =
            SavedRegex::new(|| Regex::new(r#"[“”‘’]"#).unwrap());

        // Iterate through the string backwards so we don't invalidate our own indexes
        for (replacement, replace_range) in SMART_QUOTE_REMOVAL_REGEX
            .find_iter(&self.text)
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .map(|quote_match| {
                let replacement = if quote_match.as_str() == "“" || quote_match.as_str() == "”"
                {
                    "\""
                } else {
                    "\'"
                };

                (replacement, quote_match.range())
            })
            .collect::<Vec<_>>()
        {
            // Replace the string text in place
            self.text.replace_range(replace_range, replacement);
        }
    }
}
