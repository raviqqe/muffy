use super::Timer;
use tokio::time::Instant;

/// A wall-clock timer.
#[derive(Debug, Default)]
pub struct ClockTimer {}

impl ClockTimer {
    /// Creates a timer.
    pub const fn new() -> Self {
        Self {}
    }
}

impl Timer for ClockTimer {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
