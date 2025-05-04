use super::element_output::ElementOutput;
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
pub struct DocumentOutput<'a> {
    url: &'a Url,
    elements: Vec<ElementOutput<'a>>,
}

impl<'a> DocumentOutput<'a> {
    pub const fn url(&self) -> &'a Url {
        self.url
    }

    pub fn elements(&self) -> impl ExactSizeIterator<Item = &ElementOutput<'a>> {
        self.elements.iter()
    }

    pub(crate) fn retain_error(&mut self) {
        for element in &mut self.elements {
            *element = element.retain_error();
        }

        self.elements.retain(|element| element.results().len() != 0);
    }
}

impl<'a> From<&'a crate::DocumentOutput> for DocumentOutput<'a> {
    fn from(output: &'a crate::DocumentOutput) -> Self {
        Self {
            url: output.url(),
            elements: output
                .elements()
                .map(|output| ElementOutput::from(output))
                .collect(),
        }
    }
}
