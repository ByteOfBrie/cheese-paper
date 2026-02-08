use crate::ui::prelude::*;

pub trait FileObjectEditor: FileObject {
    fn ui<'a>(&'a mut self, ui: &'a mut egui::Ui, ctx: &'a mut EditorContext) -> CheeseResponse;

    // we cannot use `impl FnMut`` here because we need FileObjectEditor to be dyn-compatible
    // note to Brie: in any other situation please use `impl FnMut` and not `&mut dyn FnMut``
    fn for_each_textbox<'a>(&'a self, f: &mut dyn FnMut(&Text, &'static str));

    #[allow(dead_code)] // included for the sake of completeness
    fn for_each_textbox_mut<'a>(&'a mut self, f: &mut dyn FnMut(&mut Text, &'static str));

    /// provide a list of words that this fileobject wants to automatically add to the spellcheck dictionary
    fn provide_spellcheck_additions(&self) -> Vec<&str> {
        Vec::new()
    }

    fn process_response(&mut self, response: &CheeseResponse) {
        if response.modified {
            self.get_base_mut().file.modified = true;
        }
    }
}
