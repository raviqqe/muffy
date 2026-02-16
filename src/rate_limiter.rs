use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

pub struct RateLimiter {
    count: AtomicUsize,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            count: Default::default(),
        }
    }

    pub async fn run<T>(&self, run: impl Future<Output = T>) -> T {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}
