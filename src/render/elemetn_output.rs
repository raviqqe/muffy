use crate::{element::Element, error::Error, success::Success};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ElementOutput<'a> {
    element: &'a Element,
    results: Vec<Result<Success, Error>>,
}

impl<'a> ElementOutput<'a> {
    pub const fn new(element: Element, results: Vec<Result<Success, Error>>) -> Self {
        Self { element, results }
    }

    pub(crate) fn retain_error(&mut self) {
        self.results.retain(Result::is_err)
    }
}
