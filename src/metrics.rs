#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    success: usize,
    error: usize,
}

impl Metrics {
    pub fn new(success: usize, error: usize) -> Self {
        Metrics { success, error }
    }

    pub fn success(&self) -> usize {
        self.success
    }

    pub fn error(&self) -> usize {
        self.error
    }

    pub fn total(&self) -> usize {
        self.success + self.error
    }

    pub fn has_error(&self) -> bool {
        self.error > 0
    }

    pub fn add_error(&mut self, error: bool) {
        if error {
            self.error += 1;
        } else {
            self.success += 1;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.success += other.success;
        self.error += other.error;
    }
}
