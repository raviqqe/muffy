use super::{element_output::RenderedElementOutput, utility::abbreviate_url};
use alloc::borrow::Cow;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedDocumentOutput<'a> {
    url: Cow<'a, str>,
    elements: Vec<RenderedElementOutput<'a>>,
}

impl<'a> RenderedDocumentOutput<'a> {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn elements(&self) -> impl ExactSizeIterator<Item = &RenderedElementOutput<'a>> {
        self.elements.iter()
    }

    pub(crate) fn retain_error(&mut self) {
        for element in &mut self.elements {
            element.retain_error();
        }

        self.elements.retain(|element| element.results().len() != 0);
    }
}

impl<'a> From<&'a crate::DocumentOutput> for RenderedDocumentOutput<'a> {
    fn from(output: &'a crate::DocumentOutput) -> Self {
        Self {
            url: abbreviate_url(output.url().as_str()),
            elements: output.elements().map(RenderedElementOutput::from).collect(),
        }
    }
}
