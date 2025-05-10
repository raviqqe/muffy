use super::item_output::RenderedItemOutput;
use crate::{element::Element, error::Error};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedElementOutput<'a> {
    element: &'a Element,
    results: Vec<Result<RenderedItemOutput<'a>, &'a Error>>,
}

impl<'a> RenderedElementOutput<'a> {
    pub const fn element(&self) -> &'a Element {
        self.element
    }

    pub fn results(
        &self,
    ) -> impl ExactSizeIterator<Item = &Result<RenderedItemOutput<'a>, &'a Error>> {
        self.results.iter()
    }

    pub(crate) fn retain_error(&mut self) {
        self.results.retain(|result| result.is_err());
    }
}

impl<'a> From<&'a crate::element_output::ElementOutput> for RenderedElementOutput<'a> {
    fn from(output: &'a crate::element_output::ElementOutput) -> Self {
        Self {
            element: output.element(),
            results: output
                .results()
                .map(|result| result.as_ref().map(RenderedItemOutput::from))
                .collect(),
        }
    }
}
