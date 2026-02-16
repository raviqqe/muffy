use core::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};
use tokio::time::{Instant, sleep};

const SUPPLY_DELAY: Duration = Duration::from_millis(100);

/// A token bucket rate limiter.
pub struct RateLimiter {
    token_count: AtomicU64,
    window_count: AtomicU32,
    time: Instant,
    supply: u64,
    window: Duration,
}

impl RateLimiter {
    pub fn new(supply: u64, window: Duration) -> Self {
        Self {
            token_count: AtomicU64::new(supply),
            window_count: Default::default(),
            time: Instant::now(),
            supply,
            window,
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        while {
            self.add_supply();
            self.token_count
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    if count == 0 { None } else { Some(count - 1) }
                })
                .is_err()
        } {
            sleep(SUPPLY_DELAY).await;
        }

        future.await
    }

    fn add_supply(&self) {
        let old = self.window_count.load(Ordering::Relaxed);
        let new = (Instant::now() - self.time).div_duration_f64(self.window) as _;

        if new > old
            && self
                .window_count
                .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            self.token_count.fetch_add(self.supply, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_task() {
        let limiter = RateLimiter::new(1, Duration::from_secs(1));

        assert_eq!(limiter.run(async { 42 }).await, 42);
    }
}
