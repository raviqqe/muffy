use core::sync::atomic::AtomicU64;
use core::time::Duration;
use tokio::time::Instant;

/// A token bucket rate limiter.
pub struct RateLimiter {
    count: AtomicU64,
    last_time: Instant,
    supply: u64,
    window: Duration,
}

impl RateLimiter {
    pub fn new(supply: u64, window: Duration) -> Self {
        Self {
            count: AtomicU64::new(supply),
            last_time: Instant::now(),
            supply,
            window,
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        future.await
    }
}
