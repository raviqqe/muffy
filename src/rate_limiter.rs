use core::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::time::Instant;

/// A token bucket rate limiter.
pub struct RateLimiter {
    token_count: AtomicU64,
    window_count: AtomicU64,
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
        if self.window_count.load(Ordering::Relaxed) == 0 {
            let elapsed = self.time.elapsed();
            if elapsed >= self.window {
                self.token_count.store(self.supply, Ordering::Relaxed);
                self.window_count.store(0, Ordering::Relaxed);
                self.time = Instant::now();
            } else {
                tokio::time::sleep(self.window - elapsed).await;
                self.token_count.store(self.supply, Ordering::Relaxed);
                self.window_count.store(0, Ordering::Relaxed);
                self.time = Instant::now();
            }
        }

        future.await
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
