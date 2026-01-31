use crate::ui::prelude::*;
use crate::ui::{
    elements::text_box::spellcheck::find_misspelled_words,
    project_editor::search::textbox_search::{TextBoxSearchResult, WordFind},
};

use egui::{Color32, Stroke, TextFormat, text::LayoutJob};

#[derive(Debug, Clone, Copy)]
enum StyleOption {
    Strong,
    Italic,
    Misspelled,
    NewLine,
    SearchHighlight,
    SearchHighlightFocus,
    None,
}

#[derive(Debug, Clone, Copy)]
struct StyleMarker {
    idx: usize,
    style: StyleOption,
    on: bool,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct Style {
    strong: bool,
    italic: bool,
    misspelled: bool,
    search_highlight: bool,
    search_highlight_focus: bool,
    newline: bool,
}

impl Style {
    fn update(&mut self, marker: &StyleMarker) {
        match marker.style {
            StyleOption::Strong => self.strong = marker.on,
            StyleOption::Italic => self.italic = marker.on,
            StyleOption::Misspelled => self.misspelled = marker.on,
            StyleOption::NewLine => self.newline = marker.on,
            StyleOption::SearchHighlight => self.search_highlight = marker.on,
            StyleOption::SearchHighlightFocus => self.search_highlight_focus = marker.on,
            _ => (),
        }
    }
}

fn format_from_style(egui_style: &egui::Style, text_style: &Style) -> egui::text::TextFormat {
    let Style {
        strong,
        italic: italics,
        misspelled,
        search_highlight,
        search_highlight_focus,
        newline: _newline,
    } = *text_style;

    let font_id = egui_style
        .text_styles
        .get(&egui::TextStyle::Body)
        .unwrap()
        .clone();

    let mut format = TextFormat {
        font_id,
        italics,
        ..Default::default()
    };

    if strong {
        format.color = egui_style.visuals.strong_text_color()
    } else {
        format.color = egui_style.visuals.text_color()
    };

    if misspelled {
        format.underline = Stroke {
            width: 2.0,
            color: egui_style.visuals.error_fg_color,
        }
    }

    if search_highlight {
        format.background = Color32::YELLOW;
    }

    if search_highlight_focus {
        format.background = Color32::ORANGE;
    }

    format
}

// format rules

fn format_rule_bold_italic(
    text: &str,
    _ctx: &EditorContext,
) -> (Vec<StyleMarker>, Vec<StyleMarker>) {
    let mut bold = Vec::new();
    let mut italic = Vec::new();

    static ASTERIX_GROUPS: SavedRegex = SavedRegex::new(|| Regex::new(r#"(\*+|\n)"#).unwrap());

    let mut italic_start = None;
    let mut bold_start = None;

    for ag in ASTERIX_GROUPS.captures_iter(text) {
        let ag = ag.get(0).unwrap();

        match ag.as_str() {
            "*" => {
                if let Some(start) = italic_start {
                    italic.push(StyleMarker {
                        idx: start,
                        style: StyleOption::Italic,
                        on: true,
                    });
                    italic.push(StyleMarker {
                        idx: ag.end(),
                        style: StyleOption::Italic,
                        on: false,
                    });
                    italic_start = None;
                } else {
                    italic_start = Some(ag.start());
                }
            }
            "**" => {
                if let Some(start) = bold_start {
                    bold.push(StyleMarker {
                        idx: start,
                        style: StyleOption::Strong,
                        on: true,
                    });
                    bold.push(StyleMarker {
                        idx: ag.end(),
                        style: StyleOption::Strong,
                        on: false,
                    });
                    bold_start = None;
                } else {
                    bold_start = Some(ag.start());
                }
            }
            "***" => {
                // Toggle both bold and italic
                // This might not be correct but it's better than nothing, TBD:
                if let Some(start) = bold_start {
                    bold.push(StyleMarker {
                        idx: start,
                        style: StyleOption::Strong,
                        on: true,
                    });
                    bold.push(StyleMarker {
                        idx: ag.end(),
                        style: StyleOption::Strong,
                        on: false,
                    });
                    bold_start = None;
                } else {
                    bold_start = Some(ag.start());
                }
                if let Some(start) = italic_start {
                    italic.push(StyleMarker {
                        idx: start,
                        style: StyleOption::Italic,
                        on: true,
                    });
                    italic.push(StyleMarker {
                        idx: ag.end(),
                        style: StyleOption::Italic,
                        on: false,
                    });
                    italic_start = None;
                } else {
                    italic_start = Some(ag.start());
                }
            }
            "\n" => {
                // We just want to reset to nothing at the start of the line
                italic_start = None;
                bold_start = None;
            }
            _ => (),
        }
    }

    (bold, italic)
}

fn format_rule_newlines(text: &str, _ctx: &EditorContext) -> Vec<StyleMarker> {
    let mut res = vec![StyleMarker {
        idx: 0,
        style: StyleOption::NewLine,
        on: true,
    }];

    for (idx, _) in text.match_indices('\n') {
        res.push(StyleMarker {
            idx: (idx + 1),
            style: StyleOption::NewLine,
            on: true,
        })
    }

    res
}

fn format_rule_spellcheck(text: &str, ctx: &EditorContext) -> Vec<StyleMarker> {
    find_misspelled_words(text, ctx)
        .into_iter()
        .flat_map(|(start, end)| {
            [
                StyleMarker {
                    idx: start,
                    style: StyleOption::Misspelled,
                    on: true,
                },
                StyleMarker {
                    idx: end,
                    style: StyleOption::Misspelled,
                    on: false,
                },
            ]
        })
        .collect()
}

fn format_rule_search(_text: &str, search_result: &TextBoxSearchResult) -> Vec<StyleMarker> {
    let mut res = Vec::new();

    for word_find in &search_result.finds {
        res.push(StyleMarker {
            idx: word_find.start,
            style: StyleOption::SearchHighlight,
            on: true,
        });
        res.push(StyleMarker {
            idx: word_find.end,
            style: StyleOption::SearchHighlight,
            on: false,
        });
    }

    res
}

fn format_rule_search_focus(_text: &str, word_find: &WordFind) -> Vec<StyleMarker> {
    vec![
        StyleMarker {
            idx: word_find.start,
            style: StyleOption::SearchHighlightFocus,
            on: true,
        },
        StyleMarker {
            idx: word_find.end,
            style: StyleOption::SearchHighlightFocus,
            on: false,
        },
    ]
}

// end format rules

pub fn compute_layout_job(
    text: &str,
    ctx: &EditorContext,
    search_result: Option<&TextBoxSearchResult>,
    search_result_focus: Option<&WordFind>,
    egui_style: &egui::Style,
) -> LayoutJob {
    let mut applied_rules = Vec::with_capacity(5);

    let (bold, italic) = format_rule_bold_italic(text, ctx);
    applied_rules.push(bold);
    applied_rules.push(italic);
    applied_rules.push(format_rule_newlines(text, ctx));
    applied_rules.push(format_rule_spellcheck(text, ctx));
    if let Some(search_result) = search_result {
        applied_rules.push(format_rule_search(text, search_result));
    }
    if let Some(word_find) = search_result_focus {
        applied_rules.push(format_rule_search_focus(text, word_find));
    }

    let mut styles = vec_merge(applied_rules);
    styles.push(StyleMarker {
        idx: text.len(),
        style: StyleOption::None,
        on: false,
    });

    let mut job = LayoutJob::default();
    let mut text_style = Style::default();
    let mut start = 0;

    for marker in styles {
        let end = marker.idx;
        debug_assert!(end >= start);
        debug_assert!(end <= text.len());

        if end > start {
            let leading_space = if text_style.newline && ctx.settings.indent_line_start() {
                20.0
            } else {
                0.0
            };

            job.append(
                &text[start..end],
                leading_space,
                format_from_style(egui_style, &text_style),
            );
            text_style.newline = false;

            start = end;
        }

        text_style.update(&marker);
    }

    debug_assert!(start == text.len());

    job
}

fn vec_merge(formats: Vec<Vec<StyleMarker>>) -> Vec<StyleMarker> {
    let mut res = Vec::new();
    let mut iters: Vec<_> = formats
        .into_iter()
        .map(|v| v.into_iter().peekable())
        .collect();

    loop {
        let mut next: Option<(StyleMarker, usize)> = None;
        for (idx, it) in iters.iter_mut().enumerate() {
            match (next, it.peek()) {
                (None, Some(v)) => {
                    next = Some((*v, idx));
                }
                (Some((v0, _)), Some(v1)) if v1.idx < v0.idx => {
                    next = Some((*v1, idx));
                }
                _ => (),
            }
        }
        if let Some((v, idx)) = next {
            iters[idx].next();
            res.push(v);
        } else {
            return res;
        }
    }
}
