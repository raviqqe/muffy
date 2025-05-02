use crate::{element::Element, error::Error, success::Success, Metrics};

pub struct Document {
    elements: Vec<(Element, Vec<Result<Success, Error>>)>,
    metrics: Metrics
}

impl Document {
    pub fn new(results: Vec<(Element, Vec<Result<Success, Error>>) -> Self {
        Self {
            metrics: Metrics::new(
                results
                    .iter()
                    .map(|(_, results), results)
                    .flatten()
                    .filter(|result| result.is_ok())
                    .count(),
                results
                    .iter()
                    .map(|(_, results), results)
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
