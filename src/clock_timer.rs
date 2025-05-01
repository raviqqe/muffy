use crate::timer::Timer;
use tokio::time::Instant;

#[derive(Debug, Default)]
pub struct ClockTimer {}

impl ClockTimer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Timer for ClockTimer {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
