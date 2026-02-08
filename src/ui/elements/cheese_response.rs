use egui::Id;

use crate::ui::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct CheeseResponse {
    pub modified: bool,

    pub tabable_ids: Vec<Id>,
}

impl CheeseResponse {
    pub fn process_response(&mut self, response: &Response, tabable: bool) {
        self.modified |= response.changed();
        if tabable {
            self.tabable_ids.push(response.id);
        }
    }

    pub fn extend(&mut self, other: CheeseResponse) {
        self.modified |= other.modified;
        self.tabable_ids.extend(other.tabable_ids);
    }

    pub fn append_to(self, other: &mut CheeseResponse) {
        other.extend(self);
    }
}

impl From<Response> for CheeseResponse {
    fn from(value: Response) -> Self {
        CheeseResponse {
            modified: value.changed(),
            tabable_ids: vec![value.id],
        }
    }
}
