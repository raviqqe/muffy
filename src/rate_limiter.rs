use core::sync::atomic::AtomicUsize;
use tokio::time::Instant;

pub struct RateLimiter {
    count: AtomicUsize,
    time: Instant,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            count: Default::default(),
            time: Instant::now(),
        }
    }

    pub async fn run<T>(&self, run: impl Future<Output = T>) -> T {
        run.await
    }
}
