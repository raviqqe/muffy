use core::sync::atomic::AtomicUsize;
use core::time::Duration;
use tokio::time::Instant;

/// A token bucket rate limiter.
pub struct RateLimiter {
    rate: AtomicUsize,
    count: usize,
    window: Duration,
    last_time: Instant,
}

impl RateLimiter {
    pub fn new(count: usize, window: Duration) -> Self {
        Self {
            rate: AtomicUsize::new(count),
            last_time: Instant::now(),
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        future.await
    }
}
