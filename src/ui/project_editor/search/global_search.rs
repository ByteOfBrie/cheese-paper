use egui::{Color32, Response};

use crate::ui::prelude::*;

use super::textbox_search::TextBoxSearchResult;

/// Global search ui, returns the search box response output
pub fn ui(ui: &mut Ui, project: &Project, ctx: &mut EditorContext) -> Response {
    let gs = &mut ctx.search;

    // Take up the entire area horizontally
    let search_box_response = ui.add_sized(
        egui::vec2(ui.available_width(), 0.0),
        egui::TextEdit::singleline(&mut gs.find_text)
            .hint_text("find")
            .return_key(None), // keep focus when Enter is pressed
    );

    gs.process_request_search_box_focus(ui, &search_box_response);

    // check if we need to search
    if ui.button("search").clicked()
        || (search_box_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
    {
        gs.redo_search = true;
    }

    if let Some(search_results) = &mut ctx.search.search_results {
        let mut items: Vec<(TextUID, String, &TextBoxSearchResult)> = search_results
            .iter()
            .filter_map(|(id, tbsr)| match &tbsr.page {
                Page::FileObject(tab_id) => {
                    let file_object_name = project.objects.get(tab_id)?.borrow().get_title();
                    Some((*id, file_object_name, tbsr))
                }
                Page::ProjectMetadata => Some((*id, String::from("Project Metadata"), tbsr)),
                Page::Export => unreachable!(),
                Page::Settings => unreachable!(),
                Page::Help => unreachable!(),
            })
            .filter(|(_, _, tbsr)| !tbsr.finds.is_empty())
            .collect();

        items.sort_by_key(|(_, file_object_name, tbsr)| (file_object_name.clone(), &tbsr.box_name));

        let mut file_object_id: Option<String> = None;

        for (id, file_object_name, tbsr) in items {
            if file_object_id.as_ref() != Some(&file_object_name) {
                file_object_id = Some(file_object_name.clone());
                ui.colored_label(Color32::LIGHT_GREEN, &file_object_name);
            }

            ui.colored_label(Color32::LIGHT_BLUE, &tbsr.box_name);

            for word_find in &tbsr.finds {
                if word_find.ui(ui).clicked() {
                    ctx.search.focus = Some((id, word_find.clone()));
                    ctx.search.goto_focus = true;

                    // trigger a formatting refresh
                    ctx.render_version += 1;
                }
            }
        }
    }

    search_box_response
}
