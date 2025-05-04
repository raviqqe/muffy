use crate::{element::Element, error::Error, success::Success};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedElementOutput<'a> {
    element: &'a Element,
    results: Vec<&'a Result<Success, Error>>,
}

impl<'a> RenderedElementOutput<'a> {
    pub const fn element(&self) -> &'a Element {
        self.element
    }

    pub fn results(&self) -> impl ExactSizeIterator<Item = &Result<Success, Error>> {
        self.results.iter().copied()
    }

    pub(crate) fn retain_error(&self) -> Self {
        Self {
            element: self.element,
            results: self
                .results
                .iter()
                .copied()
                .filter(|result| result.is_err())
                .collect(),
        }
    }
}

impl<'a> From<&'a crate::element_output::ElementOutput> for RenderedElementOutput<'a> {
    fn from(output: &'a crate::element_output::ElementOutput) -> Self {
        Self {
            element: output.element(),
            results: output.results().collect(),
        }
    }
}
