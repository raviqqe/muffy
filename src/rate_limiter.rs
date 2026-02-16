use core::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};
use tokio::time::{Instant, sleep};

// TODO Use `Timer`?

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
            let duration = self.time.elapsed();

            sleep(self.window * (duration.div_duration_f64(self.window) as u32 + 1) - duration)
                .await;
        }

        future.await
    }

    fn add_supply(&self) {
        let old = self.window_count.load(Ordering::Relaxed);
        let new = self.time.elapsed().div_duration_f64(self.window) as _;

        if new > old
            && self
                .window_count
                .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            self.token_count.store(self.supply, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use futures::future::join_all;
    use tokio::spawn;

    #[tokio::test]
    async fn run_once() {
        let limiter = RateLimiter::new(1, Duration::from_secs(1));

        assert_eq!(limiter.run(async { 42 }).await, 42);
    }

    #[tokio::test]
    async fn run_many_times() {
        const REQUEST_COUNT: u64 = 100_000;
        const SUPPLY: u64 = 1000;
        const WINDOW: Duration = Duration::from_millis(10);

        let time = Instant::now();
        let limiter = Arc::new(RateLimiter::new(SUPPLY, WINDOW));
        let mut futures = vec![];

        for _ in 0..REQUEST_COUNT {
            let limiter = limiter.clone();

            futures.push(spawn(async move { limiter.run(async { 42 }).await }));
        }

        join_all(futures).await;

        let duration = WINDOW * (REQUEST_COUNT / SUPPLY) as _;

        assert!(time.elapsed() >= duration);
        assert!(time.elapsed() < duration.mul_f64(1.5));
    }
}
