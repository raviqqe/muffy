use crate::{element::Element, error::Error, success::Success};
use serde::Serialize;

/// An element output.
#[derive(Debug, Serialize)]
pub struct ElementOutput {
    element: Element,
    results: Vec<Result<Success, Error>>,
}

impl ElementOutput {
    pub fn new(element: Element) -> Self {
        Self {
            element,
            results: Vec::new(),
        }
    }

    /// Returns an element.
    pub fn element(&self) -> &Element {
        &self.element
    }

    /// Returns validation results.
    pub fn results(&self) -> impl Iterator<Item = Result<Success, Error>> {
        &self.results
    }
}
