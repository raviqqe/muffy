pub struct Metrics {
    document: CategoryMetrics
    link: CategoryMetrics,
}

impl Metrics {
    pub fn new() -> Self {
        Self {}
    }

    pub fn log(&self, message: &str) {
        println!("Metrics: {}", message);
    }
}

pub struct CategoryMetrics {
    success: usize,
    error: usize,
}

impl CategoryMetrics {
    pub fn new(success: usize, error: usize) -> Self {
        CategoryMetrics {
            success,
            error,
        }
    }

    pub fn success(&self) -> usize {
        self.success
    }

    pub fn error(&self) -> usize {
        self.success
    }
}
