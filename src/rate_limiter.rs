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

    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}
