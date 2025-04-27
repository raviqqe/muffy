#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    success: usize,
    error: usize,
}

impl Metrics {
    pub const fn new(success: usize, error: usize) -> Self {
        Self { success, error }
    }

    pub const fn success(&self) -> usize {
        self.success
    }

    pub const fn error(&self) -> usize {
        self.error
    }

    pub const fn total(&self) -> usize {
        self.success + self.error
    }

    pub const fn has_error(&self) -> bool {
        self.error > 0
    }

    pub const fn add_error(&mut self, error: bool) {
        if error {
            self.error += 1;
        } else {
            self.success += 1;
        }
    }

    pub const fn merge(&mut self, other: &Self) {
        self.success += other.success;
        self.error += other.error;
    }
}
