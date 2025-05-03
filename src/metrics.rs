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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn values() {
        assert_eq!(Metrics::new(42, 7).success(), 42);
        assert_eq!(Metrics::new(42, 7).error(), 7);
        assert_eq!(Metrics::new(42, 7).total(), 49);
    }

    #[test]
    fn add() {
        let mut metrics = Metrics::default();

        metrics.add(false);
        assert_eq!(metrics, Metrics::new(1, 0));
        metrics.add(true);
        assert_eq!(metrics, Metrics::new(1, 1));
    }

    #[test]
    fn has_error() {
        assert!(!Metrics::default().has_error());
        assert!(!Metrics::new(1, 0).has_error());
        assert!(Metrics::new(0, 1).has_error());
    }
}
