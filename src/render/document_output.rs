use super::element_output::ElementOutput;
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
pub struct DocumentOutput<'a> {
    url: Url,
    elements: Vec<&'a ElementOutput<'a>>,
}

impl<'a> DocumentOutput<'a> {
    pub fn new(url: Url, elements: Vec<ElementOutput>) -> Self {
        Self { url, elements }
    }

    pub(crate) fn retain_error(&mut self) {
        for element in &mut self.elements {
            element.retain_error();
        }

        self.elements.retain(|element| element.results().len() != 0);
    }
}
