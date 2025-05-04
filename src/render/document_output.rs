use super::element_output::RenderedElementOutput;
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
pub struct RenderedDocumentOutput<'a> {
    url: &'a Url,
    elements: Vec<RenderedElementOutput<'a>>,
}

impl<'a> RenderedDocumentOutput<'a> {
    pub const fn url(&self) -> &'a Url {
        self.url
    }

    pub fn elements(&self) -> impl ExactSizeIterator<Item = &RenderedElementOutput<'a>> {
        self.elements.iter()
    }

    pub(crate) fn retain_error(&mut self) {
        for element in &mut self.elements {
            *element = element.retain_error();
        }

        self.elements.retain(|element| element.results().len() != 0);
    }
}

impl<'a> From<&'a crate::DocumentOutput> for RenderedDocumentOutput<'a> {
    fn from(output: &'a crate::DocumentOutput) -> Self {
        Self {
            url: output.url(),
            elements: output
                .elements()
                .map(|output| RenderedElementOutput::from(output))
                .collect(),
        }
    }
}
