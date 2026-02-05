mod export_selection;
pub mod file_object_editor;
mod project_metadata_editor;

use crate::ui::prelude::*;

use crate::ui::settings::settings_page::SettingsPage;
pub use file_object_editor::FileObjectEditor;

use egui::{Id, Key, Modifiers};

/// An identifier for something that can be drawn as a tab
///
/// We currently have to have a string representation for every tab value so that
/// `update_open_tabs` can write the list of open tabs for them to be reopened next
/// time. If that requirement isn't present, we should be able to avoid having strings
/// entirely
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub enum Page {
    ProjectMetadata,
    FileObject(FileID),
    Settings,
    Export,
}

impl Page {
    const PROJECT_METADATA_ID: &str = "project_metadata";
    const EXPORT_ID: &str = "export";
    const SETTINGS_ID: &str = "settings";

    /// Get an id from a string. This (and its reverse, `get_id`) could be replaced by `From`
    /// (and `Into`), but this seems like it might be more explicit?
    pub fn from_id(id: &str) -> Self {
        match id {
            Self::PROJECT_METADATA_ID => Self::ProjectMetadata,
            Self::EXPORT_ID => Self::Export,
            _ => Self::FileObject(FileID::new(id.to_owned())),
        }
    }

    pub fn get_id(&self) -> &str {
        match self {
            Self::ProjectMetadata => Self::PROJECT_METADATA_ID,
            Self::Export => Self::EXPORT_ID,
            Self::Settings => Self::SETTINGS_ID,
            Self::FileObject(id) => id,
        }
    }

    pub fn from_file_id(file_id: &FileID) -> Self {
        Self::FileObject(file_id.clone())
    }

    pub fn is_file_object(&self) -> bool {
        matches!(self, Self::FileObject(_))
    }

    pub fn is_searchable(&self) -> bool {
        match self {
            Self::Export => false,
            Self::Settings => false,
            Self::FileObject(_) => true,
            Self::ProjectMetadata => true,
        }
    }

    pub fn open(self, keep: bool) -> OpenPage {
        OpenPage { page: self, keep }
    }
}

/// the identifier for a Page which has been open in a Tab
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenPage {
    pub page: Page,

    /// indicate if the page should be kept. if not, it will be closed when a new tab is opened.
    pub keep: bool,
}

#[derive(Debug, Default)]
pub struct PageData {
    search: Search,
    last_selected_id: Option<Id>,

    settings_page: Option<SettingsPage>,
}

pub type Store = RenderDataStore<Page, PageData>;

#[derive(Debug)]
enum FocusShiftDirection {
    Next,
    Previous,
    /// We need to do this sometimes when tabs have changed, but we want to remain on the same UI element
    Same,
}

const MAX_TITLE_LENGTH: usize = 20;

impl OpenPage {
    pub fn title(&self, project: &mut Project) -> egui::WidgetText {
        let text: egui::RichText = match &self.page {
            Page::ProjectMetadata => "Project Metadata".into(),
            Page::FileObject(file_id) => {
                if let Some(object) = project.objects.get(file_id) {
                    let text = object.borrow().get_title();
                    text.chars()
                        .take(MAX_TITLE_LENGTH)
                        .collect::<String>()
                        .into()
                } else {
                    // any deleted scenes should be cleaned up before we get here, but we have this
                    // logic instead of panicking anyway
                    "<Deleted>".into()
                }
            }
            Page::Export => "Export".into(),
            Page::Settings => "Settings".into(),
        };

        let text = if self.keep { text } else { text.italics() };

        text.into()
    }

    pub fn ui(&self, ui: &mut Ui, project: &mut Project, ctx: &mut EditorContext) {
        let rdata = ctx.stores.page.get(&self.page);
        let page_data: &mut PageData = &mut rdata.borrow_mut();

        let focus_shift_option = if ui.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab)) {
            Some(FocusShiftDirection::Previous)
        } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            Some(FocusShiftDirection::Next)
        } else if ctx.focus_jumper.recieve(self.clone()) {
            Some(FocusShiftDirection::Same)
        } else {
            None
        };

        let page_search_active = if self.page.is_searchable() {
            self.process_page_search(page_data, ui, project, ctx)
        } else {
            false
        };

        // Draw the UI, saving the ids of the (selectable) elements to do tabbing on
        let page_tabable_ids = match &self.page {
            Page::ProjectMetadata => project.metadata_ui(ui, ctx),
            Page::FileObject(file_object_id) => {
                if let Some(file_object) = project.objects.get(file_object_id) {
                    file_object.borrow_mut().as_editor_mut().ui(ui, ctx)
                } else {
                    Vec::new()
                }
            }
            Page::Export => project.export_ui(ui, ctx),
            Page::Settings => {
                if page_data.settings_page.is_none() {
                    page_data.settings_page = Some(SettingsPage::load(ctx));
                }
                let settings_page = page_data.settings_page.as_mut().unwrap();
                settings_page.ui(ui, ctx)
            }
        };

        if let Some(focus_shift) = focus_shift_option {
            let current_element_index = if let Some(last_id) = page_data.last_selected_id {
                page_tabable_ids.iter().position(|&tab| tab == last_id)
            } else {
                None
            };

            let next_element = match focus_shift {
                FocusShiftDirection::Next => {
                    if let Some(current_index) = current_element_index {
                        if let Some(element) = page_tabable_ids.get(current_index + 1) {
                            element
                        } else {
                            page_tabable_ids.first().unwrap()
                        }
                    } else {
                        page_tabable_ids.first().unwrap()
                    }
                }
                FocusShiftDirection::Previous => {
                    if let Some(current_index) = current_element_index {
                        if let Some(new_index) = current_index.checked_sub(1) {
                            if let Some(element) = page_tabable_ids.get(new_index) {
                                element
                            } else {
                                page_tabable_ids.last().unwrap()
                            }
                        } else {
                            page_tabable_ids.last().unwrap()
                        }
                    } else {
                        page_tabable_ids.last().unwrap()
                    }
                }
                FocusShiftDirection::Same => {
                    if let Some(last_id) = &page_data.last_selected_id {
                        last_id
                    } else {
                        page_tabable_ids.first().unwrap()
                    }
                }
            };

            ui.memory_mut(|mem| mem.request_focus(*next_element));
        }

        // Update the currently selected element if we need to do that
        if let Some(focused) = ui.memory(|i| i.focused())
            && Some(focused) != page_data.last_selected_id
            && page_tabable_ids.contains(&focused)
        {
            page_data.last_selected_id = Some(focused);
        }

        // If this was swapped once, we need to put it back
        if page_search_active {
            std::mem::swap(&mut ctx.search, &mut page_data.search);
        }
    }

    /// Handle page search logic, including swapping the page search memory once (but not swapping it)
    /// back, which should be done by the calling function
    fn process_page_search(
        &self,
        page_data: &mut PageData,
        ui: &mut Ui,
        project: &mut Project,
        ctx: &mut EditorContext,
    ) -> bool {
        // check for ctrl-f for page search
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers::CTRL,
                logical_key: Key::F,
            })
        }) {
            page_data.search.show();
            page_data.search.redo_search = true;
        }

        if page_data.search.active {
            ui.horizontal(|ui| {
                let search_box_response = ui.add(
                    egui::TextEdit::singleline(&mut page_data.search.find_text)
                        .hint_text("find")
                        .return_key(None), // keep focus when Enter is pressed)
                );

                page_data
                    .search
                    .process_request_search_box_focus(ui, &search_box_response);

                if search_box_response.changed() {
                    page_data.search.redo_search = true;
                }

                if ui.button("close").clicked()
                    || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                {
                    page_data.search.active = false;
                    ctx.version += 1;
                }
            });

            if page_data.search.redo_search {
                page_data.search.search_results = Some(HashMap::new());

                if let Some(searchable) = project.get_searchable(&self.page) {
                    searchable.search(&self.page, &mut page_data.search);
                }

                ctx.version += 1;
                page_data.search.redo_search = false;
            }
        }

        // For now, let global search have priority over page search, so only swap in the page search
        // memory if global search isn't active
        if page_data.search.active && !ctx.search.active {
            /* Hack-y solution: swap in the file search object for the file-local search */
            std::mem::swap(&mut ctx.search, &mut page_data.search);
            true
        } else {
            false
        }
    }
}

// Needs to be &mut Tab since `egui_dock::TabViewer::id` gives us a mut reference
impl From<&mut OpenPage> for egui::Id {
    fn from(val: &mut OpenPage) -> Self {
        egui::Id::new(val)
    }
}

impl From<Rc<String>> for Page {
    fn from(id: Rc<String>) -> Self {
        match id.as_str() {
            Self::PROJECT_METADATA_ID => Self::ProjectMetadata,
            Self::EXPORT_ID => Self::Export,
            _ => Self::FileObject(id),
        }
    }
}
