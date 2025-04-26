pub struct Metrics {
    document: success
    link: usize,
    error: usize,
}

impl Metrics {
    pub fn new() -> Self {
        Metrics {}
    }

    pub fn log(&self, message: &str) {
        println!("Metrics: {}", message);
    }
}

pub struct SubMetrics {
    success: usize,
    error: usize,
}

impl SubMetrics {
    pub fn new() -> Self {
        SubMetrics {
            success: 0,
            error: 0,
        }
    }

    pub fn success(&self) -> usize {
        self.success
    }

    pub fn error(&self) -> usize {
        self.success
    }
}
