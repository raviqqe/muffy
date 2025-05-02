use crate::{Metrics, element::Element, error::Error, success::Success};

pub struct Document {
    elements: Vec<(Element, Vec<Result<Success, Error>>)>,
    metrics: Metrics,
}

impl Document {
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

    pub fn metrics(&self) -> Metrics {
        self.metrics
    }
}
