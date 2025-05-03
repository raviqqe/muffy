use serde::Serialize;

/// Validation metrics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Metrics {
    success: usize,
    error: usize,
}

impl Metrics {
    /// Creates metrics.
    pub const fn new(success: usize, error: usize) -> Self {
        Self { success, error }
    }

    /// Returns a number of successes.
    pub const fn success(&self) -> usize {
        self.success
    }

    /// Returns a number of errors.
    pub const fn error(&self) -> usize {
        self.error
    }

    /// Returns a total number of successes and errors.
    pub const fn total(&self) -> usize {
        self.success + self.error
    }

    /// Returns `true` if metrics has errors, or `false` otherwise.
    pub const fn has_error(&self) -> bool {
        self.error > 0
    }

    /// Adds a success or error.
    pub const fn add(&mut self, error: bool) {
        if error {
            self.error += 1;
        } else {
            self.success += 1;
        }
    }

    /// Merges two metrics.
    pub const fn merge(&mut self, other: &Self) {
        self.success += other.success;
        self.error += other.error;
    }
}
