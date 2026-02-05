use crate::ui::prelude::*;

#[derive(Debug, PartialEq, Eq)]
pub enum FocusTarget {
    Page(OpenPage),
    Id(&'static str),
}

impl From<OpenPage> for FocusTarget {
    fn from(value: OpenPage) -> Self {
        Self::Page(value)
    }
}

impl From<&'static str> for FocusTarget {
    fn from(value: &'static str) -> Self {
        Self::Id(value)
    }
}

#[derive(Debug, Default)]
pub struct FocusJumper {
    target: Option<FocusTarget>,
}

impl FocusJumper {
    pub fn send<T: Into<FocusTarget>>(&mut self, target: T) {
        self.target = Some(target.into())
    }

    pub fn recieve<T: Into<FocusTarget> + Clone>(&mut self, current: &T) -> bool {
        if let Some(target) = &self.target
            && current.clone().into() == *target
        {
            self.target = None;
            true
        } else {
            false
        }
    }
}
