use super::ProjectEditor;

type ActionFunction = dyn FnOnce(&mut ProjectEditor, &egui::Context);

#[derive(Default)]
pub struct Actions(Vec<Box<ActionFunction>>);

impl std::fmt::Debug for Actions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "[Actions ({} scheduled)]", self.0.len())
    }
}

impl Actions {
    pub fn schedule(&mut self, f: impl FnOnce(&mut ProjectEditor, &egui::Context) + 'static) {
        self.0.push(Box::new(f))
    }

    pub fn get(&mut self) -> Vec<Box<ActionFunction>> {
        std::mem::take(&mut self.0)
    }
}
