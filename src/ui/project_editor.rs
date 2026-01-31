pub mod action;
mod file_tree;
pub mod page;
pub mod search;
mod util;

use crate::ui::settings::ThemeSelection;
use crate::ui::{prelude::*, render_data};

use crate::components::file_objects::utils::process_name_for_filename;
use crate::ui::editor_base::EditorState;
use crate::ui::project_editor::search::global_search;
use crate::ui::project_tracker::ProjectTracker;

use action::Actions;

use std::collections::{BTreeMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::ops::Range;
use std::path::PathBuf;

use egui::{Key, Modifiers};
use egui_dock::{DockArea, DockState};
use egui_ltreeview::TreeViewState;
use rfd::FileDialog;
use spellbook::Dictionary;

#[derive(Debug, Default)]
pub struct SpellCheckStatus {
    pub selected_word: String,
    pub correct: bool,
    pub suggestions: Vec<String>,
    pub word_range: Range<usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypingStatus {
    pub is_new_word: bool,
    pub current_word: Range<usize>,
}

pub struct ProjectEditor {
    pub project: Project,

    /// List of tabs that are open (egui::Dock requires state to be stored this way)
    dock_state: DockState<OpenPage>,

    pub editor_context: EditorContext,

    tracker: Option<ProjectTracker>,

    /// We need to keep track of the tree state to set selection
    tree_state: TreeViewState<Page>,

    /// Set by the tab viewer, used to sync the file tree
    current_open_tab: Option<OpenPage>,
}

impl Debug for ProjectEditor {
    /// Manual implementation because TreeViewState doesn't implement debug
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectEditor")
            .field("project", &self.project)
            .field("dock_state", &self.dock_state)
            .field("editor_context", &self.editor_context)
            .field("tracker", &self.tracker)
            .finish()
    }
}

#[derive(Debug)]
pub struct DictionaryState {
    pub dictionary: Option<Dictionary>,
    _fresh_dictionary: Option<Dictionary>,
    /// The words that are explictly ignored, are stored in the data toml file
    ignored_words: HashSet<String>,
    characters_and_places: HashSet<String>,
    old_characters_and_places: HashSet<String>,
    added_file_object_names: HashSet<String>,
    pub ignore_list_updated: bool,
}

impl DictionaryState {
    pub fn new(dict: Option<Dictionary>) -> Self {
        Self {
            dictionary: dict.clone(),
            _fresh_dictionary: dict,
            ignored_words: HashSet::new(),
            characters_and_places: HashSet::new(),
            old_characters_and_places: HashSet::new(),
            added_file_object_names: HashSet::new(),
            ignore_list_updated: false,
        }
    }

    pub fn add_ignored(&mut self, ignored_word: &str) -> bool {
        if self.add_ignored_startup(ignored_word) {
            self.ignore_list_updated = true;
            true
        } else {
            false
        }
    }

    // Version of add_ignored that doesn't count as an update, should be used for words being added
    // at startup or the like
    pub fn add_ignored_startup(&mut self, ignored_word: &str) -> bool {
        if let Some(dictionary) = &mut self.dictionary
            && !dictionary.check(ignored_word)
        {
            if let Err(err) = dictionary.add(ignored_word) {
                log::error!("Could not add word {ignored_word} to dictionary: {err}");
            };
            self.ignored_words.insert(ignored_word.to_string());
            true
        } else {
            false
        }
    }

    pub fn get_ignore_list(&self) -> HashSet<String> {
        self.ignored_words.clone()
    }

    pub fn add_file_object_name(&mut self, object_name: impl AsRef<str>) {
        if let Some(dictionary) = &mut self.dictionary {
            for part in object_name.as_ref().split(&[' ', '/'][..]) {
                let trimmed = part.trim_matches('"');

                if self.old_characters_and_places.contains(trimmed) || !dictionary.check(trimmed) {
                    self.characters_and_places.insert(trimmed.to_string());
                }
            }
        }
    }

    pub fn resync_file_names(&mut self) {
        let names_to_remove = self
            .added_file_object_names
            .difference(&self.characters_and_places);

        if names_to_remove.count() != 0 {
            // we have words added to the dictionary that need to be removed now, the only way we can do
            // this is to freshly clone the dictionary and add everything back
            self.dictionary = self._fresh_dictionary.clone();

            if let Some(dictionary) = &mut self.dictionary {
                for word in &self.ignored_words {
                    if !dictionary.check(word)
                        && let Err(err) = dictionary.add(word)
                    {
                        log::error!(
                            "Could not add already ignored word {word} to dictionary: {err}"
                        );
                    }
                }
            }
            self.added_file_object_names.clear();
        }

        if let Some(dictionary) = &mut self.dictionary {
            let names_to_add: Vec<_> = self
                .characters_and_places
                .difference(&self.added_file_object_names)
                .cloned()
                .collect();

            for file_object_word in names_to_add {
                if !dictionary.check(&file_object_word) {
                    match dictionary.add(&file_object_word) {
                        Ok(()) => {
                            self.added_file_object_names.insert(file_object_word);
                        }
                        Err(err) => {
                            log::error!(
                                "Could not add word {file_object_word} from file object to dictionary: {err}"
                            );
                        }
                    };
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct References {
    pub file_types: &'static [FileType],
    pub r: HashMap<FileType, BTreeMap<FileID, String>>,
}

impl References {
    pub fn new(project: &Project) -> Self {
        let mut references = Self {
            file_types: project.schema.get_all_file_types(),
            r: HashMap::new(),
        };

        for file_type in references.file_types {
            references.r.insert(file_type, BTreeMap::new());
        }

        references.update(&project.objects);

        references
    }

    pub fn for_type(&self, file_type: FileType) -> &BTreeMap<FileID, String> {
        self.r.get(&file_type).expect("FileType should exist")
    }

    /// Populate the list of references based on the objects, complete with names (for use in UI)
    pub fn update(&mut self, objects: &FileObjectStore) {
        // Eve note: I'm pretty sure that these shenanigans have a higher performance cost than
        // the malloc you're trying to avoid with them. I will however leave them here, out of respect for the craft

        let mut old_refs = std::mem::take(&mut self.r);
        for file_type in self.file_types {
            self.r.insert(file_type, BTreeMap::new());
        }

        for file_object in objects.values() {
            let object_borrowed = file_object.borrow();
            if let Some(old_name) = old_refs
                .get_mut(object_borrowed.get_type())
                .unwrap()
                .remove(object_borrowed.id())
                && object_borrowed.get_base().metadata.name == old_name
            {
                self.r
                    .get_mut(object_borrowed.get_type())
                    .unwrap()
                    .insert(object_borrowed.id().clone(), old_name);
            } else {
                self.r
                    .get_mut(object_borrowed.get_type())
                    .unwrap()
                    .insert(object_borrowed.id().clone(), object_borrowed.get_title());
            }
        }
    }
}

#[derive(Debug)]
pub struct EditorContext {
    pub settings: Settings,
    pub dictionary_state: DictionaryState,
    pub spellcheck_status: SpellCheckStatus,
    pub typing_status: TypingStatus,
    pub search: Search,
    pub stores: Stores,
    pub references: References,
    pub actions: Actions,

    /// Duplicates the value from state.data, which is then more recent
    pub last_export_folder: PathBuf,

    /// version number. increment to trigger a project-wide formatting refresh
    pub version: usize,
}

#[derive(Debug, Default)]
pub struct Stores {
    pub text_box: crate::ui::elements::text_box::Store,
    pub page: page::Store,
    pub file_objects: render_data::FileObjectRDStore,
}

pub enum TabMove {
    Previous,
    Next,
}

pub struct TabViewer<'a> {
    pub project: &'a mut Project,
    pub editor_context: &'a mut EditorContext,
    pub open_tab: &'a mut Option<OpenPage>,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = OpenPage;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        tab.into()
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title(self.project)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        // Tell the editor which tab we have open (so that the treeview selection can be updated)
        if self.open_tab.as_ref() != Some(tab) {
            *self.open_tab = Some(tab.clone());
        }

        // check for ctrl-shift-f for search
        if ui.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers::CTRL | Modifiers::SHIFT,
                logical_key: Key::F,
            })
        }) {
            self.editor_context.search.show();
        }

        // draw the actual UI for the tab open in the editor
        tab.ui(ui, self.project, self.editor_context);
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &egui::Response) {
        if response.double_clicked() {
            let page = tab.page.clone();
            self.editor_context
                .actions
                .schedule(move |project_editor, _ctx| project_editor.keep_editor_tab(&page));
        }
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        // disable moving tabs into windows (untested, could maybe be supported later)
        false
    }
}

/// Update the title of the project
fn update_title(project_name: &str, ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
        "Cheese Paper - {project_name}",
    )));
}

impl ProjectEditor {
    pub fn panels(&mut self, ctx: &egui::Context, state: &mut EditorState) {
        self.process_input(ctx);
        self.process_state(ctx);

        self.draw_menu(ctx, state);

        egui::SidePanel::left("project tree panel").show(ctx, |ui| {
            self.side_panel(ui);
        });

        // Before rendering the tab view, clear out any deleted scenes
        self.dock_state.retain_tabs(|tab| match &tab.page {
            Page::ProjectMetadata => true,
            Page::Export => true,
            Page::Settings => true,
            Page::FileObject(tab_id) => self.project.objects.contains_key(tab_id),
        });

        // render the tab view
        DockArea::new(&mut self.dock_state)
            .allowed_splits(egui_dock::AllowedSplits::None)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(
                ctx,
                &mut TabViewer {
                    project: &mut self.project,
                    editor_context: &mut self.editor_context,
                    open_tab: &mut self.current_open_tab,
                },
            );

        // If there aren't any tabs open, reflect that state
        if self.dock_state.iter_all_tabs().next().is_none() {
            self.current_open_tab = None
        }

        if let Some(open_tab) = &self.current_open_tab
            && self
                .tree_state
                .selected()
                .first()
                .is_none_or(|page| page != &open_tab.page)
        {
            self.tree_state.set_one_selected(open_tab.page.clone());
        }
    }

    /// Get input that the project editor itself will read (hotkeys to switch or close tabs)
    fn process_input(&mut self, ctx: &egui::Context) {
        // close current tab if ctrl-w is pressed
        if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers::CTRL,
                logical_key: Key::W,
            })
        }) && let Some((_, current_tab_ref)) = self.dock_state.find_active_focused()
        {
            // We get an &mut reference so we have to clone it ;)
            let current_tab = current_tab_ref.clone();
            let tab_position = self.dock_state.find_tab(&current_tab).unwrap();
            self.dock_state.remove_tab(tab_position);
        }

        // Move between tabs (ctrl-tab or ctrl-shift-tab)
        let tab_move_option = if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers::CTRL | Modifiers::SHIFT,
                logical_key: Key::Tab,
            })
        }) {
            // ctrl-shift-tab was pressed, move backwards
            Some(TabMove::Previous)
        } else if ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers::CTRL,
                logical_key: Key::Tab,
            })
        }) {
            // ctrl-tab was pressed, move fowards
            Some(TabMove::Next)
        } else {
            None
        };

        if let Some(tab_move) = tab_move_option
            && let Some(new_tab) = self.move_tab(tab_move)
        {
            new_tab.rerequest_focus(&mut self.editor_context)
        }
    }

    fn move_tab(&mut self, tab_move: TabMove) -> Option<OpenPage> {
        // We could probably get around this by learning how dock_state works better, but
        // this is easy and reliable
        // Eve note: nah this is probably the best way actually. egui_dock doesn't expose the logic
        // for "next tab" and "previous tab" except in the iterator function
        let open_tabs: Vec<_> = self.get_open_tabs();

        // Make sure we have something to do
        if open_tabs.len() > 1
            && let Some((_, current_tab)) = self.dock_state.find_active_focused()
        {
            let current_pos = open_tabs
                .iter()
                .position(|val| val == current_tab)
                .expect("focused tab should be in list of tabs");

            let new_pos = match tab_move {
                TabMove::Next => (current_pos + 1) % open_tabs.len(),
                TabMove::Previous => current_pos
                    .checked_sub(1)
                    .unwrap_or_else(|| open_tabs.len() - 1),
            };

            let new_tab_page = open_tabs.get(new_pos).unwrap();

            let new_tab_index = self.dock_state.find_tab(new_tab_page).unwrap();
            self.dock_state.set_active_tab(new_tab_index);

            Some(new_tab_page.clone())
        } else {
            None
        }
    }

    fn draw_menu(&mut self, ctx: &egui::Context, state: &mut EditorState) {
        egui::TopBottomPanel::top("menu_bar_panel")
            .show_separator_line(false)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Close Project").clicked() {
                            state.closing_project = true;
                        }

                        ui.menu_button("Recent Projects", |ui| {
                            for project in state.data.recent_projects.iter() {
                                if ui.button(project.to_string_lossy()).clicked() {
                                    state.closing_project = true;
                                    state.next_project = Some(project.clone());
                                }
                            }
                        });

                        if ui.button("Export Story Text").clicked() {
                            self.set_editor_tab(&Page::Export, true);
                        }

                        if ui.button("Export Outline").clicked() {
                            let project_title = &self.project.base_metadata.name;
                            let suggested_title =
                                format!("{}_outline.md", process_name_for_filename(project_title));
                            let export_location_option = FileDialog::new()
                                .set_title(format!("Export {project_title} Outline"))
                                .set_directory(&state.data.last_export_folder)
                                .set_file_name(suggested_title)
                                .save_file();

                            if let Some(export_location) = export_location_option {
                                let outline_contents = self.project.export_outline();
                                if let Err(err) = std::fs::write(&export_location, outline_contents)
                                {
                                    log::error!("Error while attempting to write outline: {err}");
                                }

                                state.data.last_export_folder = export_location
                                    .parent()
                                    .map(|val| val.to_path_buf())
                                    .unwrap_or_default();
                            }
                        }

                        if ui.button("Quit").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.menu_button("Edit", |ui| {
                        if ui.button("Find (Global)").clicked() {
                            self.editor_context.search.show();
                        }

                        if ui.button("Settings").clicked() {
                            self.set_editor_tab(&Page::Settings, true);
                        }

                        if ui.button("Randomize Theme").clicked() {
                            self.editor_context
                                .settings
                                .select_theme(ThemeSelection::Random)
                                .unwrap();
                            self.editor_context.actions.schedule(|project_editor, ctx| {
                                project_editor.update_theme(ctx);
                            });
                        }
                    });

                    ui.menu_button("View", |ui| {
                        if ui.button("Close All Tabs").clicked() {
                            self.close_all_tabs();
                        }
                    });
                });
            });
    }

    // the side panel containing the tree view or the global search
    fn side_panel(&mut self, ui: &mut egui::Ui) {
        if self.editor_context.search.active {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.editor_context.search.hide();
            }
            egui::ScrollArea::vertical()
                .id_salt("search scroll")
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    global_search::ui(ui, &self.project, &mut self.editor_context);
                });
        } else {
            egui::ScrollArea::both()
                .id_salt("tree scroll")
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    file_tree::ui(self, ui);
                });
        }
    }

    pub fn update_theme(&self, ctx: &egui::Context) {
        ctx.style_mut(|style| {
            self.editor_context.settings.theme().apply(style);
        });
    }

    fn process_state(&mut self, ctx: &egui::Context) {
        if self.editor_context.search.exiting_search {
            self.editor_context.version += 1;
        }

        // This is kinda dumb but will ensure that the names are always up to date
        // We can always optimize this later if needed
        self.editor_context.references.update(&self.project.objects);

        self.project.receive_updates();

        // automatically track progress if we have a tracker
        if let Some(tracker) = &mut self.tracker
            && tracker.snapshot_time.elapsed().as_secs() >= 60 * 15
            && let Err(err) = tracker.snapshot("Autosave")
        {
            log::warn!("Failed to track changes: {err}");
        }

        if self.editor_context.search.redo_search {
            self.editor_context.search.redo_search = false;
            self.search();
        }

        // if one of the search results has been clicked, open that now
        // BY THE POWER OF IF LET CHAINS
        if self.editor_context.search.goto_focus
            && let Some((uid, _word_find)) = &self.editor_context.search.focus.as_ref()
            && let Some(search_results) = &self.editor_context.search.search_results.as_ref()
            && let Some(focused_text_box) = search_results.get(uid)
        {
            self.set_editor_tab(&focused_text_box.page.clone(), false);
        }

        let actions = self.editor_context.actions.get();
        for action in actions {
            action(self, ctx);
        }
    }

    fn set_editor_tab(&mut self, page: &Page, keep: bool) {
        // We don't want to open these, so just exit early
        if let Page::FileObject(id) = page
            && self.project.is_top_level_folder(id)
        {
            return;
        }

        if let Some(tab_position) = self
            .dock_state
            .find_tab_from(|open_tab| &open_tab.page == page)
        {
            // We've already opened this, just select it
            self.dock_state.set_active_tab(tab_position);
        } else {
            if let Some(tab_position) = self.dock_state.find_tab_from(|tab| !tab.keep) {
                // there's a tab open in browsing mode, close it
                self.dock_state.remove_tab(tab_position);
            }
            // New file object, open it for editing
            self.dock_state.push_to_first_leaf(page.clone().open(keep));
        }
    }

    /// set an editor tab to edit mode, indicating it should be kept
    fn keep_editor_tab(&mut self, page: &Page) {
        for (_, tab) in self.dock_state.iter_all_tabs_mut() {
            if &tab.page == page {
                tab.keep = true;
            }
        }
    }

    fn close_all_tabs(&mut self) {
        self.dock_state.retain_tabs(|_| false);
    }

    // last_export_folder probably should be wrapped in another object but I don't have a good object
    // to wrap it in, so it's here for now
    pub fn new(
        project: Project,
        open_tab_ids: Vec<String>,
        dictionary: Option<Dictionary>,
        settings: Settings,
        last_export_folder: PathBuf,
        ignored_words: impl IntoIterator<Item: AsRef<str>>,
    ) -> Self {
        let tracker = match ProjectTracker::new(&project.get_path()) {
            Ok(mut tracker) => {
                if let Err(err) = tracker.snapshot("Startup") {
                    log::warn!("Failed to snapshot tracker: {err}");
                };
                Some(tracker)
            }
            Err(err) => {
                log::warn!("failed to create project tracker: {err}");
                None
            }
        };

        // Spellbook docs say not to do this in the UI thread
        let mut dictionary_state = DictionaryState::new(dictionary);

        for ignored_word in ignored_words.into_iter() {
            dictionary_state.add_ignored(ignored_word.as_ref());
        }

        let open_tabs = open_tab_ids
            .iter()
            .map(|tab_id| Page::from_id(tab_id).open(true))
            .collect();

        let references = References::new(&project);

        let mut actions = Actions::default();

        actions.schedule(|editor, ctx| update_title(&editor.project.base_metadata.name, ctx));

        let mut project_editor = Self {
            project,
            dock_state: DockState::new(open_tabs),
            editor_context: EditorContext {
                settings,
                dictionary_state,
                spellcheck_status: SpellCheckStatus::default(),
                typing_status: TypingStatus::default(),
                search: Search::default(),
                stores: Stores::default(),
                actions,
                references,
                last_export_folder,
                version: 0,
            },
            tracker,
            tree_state: Default::default(),
            current_open_tab: None,
        };

        project_editor.update_spellcheck_file_object_names();
        project_editor
            .editor_context
            .dictionary_state
            .resync_file_names();
        project_editor.editor_context.version += 1;

        project_editor
    }

    pub fn update_spellcheck_file_object_names(&mut self) {
        // first, reset the listing of characters and places
        self.editor_context
            .dictionary_state
            .old_characters_and_places =
            std::mem::take(&mut self.editor_context.dictionary_state.characters_and_places);

        for object in self.project.objects.values() {
            for item in object.borrow().as_editor().provide_spellcheck_additions() {
                self.editor_context
                    .dictionary_state
                    .add_file_object_name(item);
            }
        }
    }

    pub fn get_open_tabs(&self) -> Vec<OpenPage> {
        // the indexes provided to use are meaningless (I think), just put all the tabs in the
        // order it gave us.
        self.dock_state
            .iter_all_tabs()
            .map(|((_, _), tab)| tab.clone())
            .collect()
    }

    /// Process any queued events and then do the actual save
    pub fn save(&mut self) {
        self.project.process_updates();
        if let Err(err) = self.project.save() {
            log::error!("encountered error while saving project: {err}");
        }
    }
}
