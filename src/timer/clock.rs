use super::Timer;
use tokio::time::Instant;

#[derive(Debug, Default)]
pub struct ClockTimer {}

impl ClockTimer {
    pub const fn new() -> Self {
        Self {}
    }
}

impl Timer for ClockTimer {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
