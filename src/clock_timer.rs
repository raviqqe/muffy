use crate::timer::Timer;
use tokio::time::Instant;

pub struct ClockTimer {}

impl Timer for ClockTimer {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
