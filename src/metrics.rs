pub struct Metrics {
    document: CategoryMetrics,
    element: CategoryMetrics,
}

impl Metrics {
    pub fn new(document: CategoryMetrics, element: CategoryMetrics) -> Self {
        Self { document, element }
    }
}

pub struct CategoryMetrics {
    success: usize,
    error: usize,
}

impl CategoryMetrics {
    pub fn new(success: usize, error: usize) -> Self {
        CategoryMetrics { success, error }
    }

    pub fn success(&self) -> usize {
        self.success
    }

    pub fn error(&self) -> usize {
        self.success
    }
}
