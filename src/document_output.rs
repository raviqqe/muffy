use crate::{Metrics, element_output::ElementOutput};
use serde::Serialize;
use url::Url;

/// A document output.
#[derive(Debug, Serialize)]
pub struct DocumentOutput {
    url: Url,
    elements: Vec<ElementOutput>,
    metrics: Metrics,
}

impl DocumentOutput {
    /// Creates a document output.
    pub fn new(url: Url, elements: Vec<ElementOutput>) -> Self {
        Self {
            url,
            metrics: Metrics::new(
                elements
                    .iter()
                    .flat_map(ElementOutput::results)
                    .filter(|result| result.is_ok())
                    .count(),
                elements
                    .iter()
                    .flat_map(ElementOutput::results)
                    .filter(|result| result.is_err())
                    .count(),
            ),
            elements,
        }
    }

    /// Returns a URL.
    pub const fn url(&self) -> &Url {
        &self.url
    }

    /// Returns elements with their validation results.
    pub fn elements(&self) -> impl Iterator<Item = &ElementOutput> {
        self.elements.iter()
    }

    /// Returns metrics of document validation.
    pub const fn metrics(&self) -> Metrics {
        self.metrics
    }
}
