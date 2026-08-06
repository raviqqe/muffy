use super::utility::truncate_url;
use crate::element::Element;
use alloc::borrow::Cow;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedElement<'a> {
    name: &'a str,
    attributes: Vec<(&'a str, Cow<'a, str>)>,
}

impl<'a> RenderedElement<'a> {
    pub const fn name(&self) -> &'a str {
        self.name
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn attributes(&self) -> &[(&'a str, Cow<'a, str>)] {
        &self.attributes
    }
}

impl<'a> From<&'a Element> for RenderedElement<'a> {
    fn from(element: &'a Element) -> Self {
        Self {
            name: element.name(),
            attributes: element
                .attributes()
                .iter()
                .map(|(name, value)| (name.as_str(), truncate_url(value)))
                .collect(),
        }
    }
}
