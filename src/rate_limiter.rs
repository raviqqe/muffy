use core::sync::atomic::AtomicU64;
use core::time::Duration;
use tokio::time::Instant;

/// A token bucket rate limiter.
pub struct RateLimiter {
    count: AtomicU64,
    time: Instant,
    supply: u64,
    window: Duration,
}

impl RateLimiter {
    pub fn new(supply: u64, window: Duration) -> Self {
        Self {
            count: AtomicU64::new(supply),
            time: Instant::now(),
            supply,
            window,
        }
    }

    pub async fn run<T>(&self, future: impl Future<Output = T>) -> T {
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
