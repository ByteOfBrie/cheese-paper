use super::ProjectEditor;

use crate::{
    components::{
        file_objects::utils::process_name_for_filename,
        project::{ExportDepth, ExportOptions},
    },
    ui::prelude::*,
};

use egui_ltreeview::{Action, DirPosition, NodeBuilder, TreeView};
use rfd::FileDialog;

/// Temporary solution. Point to the schema statically here.
/// Eventually, a solution for loading the schema when opening the project will be needed
const SCHEMA: &'static dyn Schema = &crate::schemas::DEFAULT_SCHEMA;

/// Context menu actions for file objects, should only be constructed by file objects
enum ContextMenuActions {
    Delete {
        parent: FileID,
        deleting: FileID,
    },
    Add {
        parent: FileID,
        position: DirPosition<FileID>,
        file_type: FileType,
    },
    Export {
        id: FileID,
    },
}

impl dyn FileObject {
    fn build_tree(
        &self,
        objects: &FileObjectStore,
        builder: &mut egui_ltreeview::TreeViewBuilder<'_, Page>,
        actions: &mut Vec<ContextMenuActions>,
        parent_id: Option<FileID>,
        node_height: f32,
    ) {
        let node_name = if self.get_base().metadata.name.is_empty() {
            self.empty_string_name().to_string()
        } else {
            self.get_base().metadata.name.clone()
        };

        // first, construct the node. we avoid a lot of duplication by putting it into a variable
        // before sticking it in the nodebuilder
        let base_node_id: Page = self.id().clone().into();
        let base_node_builder = if self.is_folder() {
            NodeBuilder::dir(base_node_id)
        } else {
            NodeBuilder::leaf(base_node_id)
        };

        // compute some stuff for our context menu:
        let (add_parent, position) = if self.is_folder() {
            (Some(self.id().clone()), DirPosition::Last)
        } else {
            (parent_id.clone(), DirPosition::After(self.id().clone()))
        };

        let node = base_node_builder
            .height(node_height)
            .label(node_name)
            .context_menu(|ui| {
                for file_type in self.get_schema().get_all_file_types() {
                    let label = format!("New {}", file_type.type_name());
                    if ui.button(label).clicked() {
                        // We can safely call unwrap on parent here because children can't be root nodes
                        actions.push(ContextMenuActions::Add {
                            parent: add_parent.as_ref().unwrap().clone(),
                            position: position.clone(),
                            file_type,
                        });
                        ui.close();
                    }
                }

                if parent_id.is_some() && self.get_type().exportable() {
                    ui.separator();

                    if ui.button("Export").clicked() {
                        actions.push(ContextMenuActions::Export {
                            id: self.id().clone(),
                        });
                    }
                }

                ui.separator();

                if let Some(parent) = &parent_id
                    && ui.button("Delete").clicked()
                {
                    actions.push(ContextMenuActions::Delete {
                        parent: parent.clone(),
                        deleting: self.id().clone(),
                    });
                }
            });

        builder.node(node);

        if self.is_folder() {
            for child in self.children(objects) {
                child.borrow_mut().build_tree(
                    objects,
                    builder,
                    actions,
                    Some(self.id().clone()),
                    node_height,
                );
            }

            builder.close_dir();
        }
    }
}

impl Project {
    fn build_tree(
        &mut self,
        builder: &mut egui_ltreeview::TreeViewBuilder<'_, Page>,
        actions: &mut Vec<ContextMenuActions>,
        node_height: f32,
    ) {
        // Add special project metadata to the tree
        builder.node(
            NodeBuilder::leaf(Page::ProjectMetadata)
                .label("Project")
                .height(node_height),
        );

        // Create the rest of the top level tree
        for top_level_folder in &self.top_level_folders {
            self.objects
                .get(top_level_folder)
                .unwrap()
                .borrow_mut()
                .build_tree(&self.objects, builder, actions, None, node_height);
        }
    }
}

pub fn ui(editor: &mut ProjectEditor, ui: &mut egui::Ui) {
    let font_size = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .unwrap()
        .size;
    let node_height = (font_size * 1.1).ceil();
    let mut context_menu_actions: Vec<ContextMenuActions> = Vec::new();

    let (_response, actions) = TreeView::new(ui.make_persistent_id("project tree"))
        .allow_multi_selection(false)
        .show_state(ui, &mut editor.tree_state, |builder| {
            editor
                .project
                .build_tree(builder, &mut context_menu_actions, node_height);
        });

    for action in actions {
        match action {
            Action::SetSelected(selected_file_ids) => {
                // Open nodes when they're selected
                if let Some(file_id) = selected_file_ids.first() {
                    editor.set_editor_tab(file_id, false);
                }
            }
            Action::Activate(activation_info) => {
                if let Some(file_id) = activation_info.selected.first() {
                    editor.keep_editor_tab(file_id);
                }
            }
            Action::Move(drag_and_drop) => {
                // Moves only make sense if the source and target are both file objects.
                // This logic only allows for moving individual file objects,
                if let Some(source) = drag_and_drop.source.first()
                    && let Page::FileObject(moving_file_id) = source
                    && let Page::FileObject(target_file_id) = &drag_and_drop.target
                {
                    // Don't move one of the roots
                    if editor.project.is_top_level_folder(moving_file_id) {
                        continue;
                    }

                    let index: usize = match drag_and_drop.position {
                        egui_ltreeview::DirPosition::First => 0,
                        egui_ltreeview::DirPosition::Last => editor
                            .project
                            .objects
                            .get(target_file_id)
                            .expect("objects in the tree must be in the object map")
                            .borrow()
                            .get_base()
                            .children
                            .len(),
                        egui_ltreeview::DirPosition::Before(node) => {
                            if let Page::FileObject(node_id) = node {
                                editor
                                    .project
                                    .objects
                                    .get(&node_id)
                                    .expect("objects in the tree must be in the object map")
                                    .borrow()
                                    .get_base()
                                    .index
                                    .expect("nodes in the tree should always have indexes")
                            } else {
                                log::error!(
                                    "Encountered invalid move to {target_file_id:?}: found file object \
                                     with a child that was not a file object"
                                );
                                continue;
                            }
                        }
                        egui_ltreeview::DirPosition::After(node) => {
                            if let Page::FileObject(node_id) = node {
                                let node_position = editor
                                    .project
                                    .objects
                                    .get(&node_id)
                                    .expect("objects in the tree must be in the object map")
                                    .borrow()
                                    .get_base()
                                    .index
                                    .expect("nodes in the tree should always have indexes");

                                node_position + 1
                            } else {
                                log::error!(
                                    "Encountered invalid move to {target_file_id:?}: found file object \
                                    with a child that was not a file object"
                                );
                                continue;
                            }
                        }
                    };

                    match editor.project.find_object_parent(moving_file_id) {
                        Some(source_file_id) => {
                            if let Err(err) = SCHEMA.move_child(
                                moving_file_id,
                                &source_file_id,
                                target_file_id,
                                index,
                                &editor.project.objects,
                            ) {
                                log::error!("error encountered while moving file object: {err:?}");
                            }
                        }
                        None => log::error!(
                            "failed to move {moving_file_id} to {target_file_id}: could not find moving \
                            object's parent"
                        ),
                    }
                }
            }
            _ => {}
        }
    }

    for action in context_menu_actions {
        match action {
            ContextMenuActions::Delete { parent, deleting } => {
                // Delete the actual file object (removes from other objects and file on disk)
                if let Err(err) =
                    <dyn FileObject>::remove_child(&deleting, &parent, &mut editor.project.objects)
                {
                    log::error!(
                        "Encountered error while trying to delete element: {deleting:?}: {err}"
                    );
                }
            }
            ContextMenuActions::Add {
                parent,
                position,
                file_type,
            } => {
                if let Err(err) = editor.project.create_object(file_type, &parent, position) {
                    log::error!("Encountered error while trying to add child: {err}");
                }
            }
            ContextMenuActions::Export { id } => {
                let project_title = &editor.project.base_metadata.name;
                let export_object = editor.project.objects.get(&id).unwrap().borrow();
                let suggested_title = format!(
                    "{}-{}.md",
                    process_name_for_filename(project_title),
                    process_name_for_filename(&export_object.get_title())
                );
                let export_location_option = FileDialog::new()
                    .set_title(format!("Export {project_title}"))
                    .set_directory(&editor.editor_context.data.data.borrow().last_export_folder)
                    .set_file_name(suggested_title)
                    .save_file();

                // we're exporting a folder or even just a scene, we won't include titles
                // unless the file object overrode this already
                let export_options = ExportOptions {
                    folder_title_depth: ExportDepth::None,
                    scene_title_depth: ExportDepth::None,
                    insert_breaks: editor.project.metadata.export.insert_break_at_end,
                };

                if let Some(export_location) = export_location_option {
                    let mut export_string = String::new();
                    export_object.generate_export(
                        1,
                        &mut export_string,
                        &editor.project.objects,
                        &export_options,
                        false,
                    );
                    if let Err(err) = std::fs::write(&export_location, export_string) {
                        log::error!("Error while attempting to write outline: {err}");
                    }

                    editor
                        .editor_context
                        .data
                        .data
                        .borrow_mut()
                        .last_export_folder = export_location
                        .parent()
                        .map(|val| val.to_path_buf())
                        .unwrap_or_default();
                    editor.editor_context.data.modified = true;
                }
            }
        }
    }
}
