use core::sync::atomic::AtomicUsize;
use core::time::Duration;
use tokio::time::Instant;

/// A token bucket rate limiter.
pub struct RateLimiter {
    rate: AtomicUsize,
    last_time: Instant,
    count: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(count: usize, window: Duration) -> Self {
        Self {
            rate: AtomicUsize::new(count),
            last_time: Instant::now(),
            count,
            window,
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        future.await
    }
}
