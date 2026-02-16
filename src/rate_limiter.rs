use core::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};
use tokio::time::Instant;

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
        future.await
    }

    fn add_supply(&self) -> Instant {
        let old = self.window_count.load(Ordering::Relaxed);
        let new = (Instant::now() - self.time).div_duration_f64(self.window);

        while self
            .window_count
            .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {}

        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn limit_rate() {
        let limiter = RateLimiter::new(5, Duration::from_secs(1));

        for _ in 0..5 {
            limiter
                .run(async {
                    println!("Running task");
                })
                .await;
        }

        limiter
            .run(async {
                println!("This should be rate limited");
            })
            .await;
    }
}
