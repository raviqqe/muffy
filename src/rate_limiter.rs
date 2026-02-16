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
        while self
            .token_count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                if count.is_zero() {
                    None
                } else {
                    Some(count - 1)
                }
            })
            .is_err()
        {
            self.add_supply();
        }

        future.await
    }

    fn add_supply(&self) {
        if self
            .window_count
            .compare_exchange(
                self.window_count.load(Ordering::Relaxed),
                (Instant::now() - self.time).div_duration_f64(self.window) as _,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
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
