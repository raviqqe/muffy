use crate::{Metrics, element::Element, error::Error, success::Success};

/// A document.
pub struct Document {
    elements: Vec<(Element, Vec<Result<Success, Error>>)>,
    metrics: Metrics,
}

impl Document {
    /// Creates a document.
    pub fn new(elements: Vec<(Element, Vec<Result<Success, Error>>)>) -> Self {
        Self {
            metrics: Metrics::new(
                elements
                    .iter()
                    .map(|(_, results)| results)
                    .flatten()
                    .filter(|result| result.is_ok())
                    .count(),
                elements
                    .iter()
                    .map(|(_, results)| results)
                    .flatten()
                    .filter(|result| result.is_err())
                    .count(),
            ),
            elements,
        }
    }

    /// Returns elements with their validation results.
    pub fn elements(&self) -> impl Iterator<Item = &(Element, Vec<Result<Success, Error>>)> {
        self.elements.iter()
    }

    /// Returns metrics of document validation.
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }
}
