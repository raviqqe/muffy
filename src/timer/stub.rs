use crate::timer::Timer;
use tokio::time::Instant;

pub struct StubTimer {
    instant: Instant,
}

impl StubTimer {
    pub fn new() -> Self {
        Self {
            instant: Instant::now(),
        }
    }
}

impl Timer for StubTimer {
    fn now(&self) -> Instant {
        self.instant
    }
}
