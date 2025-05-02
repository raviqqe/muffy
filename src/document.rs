use crate::{Metrics, element::Element, error::Error, success::Success};
use serde::Serialize;
use url::Url;

/// A document.
#[derive(Serialize)]
pub struct Document {
    url: Url,
    elements: Vec<(Element, Vec<Result<Success, Error>>)>,
    metrics: Metrics,
}

impl Document {
    /// Creates a document.
    pub fn new(url: Url, elements: Vec<(Element, Vec<Result<Success, Error>>)>) -> Self {
        Self {
            url,
            metrics: Metrics::new(
                elements
                    .iter()
                    .flat_map(|(_, results)| results)
                    .filter(|result| result.is_ok())
                    .count(),
                elements
                    .iter()
                    .flat_map(|(_, results)| results)
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
    pub fn elements(&self) -> impl Iterator<Item = &(Element, Vec<Result<Success, Error>>)> {
        self.elements.iter()
    }

    /// Returns metrics of document validation.
    pub const fn metrics(&self) -> Metrics {
        self.metrics
    }
}
