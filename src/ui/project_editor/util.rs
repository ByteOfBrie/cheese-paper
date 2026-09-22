use crate::ui::prelude::*;

#[allow(dead_code)]
pub fn project_word_count(project: &Project, ctx: &mut EditorContext) -> usize {
    let mut word_count = 0;

    for file_object in project.objects.values() {
        file_object
            .borrow()
            .as_editor()
            .for_each_textbox(&mut |text: &Text, _| {
                word_count += text.word_count(ctx);
            });
    }

    word_count
}
