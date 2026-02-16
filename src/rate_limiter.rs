pub struct RateLimiter {
    count: AtomicUsize,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
        }
    }

    pub fn increment(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}
