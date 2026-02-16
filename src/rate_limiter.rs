use core::sync::atomic::AtomicUsize;
use tokio::time::Instant;

pub struct RateLimiter {
    rate: AtomicUsize,
    time: Instant,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            count: Default::default(),
            time: Instant::now(),
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        future.await
    }
}
