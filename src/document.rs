pub struct Document {
    results: Vec<Result<Success, Error>>
}

impl Document {
    pub fn new(results: Vec<(Element, Result)>) -> Self {
        Document {
            metrics: Metrics::new(
        results
            .iter()
            .flatten()
            .filter(|result| result.is_ok())
            .count(),
        results
            .iter()
            .flatten()
            .filter(|result| result.is_err())
            .count()
        }
    }

    pub fn metrics(&self) -> Metrics {
        self.metrics
    }
}
