pub use {
    crate::cheese_error,
    crate::components::{
        file_objects::{FileID, FileObject, FileObjectStore},
        project::Project,
        schema::Schema,
        text::{Text, TextUID},
    },
    crate::schemas::FileType,
    crate::ui::{
        elements::{CheeseResponse, NameBox, RenderedText, layout},
        project_editor::page::{OpenPage, Page},
        project_editor::{EditorContext, ProjectEditor, search::Search},
        render_data::RenderDataStore,
        settings::Settings,
    },
    crate::util::{CheeseError, Dictionary},
    egui::{Response, Ui},
    regex::Regex,
    std::{any::Any, cell::RefCell, collections::HashMap, rc::Rc},
};

pub type SavedRegex = std::sync::LazyLock<Regex>;
