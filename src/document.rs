use crate::{
    Metrics, element::Element, element_output::ElementOutput, error::Error, success::Success,
};
use serde::Serialize;
use url::Url;

/// A document.
#[derive(Serialize)]
pub struct Document {
    url: Url,
    elements: Vec<ElementOutput>,
    metrics: Metrics,
}

impl Document {
    /// Creates a document.
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
