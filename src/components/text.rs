use std::any::TypeId;
use std::ops::{Deref, DerefMut, Range};
use std::sync::atomic::AtomicUsize;

use egui::TextBuffer;
use egui::text::CharIndex;

static GLOBAL_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn get_uid() -> usize {
    GLOBAL_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub type TextUID = usize;

/// An abstraction for a block of text.
#[derive(Debug)]
pub struct Text {
    // underlying text buffer
    pub text: String,

    // version number for knowing if the text has been updated
    pub version: usize,

    // unique id for this text struct
    pub struct_uid: TextUID,
}

impl Text {
    fn new() -> Self {
        Self {
            text: String::new(),
            version: 0,
            struct_uid: get_uid(),
        }
    }

    pub fn downcast(buffer: &dyn TextBuffer) -> &Self {
        assert!(buffer.type_id() == std::any::TypeId::of::<Text>());
        unsafe { &*(buffer as *const dyn TextBuffer as *const Text) }
    }

    pub fn id(&self) -> TextUID {
        self.struct_uid
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text {
            text: s,
            version: 0,
            struct_uid: get_uid(),
        }
    }
}

impl Deref for Text {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.version += 1;
        &mut self.text
    }
}

impl TextBuffer for Text {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        &self.text
    }

    fn insert_text(&mut self, text: &str, char_index: CharIndex) -> usize {
        self.version += 1;
        <String as TextBuffer>::insert_text(&mut self.text, text, char_index)
    }

    fn delete_char_range(&mut self, char_range: Range<CharIndex>) {
        self.version += 1;
        <String as TextBuffer>::delete_char_range(&mut self.text, char_range)
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
